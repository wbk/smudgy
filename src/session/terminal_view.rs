use crate::MainWindow;
use std::{
    cell::{Ref, RefCell},
    cmp::max,
    collections::VecDeque,
    num::NonZeroU32,
    num::NonZeroUsize,
    rc::Rc,
    sync::Arc,
};

use fontdue::{
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
    Font,
};
use lru::LruCache;
use slint::{ComponentHandle, ModelNotify, ModelTracker, Rgba8Pixel, SharedPixelBuffer};
use tiny_skia::{PixmapMut, PixmapPaint, Transform};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use super::{
    styled_line::{self, Style},
    StyledLine,
};

static FONT_DATA: &[u8] = include_bytes!("../../assets/fonts/GeistMonoVF.ttf");

static ECHO_COLOR: slint::Color = slint::Color::from_rgb_u8(255, 192, 255);
static OUTPUT_COLOR: slint::Color = slint::Color::from_rgb_u8(255, 255, 192);

static ANSI_BLACK: slint::Color = slint::Color::from_rgb_u8(0, 0, 0);
static ANSI_RED: slint::Color = slint::Color::from_rgb_u8(170, 0, 0);
static ANSI_GREEN: slint::Color = slint::Color::from_rgb_u8(0, 170, 0);
static ANSI_YELLOW: slint::Color = slint::Color::from_rgb_u8(170, 170, 0);
static ANSI_BLUE: slint::Color = slint::Color::from_rgb_u8(0, 0, 170);
static ANSI_MAGENTA: slint::Color = slint::Color::from_rgb_u8(170, 0, 170);
static ANSI_CYAN: slint::Color = slint::Color::from_rgb_u8(0, 170, 170);
static ANSI_WHITE: slint::Color = slint::Color::from_rgb_u8(204, 204, 204);
static ANSI_BLACK_BOLD: slint::Color = slint::Color::from_rgb_u8(85, 85, 85);
static ANSI_RED_BOLD: slint::Color = slint::Color::from_rgb_u8(255, 85, 85);
static ANSI_GREEN_BOLD: slint::Color = slint::Color::from_rgb_u8(85, 255, 85);
static ANSI_YELLOW_BOLD: slint::Color = slint::Color::from_rgb_u8(255, 255, 85);
static ANSI_BLUE_BOLD: slint::Color = slint::Color::from_rgb_u8(85, 85, 255);
static ANSI_MAGENTA_BOLD: slint::Color = slint::Color::from_rgb_u8(255, 85, 255);
static ANSI_CYAN_BOLD: slint::Color = slint::Color::from_rgb_u8(85, 255, 255);
static ANSI_WHITE_BOLD: slint::Color = slint::Color::from_rgb_u8(255, 255, 255);

static ANSI_COLOR_TABLE: [slint::Color; 16] = [
    ANSI_BLACK,
    ANSI_RED,
    ANSI_GREEN,
    ANSI_YELLOW,
    ANSI_BLUE,
    ANSI_MAGENTA,
    ANSI_CYAN,
    ANSI_WHITE,
    ANSI_BLACK_BOLD,
    ANSI_RED_BOLD,
    ANSI_GREEN_BOLD,
    ANSI_YELLOW_BOLD,
    ANSI_BLUE_BOLD,
    ANSI_MAGENTA_BOLD,
    ANSI_CYAN_BOLD,
    ANSI_WHITE_BOLD,
];

const NON_SCROLLBACK_SIZE_IN_LINES: i32 = 15;

enum ScrollPosition {
    PinnedToEnd,
    ToLine(i32),
}

impl From<styled_line::Color> for slint::Color {
    fn from(value: styled_line::Color) -> Self {
        match value {
            styled_line::Color::AnsiColor { color, bold } => {
                if bold {
                    ANSI_COLOR_TABLE[color as usize + 8]
                } else {
                    ANSI_COLOR_TABLE[color as usize]
                }
            }
            styled_line::Color::Output => OUTPUT_COLOR,
            styled_line::Color::Echo => ECHO_COLOR,
            styled_line::Color::RGB { r, g, b } => slint::Color::from_rgb_u8(r, g, b),
        }
    }
}

// TODO: Benchmark inline
#[inline(always)]
pub fn premultiply_u8(c: u8, a: u8) -> u8 {
    let prod = u32::from(c) * u32::from(a) + 128;
    ((prod + (prod >> 8)) >> 8) as u8
}

type ImageCache = Rc<RefCell<LruCache<usize, SharedPixelBuffer<Rgba8Pixel>>>>;
pub enum ViewableRowCount {
    Clean(usize),
    Dirty,
}

struct TerminalLine {
    font_size: f32,
    row_number: usize,
    layout: fontdue::layout::Layout<Style>,
    styled_line: Arc<StyledLine>,
    last_rasterized_width: u32,
    last_rasterized_height: u32,
    layout_max_width: u32,
}

impl TerminalLine {
    pub fn new(row_number: usize, styled_line: Arc<StyledLine>, font_size: f32) -> Self {
        Self {
            row_number: row_number,
            last_rasterized_width: 0,
            last_rasterized_height: 0,
            layout_max_width: 0,
            layout: Layout::new(CoordinateSystem::PositiveYDown),
            styled_line,
            font_size,
        }
    }

    pub fn append(&mut self, styled_line: Arc<StyledLine>) {
        // force recalc
        self.layout_max_width = 0;
        self.styled_line = Arc::new(self.styled_line.append(styled_line.as_ref()));
    }

    #[inline(always)]
    fn recalc_layout(&mut self, font: &Font, max_width: u32) {
        self.layout_max_width = max_width;

        self.layout.reset(&LayoutSettings {
            max_width: Some(max_width as f32),
            ..Default::default()
        });

        for span in self.styled_line.spans.clone() {
            self.layout.append(
                &[font],
                &TextStyle::with_user_data(
                    self.styled_line
                        .text
                        .get(span.begin_pos..span.end_pos)
                        .unwrap(),
                    self.font_size,
                    0,
                    span.style,
                ),
            )
        }

        // If we're a line, we need to at least render one space
        if self.layout.height() == 0.0f32 {
            self.layout.append(
                &[font],
                &TextStyle::with_user_data(
                    " ",
                    self.font_size,
                    0,
                    Style {
                        fg: super::connection::vt_processor::Color::AnsiColor {
                            color: super::connection::vt_processor::AnsiColor::White,
                            bold: false,
                        },
                    },
                ),
            )
        }
        self.last_rasterized_width = max(
            1,
            self.layout
                .lines()
                .unwrap()
                .iter()
                .map(|line| {
                    let glyph = self.layout.glyphs().get(line.glyph_end).unwrap();
                    glyph.x as u32 + glyph.width as u32
                })
                .max()
                .or(Some(1))
                .unwrap(),
        );

        self.last_rasterized_height = self.layout.height() as u32;
    }

    pub fn pixel_buffer(
        &mut self,
        cache: &ImageCache,
        font: &Font,
        max_width: u32,
    ) -> SharedPixelBuffer<Rgba8Pixel> {
        // recalculate if we have a different amount of room than last render
        let recalc_layout = max_width != self.layout_max_width;

        let mut cache = cache.borrow_mut();

        if recalc_layout {
            self.recalc_layout(font, max_width);
        }

        let existing_buffer = if !recalc_layout {
            cache.get(&self.row_number)
        } else {
            // TODO: this branch can also check the cache and short circuit if the dimensions
            // haven't changed
            None
        };

        if existing_buffer.is_none() {
            let mut buf: SharedPixelBuffer<Rgba8Pixel> =
                SharedPixelBuffer::new(self.last_rasterized_width, self.last_rasterized_height);

            let mut line_pixmap = tiny_skia::PixmapMut::from_bytes(
                buf.make_mut_bytes(),
                self.last_rasterized_width,
                self.last_rasterized_height,
            )
            .unwrap();

            line_pixmap.fill(tiny_skia::Color::TRANSPARENT);

            for glyph in self.layout.glyphs() {
                if glyph.char_data.rasterize() {
                    let (metrics, bitmap) = font.rasterize_config(glyph.key);

                    let mut glyph_pixels = bitmap
                        .iter()
                        .flat_map(|a| {
                            let color: slint::Color = glyph.user_data.fg.into();
                            [
                                premultiply_u8(color.red(), *a),
                                premultiply_u8(color.green(), *a),
                                premultiply_u8(color.blue(), *a),
                                *a,
                            ]
                        })
                        .collect::<Vec<_>>();
                    let glyph_pixmap = PixmapMut::from_bytes(
                        glyph_pixels.as_mut_slice(),
                        metrics.width as u32,
                        metrics.height as u32,
                    )
                    .unwrap();
                    line_pixmap.draw_pixmap(
                        glyph.x as i32,
                        glyph.y as i32,
                        glyph_pixmap.as_ref(),
                        &PixmapPaint {
                            blend_mode: tiny_skia::BlendMode::Source,
                            opacity: 1.0,
                            quality: tiny_skia::FilterQuality::Nearest,
                        },
                        Transform::default(),
                        None,
                    );
                }
            }

            cache.put(self.row_number, buf.clone());
            buf
        } else {
            existing_buffer.unwrap().clone()
        }
    }
}

pub enum ViewAction {
    AppendCompleteLine(Arc<StyledLine>),
    AppendPartialLine(Arc<StyledLine>),
}

pub struct TerminalView {
    font: fontdue::Font,
    row_pixel_buffer_cache: ImageCache,
    viewable_size: RefCell<(NonZeroU32, NonZeroU32)>,
    cached_row_count: Rc<RefCell<ViewableRowCount>>,
    current_row_number: RefCell<usize>,
    lines: Rc<RefCell<VecDeque<TerminalLine>>>,
    notify: slint::ModelNotify,
    pub tx: UnboundedSender<ViewAction>,
    rx: RefCell<UnboundedReceiver<ViewAction>>,
    font_size: f32,
    last_line_terminated: RefCell<bool>,
    row_count_model: Rc<SharedSingleIntModel>,
    scroll_position: RefCell<ScrollPosition>,
}

impl TerminalView {
    pub fn new(weak_window: slint::Weak<MainWindow>) -> Self {
        let font_size = weak_window.upgrade().unwrap().window().scale_factor() * 16.0;

        let font = fontdue::Font::from_bytes(
            FONT_DATA,
            fontdue::FontSettings {
                scale: font_size,
                load_substitutions: false,
                collection_index: 0,
            },
        )
        .unwrap();

        let (tx, rx) = mpsc::unbounded_channel::<ViewAction>();

        Self {
            font,
            viewable_size: RefCell::new((NonZeroU32::MIN, NonZeroU32::MIN)),
            current_row_number: RefCell::new(0),
            row_pixel_buffer_cache: Rc::new(RefCell::new(LruCache::new(
                NonZeroUsize::new(500).unwrap(),
            ))),
            lines: Rc::new(RefCell::new(VecDeque::with_capacity(10000))),
            notify: ModelNotify::default(),
            cached_row_count: Rc::new(RefCell::new(ViewableRowCount::Dirty)),
            font_size,
            tx,
            rx: RefCell::new(rx),
            last_line_terminated: RefCell::new(true),
            row_count_model: Rc::new(SharedSingleIntModel::new(0)),
            scroll_position: RefCell::new(ScrollPosition::PinnedToEnd),
        }
    }

    pub fn row_count_model(&self) -> Rc<SharedSingleIntModel> {
        self.row_count_model.clone()
    }

    pub fn set_scroll_position(&self, value: i32) {
        let mut scroll_position = self.scroll_position.borrow_mut();

        *scroll_position = if value == -1 {
            ScrollPosition::PinnedToEnd
        } else {
            ScrollPosition::ToLine(value)
        };

        self.cached_row_count.replace(ViewableRowCount::Dirty);

        self.notify.reset();
    }

    pub fn handle_incoming_lines(&self) {
        let mut rx = self.rx.borrow_mut();
        let pending = rx.len();
        if pending > 0 {
            let mut lines = self.lines.borrow_mut();
            let mut current_row_number = self.current_row_number.borrow_mut();
            let mut last_line_terminated = self.last_line_terminated.borrow_mut();

            for _ in 0..pending {
                let (line, is_terminated) = match rx.blocking_recv().unwrap() {
                    ViewAction::AppendCompleteLine(line) => (line, true),
                    ViewAction::AppendPartialLine(line) => (line, false),
                };

                if *last_line_terminated {
                    lines.push_back(TerminalLine::new(*current_row_number, line, self.font_size));
                    *current_row_number += 1;
                } else {
                    lines.back_mut().unwrap().append(line);
                }

                *last_line_terminated = is_terminated;
            }

            let mut cached_row_count = self.cached_row_count.borrow_mut();
            *cached_row_count = ViewableRowCount::Dirty;
            self.notify.reset();
        }
    }

    pub fn set_viewable_size(&self, width: NonZeroU32, height: NonZeroU32) {
        let mut viewable_size = self.viewable_size.borrow_mut();

        if viewable_size.0 != width || viewable_size.1 != height {
            *viewable_size = (width, height);
            let mut cached_row_count = self.cached_row_count.borrow_mut();
            *cached_row_count = ViewableRowCount::Dirty;
            self.notify.reset();
        }
    }
}

impl slint::Model for TerminalView {
    type Data = slint::Image;

    // This only shows as many (wrapping) lines as are viewable, which may change as the window
    //  is resized, or when the lines themselves change, so some light caching is done on the count

    fn row_count(&self) -> usize {
        let mut cached_row_count = self.cached_row_count.borrow_mut();

        match *cached_row_count {
            ViewableRowCount::Clean(count) => count,
            ViewableRowCount::Dirty => {
                let viewable_size: Ref<(std::num::NonZero<u32>, std::num::NonZero<u32>)> =
                    self.viewable_size.borrow();
                let mut height: u32 = viewable_size.1.into();
                let mut count = 0;

                let mut lines = self.lines.borrow_mut();

                let offset =
                    if let ScrollPosition::ToLine(ref line) = *self.scroll_position.borrow() {
                        max(0, lines.len() as i32 - line) as usize
                    } else {
                        0 as usize
                    };

                let mut scrollback_iter = lines.iter_mut().rev();
                // the first NON_SCROLLBACK_SIZE_IN_LINES (bottom) lines always start from the end
                for line in &mut scrollback_iter {
                    let pixel_buffer = line.pixel_buffer(
                        &self.row_pixel_buffer_cache,
                        &self.font,
                        viewable_size.0.into(),
                    );
                    let line_height = pixel_buffer.height();
                    if line_height >= height {
                        break;
                    }
                    height -= line_height;
                    count += 1;
                    if count == NON_SCROLLBACK_SIZE_IN_LINES {
                        break;
                    }
                }
                if count == NON_SCROLLBACK_SIZE_IN_LINES {
                    if let Some(_) = scrollback_iter.nth(offset) {
                        // subsequent lines come from the scrollback

                        for line in scrollback_iter {
                            let pixel_buffer = line.pixel_buffer(
                                &self.row_pixel_buffer_cache,
                                &self.font,
                                viewable_size.0.into(),
                            );
                            let line_height = pixel_buffer.height();
                            if line_height >= height {
                                break;
                            }
                            height -= line_height;
                            count += 1;
                        }
                    }
                }

                *cached_row_count = ViewableRowCount::Clean(count as usize);
                self.row_count_model.replace(lines.len() as i32);

                count as usize
            }
        }
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        let viewable_size = self.viewable_size.borrow();
        let mut lines = self.lines.borrow_mut();
        let scroll_position = self.scroll_position.borrow();

        let mut offset = lines.len() - self.row_count();

        if let ScrollPosition::ToLine(scroll_line) = *scroll_position {
            if row + offset + (NON_SCROLLBACK_SIZE_IN_LINES as usize) < lines.len() {
                offset = max(
                    0,
                    (scroll_line as usize)
                        .checked_sub(self.row_count())
                        .or(Some(0))
                        .unwrap(),
                );
            }
        }

        match lines.get_mut(row + offset) {
            Some(line) => {
                let pixel_buffer = line.pixel_buffer(
                    &self.row_pixel_buffer_cache,
                    &self.font,
                    viewable_size.0.into(),
                );
                Some(slint::Image::from_rgba8_premultiplied(pixel_buffer))
            }
            _ => None,
        }
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}

pub struct SharedSingleIntModel {
    value: RefCell<i32>,
    notify: ModelNotify,
}

impl SharedSingleIntModel {
    pub fn new(value: i32) -> Self {
        Self {
            value: RefCell::new(value),
            notify: ModelNotify::default(),
        }
    }

    pub fn replace(&self, value: i32) -> i32 {
        let ret = self.value.replace(value);
        self.notify.row_changed(0);
        ret
    }
}

impl slint::Model for SharedSingleIntModel {
    type Data = i32;

    fn row_count(&self) -> usize {
        1
    }

    fn row_data(&self, row: usize) -> Option<Self::Data> {
        (row == 0).then_some(self.value.borrow().clone())
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}
