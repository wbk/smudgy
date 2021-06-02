use futures::SinkExt;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use super::lines_in_codec::LinesInCodec;

const MOTD: &str = include_str!("../../assets/motd.txt");

pub async fn send(
    connection: &mut Framed<TcpStream, LinesInCodec>,
) -> Result<(), super::lines_in_codec::LinesInCodecError> {
    connection.send(MOTD).await
}
