use std::{iter, sync::Arc};

use iced::{
    Alignment, Color, Length, Point, Rectangle, Size, Vector,
    alignment::{Horizontal, Vertical},
    mouse, touch,
    widget::{
        Canvas,
        canvas::{self, stroke},
    },
};
use smudgy_map::{
    AreaId, AreaWithDetails, ExitDirection, Mapper, Room, RoomWithDetails,
    mapper::room_connection::RoomConnectionEnd,
};

use crate::{Renderer, Theme, components::Update, theme::Element};
use iced::event::Event as IcedEvent;

pub struct MapView {
    mapper: Mapper,
    area_id: AreaId,
    level: i32,
    scaling: f32,
    translation: Vector,
    show_area_name: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    SwitchArea(AreaId),
    Translated(Vector),
    Scaled(f32, Option<Vector>),
}

#[derive(Debug, Clone)]
pub enum Event {
    None,
}

#[derive(Debug, Clone)]
struct Region {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl MapView {
    const MAP_ROOM_SIZE: f32 = 0.5;
    const MAP_ROOM_SIZE_AS_SIZE: Size = Size::new(Self::MAP_ROOM_SIZE, Self::MAP_ROOM_SIZE);
    const MAP_ROOM_BORDER_RADIUS: f32 = Self::MAP_ROOM_SIZE * 0.2;
    const MIN_SCALING: f32 = 5.0;
    const MAX_SCALING: f32 = 200.0;
    const MAP_EXIT_STUB_LENGTH: f32 = 0.5;

    pub fn new(mapper: Mapper, area_id: AreaId) -> Self {
        Self {
            mapper,
            area_id,
            level: 0,
            scaling: 20.0,
            translation: Vector::new(0.0, 0.0),
            show_area_name: true,
        }
    }

    pub fn update(&mut self, message: Message) -> Update<Message, Event> {
        match message {
            Message::SwitchArea(area_id) => {
                self.area_id = area_id;
                self.level = 0;
                self.scaling = 20.0;
                self.translation = Vector::new(0.0, 0.0);
                Update::none()
            }
            Message::Translated(translation) => {
                self.translation = translation;
                Update::none()
            }
            Message::Scaled(scaling, translation) => {
                self.scaling = scaling;

                if let Some(translation) = translation {
                    self.translation = translation;
                }

                Update::none()
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    #[inline]
    fn visible_region(&self, size: Size) -> Region {
        let width = size.width / self.scaling;
        let height = size.height / self.scaling;

        Region {
            x: -self.translation.x - width / 2.0,
            y: -self.translation.y - height / 2.0,
            width,
            height,
        }
    }

    #[inline]
    fn project(&self, position: Point, size: Size) -> Point {
        let region = self.visible_region(size);

        Point::new(
            position.x / self.scaling + region.x,
            position.y / self.scaling + region.y,
        )
    }
}

#[derive(Debug, Clone, Default)]
pub enum Interaction {
    #[default]
    None,
    Panning {
        translation: Vector,
        start: Point,
    },
}

impl canvas::Program<Message, crate::theme::Theme> for MapView {
    type State = Interaction;

    fn update(
        &self,
        interaction: &mut Interaction,
        event: &IcedEvent,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let IcedEvent::Mouse(mouse::Event::ButtonReleased(_)) = event {
            *interaction = Interaction::None;
        }

        let cursor_position = cursor.position_in(bounds)?;

        match event {
            IcedEvent::Mouse(mouse_event) => match mouse_event {
                mouse::Event::ButtonPressed(button) => {
                    let message = match button {
                        mouse::Button::Right => {
                            *interaction = Interaction::Panning {
                                translation: self.translation,
                                start: cursor_position,
                            };

                            None
                        }
                        _ => None,
                    };

                    Some(
                        message
                            .map(canvas::Action::publish)
                            .unwrap_or(canvas::Action::request_redraw())
                            .and_capture(),
                    )
                }
                mouse::Event::CursorMoved { .. } => {
                    let message = match *interaction {
                        Interaction::Panning { translation, start } => Some(Message::Translated(
                            translation + (cursor_position - start) * (1.0 / self.scaling),
                        )),
                        Interaction::None => None,
                    };

                    let action = message
                        .map(canvas::Action::publish)
                        .unwrap_or(canvas::Action::request_redraw());

                    Some(match interaction {
                        Interaction::None => action,
                        _ => action.and_capture(),
                    })
                }
                mouse::Event::WheelScrolled { delta } => match *delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        if y < 0.0 && self.scaling > Self::MIN_SCALING
                            || y > 0.0 && self.scaling < Self::MAX_SCALING
                        {
                            let old_scaling = self.scaling;

                            let scaling = (self.scaling * (1.0 + y / 10.0))
                                .clamp(Self::MIN_SCALING, Self::MAX_SCALING);

                            let translation = if let Some(cursor_to_center) =
                                cursor.position_from(bounds.center())
                            {
                                let factor = scaling - old_scaling;

                                Some(
                                    self.translation
                                        - Vector::new(
                                            cursor_to_center.x * factor
                                                / (old_scaling * old_scaling),
                                            cursor_to_center.y * factor
                                                / (old_scaling * old_scaling),
                                        ),
                                )
                            } else {
                                None
                            };

                            Some(
                                canvas::Action::publish(Message::Scaled(scaling, translation))
                                    .and_capture(),
                            )
                        } else {
                            Some(canvas::Action::capture())
                        }
                    }
                },
                _ => None,
            },
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Interaction,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let atlas = self.mapper.get_current_atlas();

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

        if let Some(area) = atlas.get_area(&self.area_id) {
            if self.show_area_name {
                let area_name_text = canvas::Text {
                    content: area.get_name().to_string(),
                    align_x: iced::advanced::text::Alignment::Left,
                    align_y: Vertical::Top,
                    color: Color::WHITE,
                    size: 16.0.into(),
                    line_height: 16.0.into(),
                    position: Point::new(20.0, bounds.height),
                    ..Default::default()
                };
                frame.fill_text(area_name_text);
            }

            frame.with_save(|frame| {
                frame.translate(center);
                frame.scale(self.scaling);
                frame.translate(self.translation);
                frame.scale(1.0);

                let region = self.visible_region(bounds.size());

                // draw a grid of dots
                for x in ((region.x.floor() as i32)..((region.x + region.width).ceil() as i32))
                    .step_by(1)
                {
                    for y in ((region.y.floor() as i32)..((region.y + region.height).ceil() as i32))
                        .step_by(1)
                    {
                        let circle = canvas::Path::circle(
                            Point {
                                x: x as f32,
                                y: y as f32,
                            },
                            0.1,
                        );
                        frame.fill(&circle, Color::from_rgba8(255, 255, 255, 0.05));
                    }
                }

                // Draw rooms
                for room in area.get_rooms() {
                    let circle = canvas::Path::rounded_rectangle(
                        Point {
                            x: room.get_x() as f32 - Self::MAP_ROOM_SIZE / 2.0,
                            y: room.get_y() as f32 - Self::MAP_ROOM_SIZE / 2.0,
                        },
                        Self::MAP_ROOM_SIZE_AS_SIZE,
                        Self::MAP_ROOM_BORDER_RADIUS.into(),
                    );

                    frame.fill(&circle, Color::from_rgb8(192, 192, 192));
                }

                // Draw exits

                for connection in area.get_room_connections() {
                    match connection.to {
                        RoomConnectionEnd::Normal {
                            ref direction,
                            x,
                            y,
                            ref room,
                        } => {
                            let from_stub = match connection.from_direction {
                                ExitDirection::North => Some((
                                    Point {
                                        x: connection.from_x,
                                        y: connection.from_y,
                                    },
                                    Point {
                                        x: connection.from_x,
                                        y: connection.from_y - Self::MAP_EXIT_STUB_LENGTH,
                                    },
                                )),
                                ExitDirection::East => Some((
                                    Point {
                                        x: connection.from_x,
                                        y: connection.from_y,
                                    },
                                    Point {
                                        x: connection.from_x + Self::MAP_EXIT_STUB_LENGTH,
                                        y: connection.from_y,
                                    },
                                )),
                                ExitDirection::South => Some((
                                    Point {
                                        x: connection.from_x,
                                        y: connection.from_y,
                                    },
                                    Point {
                                        x: connection.from_x,
                                        y: connection.from_y + Self::MAP_EXIT_STUB_LENGTH,
                                    },
                                )),
                                ExitDirection::West => Some((
                                    Point {
                                        x: connection.from_x,
                                        y: connection.from_y,
                                    },
                                    Point {
                                        x: connection.from_x - Self::MAP_EXIT_STUB_LENGTH,
                                        y: connection.from_y,
                                    },
                                )),
                                _ => None,
                            };

                            let to_stub = match direction {
                                ExitDirection::North => Some((
                                    Point { x: x, y: y },
                                    Point {
                                        x: x,
                                        y: y - Self::MAP_EXIT_STUB_LENGTH,
                                    },
                                )),
                                ExitDirection::East => Some((
                                    Point { x: x, y: y },
                                    Point {
                                        x: x + Self::MAP_EXIT_STUB_LENGTH,
                                        y: y,
                                    },
                                )),
                                ExitDirection::South => Some((
                                    Point { x: x, y: y },
                                    Point {
                                        x: x,
                                        y: y + Self::MAP_EXIT_STUB_LENGTH,
                                    },
                                )),
                                ExitDirection::West => Some((
                                    Point { x: x, y: y },
                                    Point {
                                        x: x - Self::MAP_EXIT_STUB_LENGTH,
                                        y: y,
                                    },
                                )),
                                _ => None,
                            };

                            let connection = match (from_stub, to_stub) {
                                (Some(from_stub), Some(to_stub)) => Some((from_stub.1, to_stub.1)),
                                (Some(from_stub), None) => {
                                    Some((from_stub.1, Point { x: x, y: y }))
                                }
                                (None, Some(to_stub)) => Some((
                                    Point {
                                        x: connection.from_x,
                                        y: connection.from_y,
                                    },
                                    to_stub.1,
                                )),
                                (None, None) => Some((
                                    Point {
                                        x: connection.from_x,
                                        y: connection.from_y,
                                    },
                                    Point { x: x, y: y },
                                )),
                            };

                            if let Some((from, to)) = from_stub {
                                let path = canvas::Path::line(from, to);
                                frame.stroke(
                                    &path,
                                    stroke::Stroke {
                                        style: stroke::Style::Solid(Color::from_rgb8(
                                            192, 192, 192,
                                        )),
                                        width: 1.0.into(),
                                        ..Default::default()
                                    },
                                );
                            }

                            if let Some((from, to)) = to_stub {
                                let path = canvas::Path::line(from, to);
                                frame.stroke(
                                    &path,
                                    stroke::Stroke {
                                        style: stroke::Style::Solid(Color::from_rgb8(
                                            192, 192, 192,
                                        )),
                                        width: 1.0.into(),
                                        ..Default::default()
                                    },
                                );
                            }
                            if let Some((from, to)) = connection {
                                let path = canvas::Path::line(from, to);
                                frame.stroke(
                                    &path,
                                    stroke::Stroke {
                                        style: stroke::Style::Solid(Color::from_rgb8(
                                            192, 192, 192,
                                        )),
                                        width: 1.0.into(),
                                        ..Default::default()
                                    },
                                );
                            }
                        }
                        RoomConnectionEnd::ToLevel { .. } => {}
                        RoomConnectionEnd::External { .. } => {}
                    }
                }
            });
        }

        // draw dots for each integral coordinate in the area

        // Then, we produce the geometry
        vec![frame.into_geometry()]
    }
}
