use tokio_util::codec::Decoder;
use tokio_util::codec::Encoder;

use bytes::{Buf, BufMut, BytesMut};
use std::{cmp, fmt, io, str, usize};

/// This is tokio_util's [`LinesCodec`], but the Encoder does not transform the input
///
/// [`Decoder`]: tokio_util::codec::Decoder
/// [`Encoder`]: tokio_util::codec::Encoder
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct LinesInCodec {
    // Stored index of the next index to examine for a `\n` character.
    // This is used to optimize searching.
    // For example, if `decode` was called with `abc`, it would hold `3`,
    // because that is the next index to examine.
    // The next time `decode` is called with `abcde\n`, the method will
    // only look at `de\n` before returning.
    next_index: usize,

    /// The maximum length for a given line. If `usize::MAX`, lines will be
    /// read until a `\n` character is reached.
    max_length: usize,

    /// Are we currently discarding the remainder of a line which was over
    /// the length limit?
    is_discarding: bool,
}

impl LinesInCodec {
    /// Returns a `LinesInCodec` for splitting up data into lines.
    ///
    /// # Note
    ///
    /// The returned `LinesInCodec` will not have an upper bound on the length
    /// of a buffered line. See the documentation for [`new_with_max_length`]
    /// for information on why this could be a potential security risk.
    ///
    /// [`new_with_max_length`]: tokio_util::codec::LinesInCodec::new_with_max_length()
    pub fn new() -> LinesInCodec {
        LinesInCodec {
            next_index: 0,
            max_length: usize::MAX,
            is_discarding: false,
        }
    }

    /// Returns a `LinesInCodec` with a maximum line length limit.
    ///
    /// If this is set, calls to `LinesInCodec::decode` will return a
    /// [`LinesInCodecError`] when a line exceeds the length limit. Subsequent calls
    /// will discard up to `limit` bytes from that line until a newline
    /// character is reached, returning `None` until the line over the limit
    /// has been fully discarded. After that point, calls to `decode` will
    /// function as normal.
    ///
    /// # Note
    ///
    /// Setting a length limit is highly recommended for any `LinesInCodec` which
    /// will be exposed to untrusted input. Otherwise, the size of the buffer
    /// that holds the line currently being read is unbounded. An attacker could
    /// exploit this unbounded buffer by sending an unbounded amount of input
    /// without any `\n` characters, causing unbounded memory consumption.
    ///
    /// [`LinesInCodecError`]: tokio_util::codec::LinesInCodecError
    pub fn new_with_max_length(max_length: usize) -> Self {
        LinesInCodec {
            max_length,
            ..LinesInCodec::new()
        }
    }

    /// Returns the maximum line length when decoding.
    ///
    /// ```
    /// use std::usize;
    /// use tokio_util::codec::LinesInCodec;
    ///
    /// let codec = LinesInCodec::new();
    /// assert_eq!(codec.max_length(), usize::MAX);
    /// ```
    /// ```
    /// use tokio_util::codec::LinesInCodec;
    ///
    /// let codec = LinesInCodec::new_with_max_length(256);
    /// assert_eq!(codec.max_length(), 256);
    /// ```
    pub fn max_length(&self) -> usize {
        self.max_length
    }
}

fn utf8(buf: &[u8]) -> Result<&str, io::Error> {
    str::from_utf8(buf)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Unable to decode input as UTF8"))
}

fn without_carriage_return(s: &[u8]) -> &[u8] {
    if let Some(&b'\r') = s.last() {
        &s[..s.len() - 1]
    } else {
        s
    }
}

impl Decoder for LinesInCodec {
    type Item = String;
    type Error = LinesInCodecError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<String>, LinesInCodecError> {
        loop {
            // Determine how far into the buffer we'll search for a newline. If
            // there's no max_length set, we'll read to the end of the buffer.
            let read_to = cmp::min(self.max_length.saturating_add(1), buf.len());

            let newline_offset = buf[self.next_index..read_to]
                .iter()
                .position(|b| *b == b'\n');

            match (self.is_discarding, newline_offset) {
                (true, Some(offset)) => {
                    // If we found a newline, discard up to that offset and
                    // then stop discarding. On the next iteration, we'll try
                    // to read a line normally.
                    buf.advance(offset + self.next_index + 1);
                    self.is_discarding = false;
                    self.next_index = 0;
                }
                (true, None) => {
                    // Otherwise, we didn't find a newline, so we'll discard
                    // everything we read. On the next iteration, we'll continue
                    // discarding up to max_len bytes unless we find a newline.
                    buf.advance(read_to);
                    self.next_index = 0;
                    if buf.is_empty() {
                        return Ok(None);
                    }
                }
                (false, Some(offset)) => {
                    // Found a line!
                    let newline_index = offset + self.next_index;
                    self.next_index = 0;
                    let line = buf.split_to(newline_index + 1);
                    let line = &line[..line.len() - 1];
                    let line = without_carriage_return(line);
                    let line = utf8(line)?;
                    return Ok(Some(line.to_string()));
                }
                (false, None) if buf.len() > self.max_length => {
                    // Reached the maximum length without finding a
                    // newline, return an error and start discarding on the
                    // next call.
                    self.is_discarding = true;
                    return Err(LinesInCodecError::MaxLineLengthExceeded);
                }
                (false, None) => {
                    // We didn't find a line or reach the length limit, so the next
                    // call will resume searching at the current offset.
                    self.next_index = read_to;
                    return Ok(None);
                }
            }
        }
    }

    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<String>, LinesInCodecError> {
        Ok(match self.decode(buf)? {
            Some(frame) => Some(frame),
            None => {
                // No terminating newline - return remaining data, if any
                if buf.is_empty() || buf == &b"\r"[..] {
                    None
                } else {
                    let line = buf.split_to(buf.len());
                    let line = without_carriage_return(&line);
                    let line = utf8(line)?;
                    self.next_index = 0;
                    Some(line.to_string())
                }
            }
        })
    }
}

impl<T> Encoder<T> for LinesInCodec
where
    T: AsRef<str>,
{
    type Error = LinesInCodecError;

    fn encode(&mut self, line: T, buf: &mut BytesMut) -> Result<(), LinesInCodecError> {
        let line = line.as_ref();
        buf.reserve(line.len());
        buf.put(line.as_bytes());
        Ok(())
    }
}

impl Default for LinesInCodec {
    fn default() -> Self {
        Self::new()
    }
}

/// An error occured while encoding or decoding a line.
#[derive(Debug)]
pub enum LinesInCodecError {
    /// The maximum line length was exceeded.
    MaxLineLengthExceeded,
    /// An IO error occured.
    Io(io::Error),
}

impl fmt::Display for LinesInCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinesInCodecError::MaxLineLengthExceeded => write!(f, "max line length exceeded"),
            LinesInCodecError::Io(e) => write!(f, "{}", e),
        }
    }
}

impl From<io::Error> for LinesInCodecError {
    fn from(e: io::Error) -> LinesInCodecError {
        LinesInCodecError::Io(e)
    }
}

impl std::error::Error for LinesInCodecError {}
