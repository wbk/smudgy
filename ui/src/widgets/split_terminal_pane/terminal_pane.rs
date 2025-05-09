use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

use iced::{
    Background, Event, Pixels, Rectangle,
    advanced::{
        self, Layout, Widget, clipboard,
        graphics::core::keyboard,
        layout, mouse,
        renderer::{self, Quad},
        text::{self, Paragraph},
        widget::{Tree, tree},
    },
    alignment,
    event, touch,
    widget::text::LineHeight,
};
use smudgy_core::terminal_buffer::TerminalBuffer;

use crate::assets::fonts::GEIST_MONO_VF;

mod spans;

use smudgy_core::terminal_buffer::selection::{BufferPosition, LineSelection, Selection};
use spans::Spans;

type Link = ();

#[derive(Debug, Clone)]
struct ParagraphCache<P: text::Paragraph> {
    spans: Spans<Link>,
    paragraph: P,
    max_valid_width: f32,
    selection: LineSelection,
}

/// State specific to the TerminalPane widget instance.
#[derive(Debug, Clone)]
struct State<P: text::Paragraph> {
    pub last_line_number: usize,
    pub cache: Vec<ParagraphCache<P>>,
    pub is_focused: bool,
}

impl<P: text::Paragraph> Default for State<P> {
    fn default() -> Self {
        Self {
            last_line_number: 0,
            cache: Vec::new(),
            is_focused: false,
        }
    }
}

impl<P: text::Paragraph> State<P> {
    fn hit_test(&self, bounds: Rectangle, point: iced::Point) -> Option<BufferPosition> {
        let mut line_top = bounds.height;

        for (line, offset) in self.cache.iter().zip(0..) {
            let line_number = self.last_line_number - offset;
            let line_bottom = line_top;
            line_top -= line.paragraph.min_height();

            if point.y >= line_top && point.y < line_bottom {
                let point_in_paragraph = iced::Point::new(point.x, point.y - line_top);
                return match line.paragraph.hit_test(point_in_paragraph) {
                    Some(hit) => Some(BufferPosition {
                        line: line_number,
                        column: hit.cursor(),
                    }),
                    None => {
                        // The point is not in the paragraph, but it is to the left or right of it, let's snap to it
                        if point_in_paragraph.x < 0.0 {
                            Some(BufferPosition {
                                line: line_number,
                                column: 0,
                            })
                        } else {
                            // The point is to the right of the paragraph, but we need to figure out which line it is on
                            // Let's find the last span that is to the left of the point

                            (0..line.spans.spans().len())
                                .filter_map(|idx| {
                                    line.paragraph
                                        .span_bounds(idx)
                                        .iter()
                                        .filter(|span_bounds| {
                                            span_bounds.y <= point_in_paragraph.y
                                                && span_bounds.y + span_bounds.height
                                                    > point_in_paragraph.y
                                        })
                                        .reduce(|acc, item| if acc.x > item.x { acc } else { item })
                                        .map(|span_bounds| (span_bounds.clone(), idx))
                                })
                                .reduce(|acc, item| if acc.0.x > item.0.x { acc } else { item })
                                .map(|(_, idx)| BufferPosition {
                                    line: line_number,
                                    column: line
                                        .spans
                                        .spans()
                                        .iter()
                                        .take(idx + 1)
                                        .fold(0, |acc, span| acc + span.text.len()),
                                })
                        }
                    }
                };
            }
        }
        None
    }
}

pub struct TerminalPane<'a> {
    terminal_buffer: Ref<'a, TerminalBuffer>,
    selection: Rc<RefCell<Selection>>,
    last_line_number: Option<usize>,
}

impl<'a> TerminalPane<'a> {
    pub fn new(buffer: Ref<'a, TerminalBuffer>, selection: Rc<RefCell<Selection>>) -> Self {
        log::debug!("TerminalPane::new() called");
        Self {
            terminal_buffer: buffer,
            selection,
            last_line_number: None,
        }
    }

    pub fn last_line_number(mut self, last_line_number: usize) -> Self {
        self.last_line_number = Some(last_line_number);
        self
    }
}
// Widget impl now uses concrete theme::Theme for its Theme generic

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for TerminalPane<'a>
where
    Renderer: text::Renderer<Font = iced::Font> + 'a,
    Renderer::Paragraph:
        iced::advanced::text::Paragraph<Font = iced::Font> + Clone + std::fmt::Debug + 'static,
    Theme: iced::widget::text::Catalog + 'a,
{
    fn size(&self) -> iced::Size<iced::Length> {
        iced::Size::new(iced::Length::Fill, iced::Length::Fill)
    }

    fn size_hint(&self) -> iced::Size<iced::Length> {
        iced::Size::new(iced::Length::Fill, iced::Length::Fill)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Renderer::Paragraph>::default())
    }

    fn layout(
        &self,
        tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let selection = self.selection.borrow();

        let mut new_cache: Vec<ParagraphCache<Renderer::Paragraph>> =
            Vec::with_capacity(state.cache.len());

        let mut i = 0;

        let mut available_y = limits.max().height;

        state.last_line_number = self
            .last_line_number
            .unwrap_or(self.terminal_buffer.last_line_number());

        for (line_number, line) in self
            .terminal_buffer
            .iter_rev_with_line_number(self.last_line_number)
        {
            if available_y < 0.0 {
                break;
            }

            // look for a matching cached Paragraph in state.paragraphs[i] or state.paragraphs[i + 1],
            // advancing i by 1 if a match is found
            if let Some(cache) = state.cache.get_mut(i) {
                let line_selection = selection.for_line(line_number);

                if cache.selection != line_selection {
                    match line_selection {
                        None => {
                            cache.spans.select_none();
                        }
                        Some((0, usize::MAX)) => {
                            cache.spans.select_all();
                        }
                        Some((from, to)) => {
                            cache.spans.select_range(from, to);
                        }
                    }
                } else if Rc::ptr_eq(&cache.spans.spans(), &line.spans) {
                    i = i + 1;

                    if limits.max().width > cache.max_valid_width
                        || limits.max().width < cache.paragraph.min_bounds().width
                    {
                        cache.paragraph.resize(limits.max());
                        cache.max_valid_width = limits.max().width;
                    }

                    new_cache.push(cache.clone());

                    available_y -= cache.paragraph.min_height();
                    continue;
                }
            }

            let line_selection = selection.for_line(line_number);
            let spans = Spans::with_selection(line.spans.clone(), line_selection);

            let spans_vec = spans.spans();

            let text = iced::advanced::text::Text {
                content: Vec::as_ref(&spans_vec),
                bounds: limits.max(),
                size: Pixels(16.0),
                font: GEIST_MONO_VF,
                line_height: LineHeight::Absolute(Pixels(20.0)),
                align_x: text::Alignment::Left,
                align_y: alignment::Vertical::Top,
                shaping: text::Shaping::Advanced,
                wrapping: text::Wrapping::WordOrGlyph,
            };

            let paragraph = Renderer::Paragraph::with_spans(text);

            available_y -= paragraph.min_height();

            new_cache.push(ParagraphCache {
                spans: spans,
                paragraph,
                max_valid_width: limits.max().width,
                selection: line_selection,
            });
        }

        state.cache = new_cache;

        layout::atomic(limits, iced::Length::Fill, iced::Length::Fill)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style_defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();

        layout
            .bounds()
            .intersection(viewport)
            .map(|clipped_viewport| {
                let mut y = layout.bounds().y + layout.bounds().height;
                for cache in state.cache.iter() {
                    y -= cache.paragraph.min_height();

                    for selected_span_idx in cache.spans.selected().iter() {
                        let span_bounds_list = cache.paragraph.span_bounds(*selected_span_idx);

                        for span_bounds in span_bounds_list.iter() {
                            Rectangle {
                                x: layout.bounds().x + span_bounds.x,
                                y: span_bounds.y + y,
                                width: span_bounds.width,
                                height: span_bounds.height,
                            }
                            .intersection(&clipped_viewport)
                            .map(|bounds| {
                                renderer.fill_quad(
                                    Quad {
                                        bounds,
                                        ..Default::default()
                                    },
                                    Background::Color(iced::Color::from_rgb8(60, 60, 60)),
                                );
                            });
                        }
                    }

                    renderer.fill_paragraph(
                        &cache.paragraph,
                        iced::Point::new(layout.bounds().x, y),
                        iced::Color::WHITE,
                        clipped_viewport,
                    );
                }
            });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::Idle
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        clipboard: &mut dyn advanced::Clipboard,
        shell: &mut advanced::Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
                let mut selection = self.selection.borrow_mut();

                if cursor
                    .position_in(layout.bounds())
                    .map(|click_position| {
                        if let Some(position) = state.hit_test(layout.bounds(), click_position) {
                            *selection = Selection::Selecting {
                                origin: position.clone(),
                                from: position.clone(),
                                to: position,
                            };
                            shell.invalidate_layout();
                        }
                    })
                    .is_some()
                {
                    state.is_focused = true;
                    // We don't capture the event here because we want the click input to bubble up, so we can also use it to focus this session's input
                } else {
                    state.is_focused = false;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. }) => {
                let mut selection = self.selection.borrow_mut();

                if let Selection::Selecting {
                    origin: _,
                    ref from,
                    ref to,
                } = *selection
                {
                    *selection = Selection::Selected {
                        from: from.clone(),
                        to: to.clone(),
                    };

                    shell.invalidate_layout();
                    // We don't capture the event here because we want the click input to bubble up, so we can also use it to focus this session's input
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position: _ }) => {
                let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
                let mut selection = self.selection.borrow_mut();

                match *selection {
                    Selection::Selecting {
                        ref origin,
                        from: _,
                        to: _,
                    } => {
                        if let Some(cursor_position) = cursor.position_from(layout.position()) {
                            if let Some(position) = state.hit_test(layout.bounds(), cursor_position)
                            {
                                let (from, to) = if position.line < origin.line
                                    || (position.line == origin.line
                                        && position.column < origin.column)
                                {
                                    (position, origin.clone())
                                } else {
                                    (origin.clone(), position)
                                };

                                *selection = Selection::Selecting {
                                    origin: origin.clone(),
                                    from: from,
                                    to: to,
                                };

                                shell.invalidate_layout();
                                shell.request_redraw();
                                shell.capture_event();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

                if state.is_focused {
                    match key.as_ref() {
                        keyboard::Key::Character("c") if modifiers.command() => {
                            let to_copy =
                                self.terminal_buffer.selected_text(&self.selection.borrow());

                            if !to_copy.is_empty() {
                                clipboard.write(clipboard::Kind::Standard, to_copy);
                            }

                            shell.capture_event();
                        }
                        _ => {},
                    }
                }
            }
            _ => {},
        }
    }
}

pub fn terminal_pane<'a>(
    buffer: Ref<'a, TerminalBuffer>,
    selection: Rc<RefCell<Selection>>,
) -> TerminalPane<'a> {
    TerminalPane::new(buffer, selection)
}
