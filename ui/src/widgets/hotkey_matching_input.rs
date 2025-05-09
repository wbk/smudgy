use std::{collections::HashMap, marker::PhantomData};

use iced::{
    Element,
    advanced::{
        Widget,
        widget::{Tree, tree},
    },
    keyboard::{
        self, Key,
        key::{self, Named, Physical},
    },
    widget::{
        Text, TextInput, text,
        text_input::{self, Id},
    },
};
use smudgy_core::session::HotkeyId;

use crate::helpers::hotkeys::MaybePhysicalKey;

pub struct HotkeyMatchingInput<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: text_input::Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    hotkeys: &'a HashMap<MaybePhysicalKey, Vec<(keyboard::Modifiers, HotkeyId)>>,
    hooks: HashMap<keyboard::Key, Message>,
    text_input: TextInput<'a, Message, Theme, Renderer>,
    on_match: Option<Box<dyn Fn(HotkeyId) -> Message>>,
    _p: PhantomData<(Message, Theme, Renderer)>,
}

impl<'a, Message, Theme, Renderer> HotkeyMatchingInput<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: text_input::Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    /// Create a new HotkeyInput widget with the given keys
    pub fn new(
        hotkeys: &'a HashMap<MaybePhysicalKey, Vec<(keyboard::Modifiers, HotkeyId)>>,
        placeholder: &'a str,
        value: &'a str,
    ) -> Self {
        Self {
            hotkeys,
            hooks: HashMap::new(),
            text_input: TextInput::<'a, Message, Theme, Renderer>::new(placeholder, value),
            on_match: None,
            _p: PhantomData,
        }
    }

    /// Set the callback for when a hotkey is captured
    pub fn on_match(mut self, f: impl Fn(HotkeyId) -> Message + 'static) -> Self {
        self.on_match = Some(Box::new(f));
        self
    }

    pub fn font(mut self, font: Renderer::Font) -> Self {
        self.text_input = self.text_input.font(font);
        self
    }

    pub fn id(mut self, id: Id) -> Self {
        self.text_input = self.text_input.id(id);
        self
    }

    pub fn on_input(mut self, f: impl Fn(String) -> Message + 'static) -> Self {
        self.text_input = self.text_input.on_input(f);
        self
    }

    pub fn on_key_pressed(mut self, key: keyboard::Key, f: Message) -> Self {
        self.hooks.insert(key, f);
        self
    }

    pub fn on_submit(mut self, f: Message) -> Self {
        self.text_input = self.text_input.on_submit(f);
        self
    }

    pub fn style(
        mut self,
        style: impl Fn(&Theme, iced::widget::text_input::Status) -> iced::widget::text_input::Style + 'a,
    ) -> Self
    where
        Theme::Class<'a>: From<text_input::StyleFn<'a, Theme>>,
    {
        self.text_input = self.text_input.style(style);
        self
    }

    pub fn width(mut self, width: iced::Length) -> Self {
        self.text_input = self.text_input.width(width);
        self
    }

    fn check_hotkey(
        &self,
        key: &keyboard::Key,
        physical_key: &key::Physical,
        modifiers: &keyboard::Modifiers,
    ) -> Option<HotkeyId> {
        // Create a MaybePhysicalKey from the incoming key for lookup
        let maybe_key = MaybePhysicalKey::Key(key.clone());
        let maybe_physical_key = MaybePhysicalKey::Physical(physical_key.clone());

        if let Some(modifier_entries) = self.hotkeys.get(&maybe_key) {
            for (required_modifiers, hotkey_id) in modifier_entries {
                if modifiers == required_modifiers {
                    return Some(hotkey_id.clone());
                }
            }
        }
        if let Some(modifier_entries) = self.hotkeys.get(&maybe_physical_key) {
            for (required_modifiers, hotkey_id) in modifier_entries {
                if modifiers == required_modifiers {
                    return Some(hotkey_id.clone());
                }
            }
        }
        None
    }
}

#[derive(Default)]
struct State {}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for HotkeyMatchingInput<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: text_input::Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    fn children(&self) -> Vec<tree::Tree> {
        vec![Tree::new(
            &self.text_input as &dyn Widget<Message, Theme, Renderer>,
        )]
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn diff(&self, _tree: &mut Tree) {}

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> iced::Size<iced::Length> {
        Widget::<Message, Theme, Renderer>::size(&self.text_input)
    }

    fn size_hint(&self) -> iced::Size<iced::Length> {
        Widget::<Message, Theme, Renderer>::size_hint(&self.text_input)
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        Widget::<Message, Theme, Renderer>::layout(
            &self.text_input,
            &mut tree.children[0],
            renderer,
            limits,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        Widget::<Message, Theme, Renderer>::draw(
            &self.text_input,
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        )
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &Renderer,
    ) -> iced::advanced::mouse::Interaction {
        self.text_input.mouse_interaction(
            tree.children.get(0).unwrap(),
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
        let is_focused = {
            let text_input_state = tree.children[0]
                .state
                .downcast_ref::<text_input::State<Renderer::Paragraph>>();

            text_input_state.is_focused()
        };

        if is_focused {
            if let iced::Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                ..
            }) = event
            {
                if let Some(hotkey_id) = self.check_hotkey(&key, &physical_key, modifiers).as_ref()
                {
                    if let Some(on_match) = self.on_match.as_ref() {
                        shell.publish(on_match(hotkey_id.clone()));
                    }
                    shell.capture_event();
                    return;
                }

                if let Some(hook) = self.hooks.get(&key) {
                    if modifiers.is_empty() {
                        shell.publish(hook.clone());
                        shell.capture_event();
                        return;
                    }
                }
            }
        }

        self.text_input.update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.text_input
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
}
