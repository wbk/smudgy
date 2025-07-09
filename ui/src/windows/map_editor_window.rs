use std::sync::Arc;

use crate::components::Update;
use crate::components::map_view::{self, MapView};
use crate::theme::Element as ThemedElement;
use iced::border::Radius;
use iced::widget::canvas::{Stroke, stroke};
use iced::widget::{canvas, column, scrollable, text};
use iced::{Color, Rectangle, Renderer};
use iced::{Size, Vector, mouse};
use smudgy_map::{AreaId, AreaWithDetails, Mapper, Uuid};

#[derive(Debug, Clone)]
pub enum Message {
    MapViewMessage(map_view::Message),
    SelectArea(AreaId),
}

#[derive(Debug, Clone)]
pub enum Event {
    None,
}

pub struct MapEditorWindow {
    mapper: Mapper,
    map_view: MapView,
}

impl MapEditorWindow {
    pub fn new(mapper: Mapper) -> Self {
        let area_id = mapper.get_current_atlas().areas().next().map(|area| area.get_id().clone()).unwrap_or_else(|| AreaId(Uuid::nil()));

        Self {
            map_view: MapView::new(mapper.clone(), area_id),
            mapper,
        }
    }
    pub fn update(&mut self, message: Message) -> Update<Message, Event> {
        match message {
            Message::MapViewMessage(message) => {
                let update = self.map_view.update(message).map_message(Message::MapViewMessage);

                if let Some(event) = update.event {
                    match event {
                        map_view::Event::None => {
                            Update::with_task(update.task)
                        },
                    }
                } else {
                    Update::none()
                }
            }
            Message::SelectArea(area_id) => {
                self.map_view.update(map_view::Message::SwitchArea(area_id));
                Update::none()
            }
        }
    }

    pub fn view(&self) -> ThemedElement<Message> {
        self.map_view.view().map(Message::MapViewMessage)
    }
}
