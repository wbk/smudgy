use std::{
    cell::{Ref, RefCell},
    rc::{self, Rc},
};

use iced::{
    Element, Event, Point, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{self, Node},
        mouse, text,
        widget::{Tree, tree},
    },
    event,
};
use log::info;
use smudgy_core::terminal_buffer::{TerminalBuffer, selection::Selection};

mod scroll_bar;
mod terminal_pane;

use terminal_pane::{TerminalPane, terminal_pane};

const LINE_HEIGHT: f32 = 20.0;

struct SplitTerminalPane<'a> {
    pub selection: Rc<RefCell<Selection>>,
    pub buffer: Ref<'a, TerminalBuffer>,
}

impl<'a> SplitTerminalPane<'a> {
    pub fn new(buffer: Ref<'a, TerminalBuffer>, selection: Rc<RefCell<Selection>>) -> Self {
        Self { selection, buffer }
    }

    fn terminal_pane(&self) -> TerminalPane<'a> {
        terminal_pane(Ref::clone(&self.buffer), self.selection.clone())
    }

    fn scroll_bar_element<Message, Theme, Renderer: iced::advanced::Renderer>(
        &self,
        visible_lines: f32,
        state: Option<rc::Weak<RefCell<State>>>,
    ) -> Element<'a, Message, Theme, Renderer> {
        let max_line = self.buffer.last_line_number() as f32;
        let min_line = (self.buffer.last_line_number() - self.buffer.len()) as f32;
        let local_state = state.clone();

        let last_line = state
            .map(|state| {
                state
                    .upgrade()
                    .map(|state| {
                        let state = state.borrow();

                        if state.is_split() {
                            state.scroll_bar_value
                        } else {
                            max_line
                        }
                    })
                    .unwrap_or(max_line)
            })
            .unwrap_or(max_line);

        scroll_bar::scroll_bar(min_line, max_line, visible_lines, last_line)
            .on_change(move |value| {
                local_state.as_ref().map(|state| {
                    state.upgrade().map(|state| {
                        let mut state = state.borrow_mut();

                        let value = if max_line < visible_lines {
                            max_line
                        } else {
                            value
                        };
                        state.scroll_bar_value = value;
                        state.is_split = value < max_line;
                    })
                });
            })
            .into()
    }
}

#[derive(Default)]
struct State {
    visible_lines: f32,
    scroll_bar_value: f32,
    is_split: bool,
}

impl State {
    fn is_split(&self) -> bool {
        self.is_split
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for SplitTerminalPane<'a>
where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font> + 'a,
    Renderer::Paragraph:
        iced::advanced::text::Paragraph<Font = iced::Font> + Clone + std::fmt::Debug + 'static,
    Theme: iced::widget::text::Catalog + 'a,
{
    fn children(&self) -> Vec<tree::Tree> {
        vec![
            Tree::new(&Element::<(), Theme, Renderer>::new(self.terminal_pane())),
            Tree::new(&Element::<(), Theme, Renderer>::new(self.terminal_pane())),
            Tree::new::<(), Theme, Renderer>(&self.scroll_bar_element(0.0, None)),
        ]
    }

    fn diff(&self, _tree: &mut Tree) {}

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Rc<RefCell<State>>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Rc::new(RefCell::new(State::default())))
    }

    fn size(&self) -> iced::Size<iced::Length> {
        iced::Size::new(iced::Length::Fill, iced::Length::Fill)
    }

    fn size_hint(&self) -> iced::Size<iced::Length> {
        iced::Size::new(iced::Length::Fill, iced::Length::Fill)
    }

    fn layout(
        &self,
        tree: &mut tree::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_ref::<Rc<RefCell<State>>>();

        let mut children = tree.children.iter_mut();
        let scrollback_pane_tree = children.next().unwrap();
        let main_pane_tree = children.next().unwrap();
        let scrollbar_tree = children.next().unwrap();

        let terminal_pane_limits = limits.shrink(Size::new(scroll_bar::SCROLLBAR_WIDTH, 0.0));
        let scrollbar_limits = limits.shrink(Size::new(terminal_pane_limits.max().width, 0.0));

        let (main_pane_node, scrollback_pane_node) = if state.borrow().is_split() {
            let main_pane_limits = terminal_pane_limits.loose().max_height(200.0);

            let mut main_pane_node = <TerminalPane<'_> as Widget<Message, Theme, Renderer>>::layout(
                &self.terminal_pane(),
                main_pane_tree,
                renderer,
                &main_pane_limits,
            );

            let scrollback_pane_limits =
                terminal_pane_limits.shrink(Size::new(0.0, main_pane_node.bounds().height));

            let scrollback_pane_node =
                <TerminalPane<'_> as Widget<Message, Theme, Renderer>>::layout(
                    &self
                        .terminal_pane()
                        .last_line_number(state.borrow().scroll_bar_value as usize),
                    scrollback_pane_tree,
                    renderer,
                    &scrollback_pane_limits,
                );

            main_pane_node =
                main_pane_node.move_to(Point::new(0.0, scrollback_pane_node.size().height));

            (main_pane_node, scrollback_pane_node)
        } else {
            let main_pane_node = <TerminalPane<'_> as Widget<Message, Theme, Renderer>>::layout(
                &self.terminal_pane(),
                main_pane_tree,
                renderer,
                &terminal_pane_limits,
            );

            (main_pane_node, Node::new(Size::new(0.0, 0.0)))
        };

        let visible_lines = terminal_pane_limits.max().height / LINE_HEIGHT;

        let scrollbar_node = self
            .scroll_bar_element::<Message, Theme, Renderer>(
                visible_lines,
                Some(Rc::downgrade(state)),
            )
            .as_widget()
            .layout(scrollbar_tree, renderer, &scrollbar_limits);

        let main_pane_width = main_pane_node.size().width;

        let mut state = state.borrow_mut();
        state.visible_lines = visible_lines;

        Node::with_children(
            limits.max(),
            vec![
                scrollback_pane_node,
                main_pane_node,
                scrollbar_node.move_to(Point::new(main_pane_width, 0.0)),
            ],
        )
    }

    fn draw(
        &self,
        tree: &tree::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        let state = tree.state.downcast_ref::<Rc<RefCell<State>>>();

        let mut children = tree.children.iter();
        let scrollback_pane_tree = children.next().unwrap();
        let main_pane_tree = children.next().unwrap();
        let scroll_bar_tree = children.next().unwrap();

        let mut children = layout.children();
        let scrollback_pane_layout = children.next().unwrap();
        let main_pane_layout = children.next().unwrap();
        let scrollbar_layout = children.next().unwrap();

        if state.borrow().is_split() {
            <TerminalPane<'_> as Widget<Message, Theme, Renderer>>::draw(
                &self.terminal_pane(),
                scrollback_pane_tree,
                renderer,
                theme,
                style,
                scrollback_pane_layout,
                cursor,
                viewport,
            );
        }

        <TerminalPane<'_> as Widget<Message, Theme, Renderer>>::draw(
            &self.terminal_pane(),
            main_pane_tree,
            renderer,
            theme,
            style,
            main_pane_layout,
            cursor,
            viewport,
        );

        self.scroll_bar_element::<Message, Theme, Renderer>(
            state.borrow().visible_lines,
            Some(Rc::downgrade(state)),
        )
        .as_widget()
        .draw(
            scroll_bar_tree,
            renderer,
            theme,
            style,
            scrollbar_layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<Rc<RefCell<State>>>();

        let scroll_bar = self.scroll_bar_element(0.0, Some(Rc::downgrade(state)));

        [
            &Element::<Message, Theme, Renderer>::new(self.terminal_pane()),
            &Element::<Message, Theme, Renderer>::new(self.terminal_pane()),
            &scroll_bar,
        ]
        .iter_mut()
        .zip(&tree.children)
        .zip(layout.children())
        .map(|((child, state), layout)| {
            child
                .as_widget()
                .mouse_interaction(state, layout, cursor, viewport, renderer)
        })
        .fold(mouse::Interaction::Idle, |left_i, right_i| {
            if left_i == mouse::Interaction::Idle {
                right_i
            } else {
                left_i
            }
        })
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<Rc<RefCell<State>>>();

        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event {
            if cursor.position_in(layout.bounds()).is_some() {
                let mut state = state.borrow_mut();
                let max_line = self.buffer.last_line_number() as f32;
                let min_line = (self.buffer.last_line_number() - self.buffer.len()) as f32;

                // We don't update the scroll bar position when new lines come in, so if we're not split (it's fixed to the bottom),
                // update it lazily now before we do any arithmetic dependant on its value
                if !state.is_split {
                    state.scroll_bar_value = max_line;
                }

                match delta {
                    mouse::ScrollDelta::Lines { y, .. } => {
                        state.scroll_bar_value -= y;
                        state.scroll_bar_value = state
                            .scroll_bar_value
                            .clamp(min_line, max_line);
                        state.is_split = state.scroll_bar_value < max_line;
                        shell.invalidate_layout();
                        shell.request_redraw();
                        shell.capture_event();
                    }
                    mouse::ScrollDelta::Pixels { y, .. } => {
                        if *y > 0.0 {
                            state.scroll_bar_value -= (*y / 10.0).min(1.0);
                        } else {
                            state.scroll_bar_value += (*y / 10.0).min(1.0);
                        }
                        state.scroll_bar_value = state
                            .scroll_bar_value
                            .clamp(min_line, max_line);
                        state.is_split = state.scroll_bar_value < max_line;
                        shell.invalidate_layout();
                        shell.request_redraw();
                        shell.capture_event();
                    }
                }
                return;
            }
        }

        let mut scroll_bar =
            self.scroll_bar_element(state.borrow().visible_lines, Some(Rc::downgrade(state)));

        [
            &mut Element::<Message, Theme, Renderer>::new(self.terminal_pane()),
            &mut Element::<Message, Theme, Renderer>::new(self.terminal_pane()),
            &mut scroll_bar,
        ]
        .iter_mut()
        .zip(&mut tree.children)
        .zip(layout.children())
        .map(|((child, state), layout)| {
            child.as_widget_mut().update(
                state, event, layout, cursor, renderer, clipboard, shell, viewport,
            )
        })
        .for_each(drop);
    }
}

pub fn split_terminal_pane<'a, Message, Theme, Renderer>(
    buffer: Ref<'a, TerminalBuffer>,
    selection: Rc<RefCell<Selection>>,
) -> Element<'a, Message, Theme, Renderer>
where
    Renderer: text::Renderer<Font = iced::Font> + 'a,
    Renderer::Paragraph:
        iced::advanced::text::Paragraph<Font = iced::Font> + Clone + std::fmt::Debug + 'static,
    Theme: iced::widget::text::Catalog + 'a,
    Message: 'a,
{
    Element::new(SplitTerminalPane::new(buffer, selection))
}
