use std::sync::Arc;

use crate::assets::fonts::GEIST_MONO_VF;
use crate::components::Update;
use crate::components::map_view::{self, MapView};
use crate::theme::{self, Element as ThemedElement};
use iced::border::Radius;
use iced::widget::canvas::{Stroke, stroke};
use iced::widget::{canvas, column, container, scrollable, stack, text};
use iced::{Color, Length, Rectangle, Renderer};
use iced::{Size, Vector, mouse};
use smudgy_map::mapper::RoomKey;
use smudgy_map::{AreaId, AreaWithDetails, Mapper, Uuid};

#[derive(Debug, Clone)]
pub enum Message {
    MapViewMessage(map_view::Message),
    SetCurrentLocation(AreaId, Option<i32>),
}

#[derive(Debug, Clone)]
pub enum Event {
    None,
}

pub struct MapEditorWindow {
    mapper: Mapper,
    map_view: MapView,
    hovered_room: Option<RoomKey>,
    area_id: AreaId,
}

impl MapEditorWindow {
    pub fn new(mapper: Mapper) -> Self {
        let area_id = mapper
            .get_current_atlas()
            .areas()
            .next()
            .map(|area| area.get_id().clone())
            .unwrap_or_else(|| AreaId(Uuid::nil()));

        Self {
            map_view: MapView::new(mapper.clone(), area_id),
            mapper,
            hovered_room: None,
            area_id,
        }
    }
    pub fn update(&mut self, message: Message) -> Update<Message, Event> {
        match message {
            Message::MapViewMessage(message) => {
                let update = self
                    .map_view
                    .update(message)
                    .map_message(Message::MapViewMessage);

                if let Some(event) = update.event {
                    match event {
                        map_view::Event::HoveredRoomChanged(room_key) => {
                            self.hovered_room = room_key;
                            Update::with_task(update.task)
                        }
                    }
                } else {
                    Update::none()
                }
            }
            Message::SetCurrentLocation(area_id, room_number) => {
                self.area_id = area_id;
                self.map_view
                    .update(map_view::Message::SetPlayerLocation(area_id, room_number));
                Update::none()
            }
        }
    }

    pub fn view(&self) -> ThemedElement<Message> {
        let mut text_col = column![];

        if let Some(area_name) = self.mapper.get_current_atlas().get_area(&self.area_id) {
            if let Some(ref room_key) = self.hovered_room {
                let room = self.mapper.get_current_atlas().get_room(room_key);
                if let Some(room) = room {
                    text_col = text_col.push(
                        text(format!(
                            "{} - Room #{}",
                            area_name.get_name(),
                            room.get_room_number()
                        ))
                        .size(16)
                        .font(GEIST_MONO_VF)
                        .color(iced::Color::from_rgb8(170, 170, 0)),
                    );
                    text_col = text_col.push(
                        text(room.get_title().to_string())
                            .size(16)
                            .color(iced::Color::from_rgb8(0, 170, 170))
                            .font(GEIST_MONO_VF),
                    );
                    text_col = text_col.push(
                        text(room.get_description().to_string())
                            .size(16)
                            .font(GEIST_MONO_VF),
                    );
                }
            } else {
                let area_name = area_name.get_name().to_string();
                text_col = text_col.push(text(area_name).size(16));
            }

            let text_container = container(text_col)
                .width(Length::Shrink)
                .height(Length::Shrink)
                .style(|_| container::Style {
                    background: Some(Color::from_rgba8(0, 0, 0, 0.8).into()),
                    ..Default::default()
                });

            stack![
                self.map_view.view().map(Message::MapViewMessage),
                text_container
            ]
            .into()
        } else {
            text("No area selected").into()
        }
    }
}
