use std::{iter, sync::Arc};

use iced::{
    advanced::text::Alignment, alignment::{Horizontal, Vertical}, mouse, touch, widget::{
        canvas::{self, stroke, LineDash, Stroke}, container, Canvas
    }, Color, Length, Pixels, Point, Rectangle, Size, Vector
};
use smudgy_map::{
    AreaId, AreaWithDetails, ExitDirection, Mapper, Room, RoomNumber, RoomWithDetails,
    mapper::{RoomKey, room_connection::RoomConnectionEnd},
};

use iced_anim::{Animate, Animated, Animation, Event as AnimEvent, spring::Motion};

use crate::{components::Update, theme::{Element}, Renderer, Theme};
use iced::event::Event as IcedEvent;

const EXIT_COLOR: Color = Color::from_rgb8(164, 164, 164);
const AREA_NAME_FONT_COLOR: Color = EXIT_COLOR;
const DEFAULT_ROOM_COLOR: Color = Color::from_rgb8(192, 192, 192);
const EXIT_STROKE: Stroke = stroke::Stroke {
    style: stroke::Style::Solid(EXIT_COLOR),
    width: 1.0,
    line_cap: stroke::LineCap::Butt,
    line_join: stroke::LineJoin::Round,
    line_dash: LineDash {
        segments: &[],
        offset: 0,
    },
};
pub struct MapView {
    mapper: Mapper,
    area_id: AreaId,
    player_location: Option<RoomKey>,
    level: i32,
    scaling: f32,
    translation: iced_anim::Animated<Vector>,

    hovered_room: Option<RoomKey>,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetPlayerLocation(AreaId, Option<i32>),
    Translated(Vector),
    Scaled(f32, Option<Vector>),
    SetHoveredRoom(Option<RoomKey>),
    UpdateTranslation(AnimEvent<Vector>),
}

#[derive(Debug, Clone)]
pub enum Event {
    HoveredRoomChanged(Option<RoomKey>),
}

#[derive(Debug, Clone)]
struct Region {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[inline]
fn draw_arrow_head(
    frame: &mut canvas::Frame,
    from: Vector,
    to: Vector,
    color: Color,
    arrow_head_size: f32,
) {
    frame.with_save(|frame| {
        frame.translate(to);
        frame.rotate((to.y - from.y).atan2(to.x - from.x));
        let mut path = canvas::path::Builder::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(-arrow_head_size, arrow_head_size));
        path.line_to(Point::new(-arrow_head_size, -arrow_head_size));
        path.close();
        let path = path.build();

        frame.fill(&path, color);
    })
}

#[inline]
fn clip_line_end_to_square(line_start: Point, line_end: Point, square_size: f32) -> Point {
    let half_size = square_size / 2.0;

    // Direction vector from start to end
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;

    // If the line has zero length, return the original end point
    if dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
        return line_end;
    }

    // Calculate the intersection with the square boundary
    // The square is centered at line_end
    let left = line_end.x - half_size;
    let right = line_end.x + half_size;
    let top = line_end.y - half_size;
    let bottom = line_end.y + half_size;

    // Find the intersection point on the boundary closest to line_start
    let mut best_t = 1.0; // Start with the original end point

    // Check intersection with left edge (x = left)
    if dx != 0.0 {
        let t = (left - line_start.x) / dx;
        if t > 0.0 && t < best_t {
            let y = line_start.y + t * dy;
            if y >= top && y <= bottom {
                best_t = t;
            }
        }
    }

    // Check intersection with right edge (x = right)
    if dx != 0.0 {
        let t = (right - line_start.x) / dx;
        if t > 0.0 && t < best_t {
            let y = line_start.y + t * dy;
            if y >= top && y <= bottom {
                best_t = t;
            }
        }
    }

    // Check intersection with top edge (y = top)
    if dy != 0.0 {
        let t = (top - line_start.y) / dy;
        if t > 0.0 && t < best_t {
            let x = line_start.x + t * dx;
            if x >= left && x <= right {
                best_t = t;
            }
        }
    }

    // Check intersection with bottom edge (y = bottom)
    if dy != 0.0 {
        let t = (bottom - line_start.y) / dy;
        if t > 0.0 && t < best_t {
            let x = line_start.x + t * dx;
            if x >= left && x <= right {
                best_t = t;
            }
        }
    }

    // Return the intersection point
    Point::new(line_start.x + best_t * dx, line_start.y + best_t * dy)
}

impl MapView {
    const MAP_ROOM_SIZE: f32 = 0.5;
    const MAP_ROOM_SIZE_AS_SIZE: Size = Size::new(Self::MAP_ROOM_SIZE, Self::MAP_ROOM_SIZE);
    const MAP_ROOM_BORDER_RADIUS: f32 = Self::MAP_ROOM_SIZE * 0.2;
    const MIN_SCALING_FOR_MAP_GRID: f32 = 20.0;
    const MIN_SCALING_FOR_MAP_GRID_OPAQUE: f32 = 50.0;
    const MIN_SCALING: f32 = 2.0;
    const MAX_SCALING: f32 = 200.0;
    const MAP_EXIT_STUB_LENGTH: f32 = 0.4;
    const MAP_PLAYER_INDICATOR_RADIUS: f32 = Self::MAP_ROOM_SIZE / 4.0;

    pub fn new(mapper: Mapper, area_id: AreaId) -> Self {
        Self {
            mapper,
            area_id,
            player_location: None,
            level: 0,
            scaling: 40.0,
            hovered_room: None,
            translation: Animated::new(
                Vector::new(0.0, 0.0),
                Motion::default().quick(),
            ),
        }
    }

    fn rooms_at_point(&self, point: &Point, bounds: &Size) -> Box<[RoomKey]> {
        let atlas = self.mapper.get_current_atlas();

        let point = self.project(&point, &bounds);

        atlas
            .get_area(&self.area_id)
            .map(|area| {
                area.get_rooms()
                    .iter()
                    .filter(|room| {
                        room.get_x() as f32 - Self::MAP_ROOM_SIZE / 2.0 < point.x
                            && room.get_x() as f32 + Self::MAP_ROOM_SIZE / 2.0 > point.x
                            && room.get_y() as f32 - Self::MAP_ROOM_SIZE / 2.0 < point.y
                            && room.get_y() as f32 + Self::MAP_ROOM_SIZE / 2.0 > point.y
                    })
                    .map(|room| RoomKey {
                        area_id: self.area_id,
                        room_number: room.get_room_number(),
                    })
                    .collect::<Box<[RoomKey]>>()
            })
            .unwrap_or_default()
    }

    pub fn update(&mut self, message: Message) -> Update<Message, Event> {
        match message {
            Message::UpdateTranslation(event) => {
                self.translation.update(event);
                Update::none()
            }
            Message::SetPlayerLocation(area_id, room_number) => {
                self.area_id = area_id;
                self.level = 0;

                if let Some(room_number) = room_number {
                    let room_key = RoomKey {
                        area_id,
                        room_number: RoomNumber(room_number),
                    };

                    // If the area is the same as the current area, we should center the room on the screen
                    if let Some(room) = self.mapper.get_current_atlas().get_room(&room_key) {
                        self.player_location = Some(room_key);
                        self.translation.set_target(Vector::new(
                            -room.get_x() as f32,
                            -room.get_y() as f32,
                        ));
                        self.level = room.get_level();
                    }
                } else {
                    self.player_location = None;
                }
                Update::none()
            }
            Message::Translated(translation) => {
                self.translation.settle_at(translation);
                Update::none()
            }
            Message::Scaled(scaling, translation) => {
                self.scaling = scaling;

                if let Some(translation) = translation {
                    self.translation.settle_at(translation);
                }

                Update::none()
            }
            Message::SetHoveredRoom(room_key) => {
                self.hovered_room = room_key.clone();
                Update::with_event(Event::HoveredRoomChanged(room_key))
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        Animation::<Vector, Message, Theme, Renderer>::new(
            &self.translation,
            Canvas::new(self).width(Length::Fill).height(Length::Fill),
        ).on_update( Message::UpdateTranslation)
        .into()
    }

    #[inline]
    fn visible_region(&self, size: &Size) -> Region {
        let width = size.width / self.scaling;
        let height = size.height / self.scaling;

        Region {
            x: -self.translation.value().x - width / 2.0,
            y: -self.translation.value().y - height / 2.0,
            width,
            height,
        }
    }

    #[inline]
    fn project(&self, position: &Point, size: &Size) -> Point {
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
                                translation: *self.translation.value(),
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
                        Interaction::None => {
                            let rooms = self.rooms_at_point(&cursor_position, &bounds.size());

                            let room_key = rooms.first().cloned();
                            if room_key != self.hovered_room {
                                Some(Message::SetHoveredRoom(room_key))
                            } else {
                                None
                            }
                        }
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
                                    *self.translation.target()
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

        let player_room_number = self
            .player_location
            .as_ref()
            .and_then(|room_key| (room_key.area_id == self.area_id).then(|| room_key.room_number));

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = Vector::new(bounds.width / 2.0, bounds.height / 2.0);

        if let Some(area) = atlas.get_area(&self.area_id) {
            frame.with_save(|frame| {
                frame.translate(center);
                frame.scale(self.scaling);
                frame.translate(*self.translation.value());
                frame.scale(1.0);

                let region = self.visible_region(&bounds.size());
                // draw a grid of dots
                if self.scaling > Self::MIN_SCALING_FOR_MAP_GRID {
                    let opacity = if self.scaling > Self::MIN_SCALING_FOR_MAP_GRID_OPAQUE {
                        0.05
                    } else {
                        (self.scaling - Self::MIN_SCALING_FOR_MAP_GRID)
                            / (Self::MIN_SCALING_FOR_MAP_GRID_OPAQUE
                                - Self::MIN_SCALING_FOR_MAP_GRID)
                            * 0.05
                    };

                    for x in ((region.x.floor() as i32)..((region.x + region.width).ceil() as i32))
                        .step_by(1)
                    {
                        for y in ((region.y.floor() as i32)
                            ..((region.y + region.height).ceil() as i32))
                            .step_by(1)
                        {
                            let circle = canvas::Path::circle(
                                Point {
                                    x: x as f32,
                                    y: y as f32,
                                },
                                0.1,
                            );
                            frame.fill(&circle, Color::from_rgba8(255, 255, 255, opacity));
                        }
                    }
                }

                // Draw exits

                for connection in area.get_room_connections() {
                    match connection.to {
                        RoomConnectionEnd::Normal {
                            ref direction,
                            x,
                            y,
                            ..
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

                            let conn_line = match (from_stub, to_stub) {
                                (Some(from_stub), _) if !connection.is_bidirectional => {
                                    let start = from_stub.1;
                                    let end = Point { x: x, y: y };
                                    let clipped_end = clip_line_end_to_square(
                                        start,
                                        end,
                                        Self::MAP_ROOM_SIZE * 1.25,
                                    );
                                    Some((start, clipped_end))
                                }
                                (None, _) if !connection.is_bidirectional => {
                                    let start = Point {
                                        x: connection.from_x,
                                        y: connection.from_y,
                                    };
                                    let end = Point { x: x, y: y };
                                    let clipped_end = clip_line_end_to_square(
                                        start,
                                        end,
                                        Self::MAP_ROOM_SIZE * 1.25,
                                    );
                                    Some((start, clipped_end))
                                }
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
                                frame.stroke(&path, EXIT_STROKE);
                            }

                            if let Some((from, to)) = to_stub {
                                let path = canvas::Path::line(from, to);
                                frame.stroke(&path, EXIT_STROKE);
                            }
                            if let Some((from, to)) = conn_line {
                                let path = canvas::Path::line(from, to);
                                frame.stroke(&path, EXIT_STROKE);

                                if !connection.is_bidirectional {
                                    draw_arrow_head(
                                        frame,
                                        Vector {
                                            x: from.x,
                                            y: from.y,
                                        },
                                        Vector { x: to.x, y: to.y },
                                        EXIT_COLOR,
                                        0.1,
                                    );
                                }
                            }
                        }
                        RoomConnectionEnd::ToLevel { .. } => {}
                        RoomConnectionEnd::External { area_id } => {
                            let area_name = atlas
                                .get_area(&area_id)
                                .map(|area| area.get_name().to_string())
                                .unwrap_or("(unknown area)".to_string());

                            let (x, y, text_x, text_y, text_align_x, text_align_y) =
                                match connection.from_direction {
                                    ExitDirection::North | ExitDirection::Up => (
                                        connection.from_x,
                                        connection.from_y - Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_x,
                                        connection.from_y - Self::MAP_EXIT_STUB_LENGTH - 0.1,
                                        Alignment::Center,
                                        Vertical::Bottom,
                                    ),
                                    ExitDirection::East => (
                                        connection.from_x + Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_y,
                                        connection.from_x + Self::MAP_EXIT_STUB_LENGTH + 0.1,
                                        connection.from_y,
                                        Alignment::Left,
                                        Vertical::Center,
                                    ),
                                    ExitDirection::West => (
                                        connection.from_x - Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_y,
                                        connection.from_x - Self::MAP_EXIT_STUB_LENGTH - 0.1,
                                        connection.from_y,
                                        Alignment::Right,
                                        Vertical::Center,
                                    ),
                                    ExitDirection::Northeast => (
                                        connection.from_x + Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_y - Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_x + Self::MAP_EXIT_STUB_LENGTH + 0.1,
                                        connection.from_y - Self::MAP_EXIT_STUB_LENGTH - 0.1,
                                        Alignment::Left,
                                        Vertical::Bottom,
                                    ),
                                    ExitDirection::Southeast => (
                                        connection.from_x + Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_y + Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_x + Self::MAP_EXIT_STUB_LENGTH + 0.1,
                                        connection.from_y + Self::MAP_EXIT_STUB_LENGTH + 0.1,
                                        Alignment::Left,
                                        Vertical::Top,
                                    ),
                                    ExitDirection::Southwest => (
                                        connection.from_x - Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_y + Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_x - Self::MAP_EXIT_STUB_LENGTH - 0.1,
                                        connection.from_y + Self::MAP_EXIT_STUB_LENGTH + 0.1,
                                        Alignment::Right,
                                        Vertical::Top,
                                    ),
                                    ExitDirection::Northwest => (
                                        connection.from_x - Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_y - Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_x - Self::MAP_EXIT_STUB_LENGTH - 0.1,
                                        connection.from_y - Self::MAP_EXIT_STUB_LENGTH - 0.1,
                                        Alignment::Right,
                                        Vertical::Bottom,
                                    ),
                                    _ => (
                                        connection.from_x,
                                        connection.from_y + Self::MAP_EXIT_STUB_LENGTH,
                                        connection.from_x,
                                        connection.from_y + Self::MAP_EXIT_STUB_LENGTH + 0.1,
                                        Alignment::Center,
                                        Vertical::Top,
                                    ),
                                };

                            let path = canvas::Path::line(
                                Point {
                                    x: connection.from_x,
                                    y: connection.from_y,
                                },
                                Point { x: x, y: y },
                            );

                            frame.stroke(&path, EXIT_STROKE);

                            let circle = canvas::Path::circle(Point { x: x, y: y }, 0.075);

                            frame.fill(&circle, EXIT_COLOR);

                            let text = canvas::Text {
                                content: area_name,
                                position: Point {
                                    x: text_x,
                                    y: text_y,
                                },
                                align_x: text_align_x,
                                align_y: text_align_y,
                                color: AREA_NAME_FONT_COLOR,
                                size: 0.375.into(),
                                ..Default::default()
                            };

                            frame.fill_text(text);
                        }
                        RoomConnectionEnd::None => {
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

                            if let Some((from, to)) = from_stub {
                                let path = canvas::Path::line(from, to);
                                frame.stroke(&path, EXIT_STROKE);
                            }
                        }
                    }
                }

                // Draw rooms
                for room in area.get_rooms() {
                    let room_shape = canvas::Path::rounded_rectangle(
                        Point {
                            x: room.get_x() as f32 - Self::MAP_ROOM_SIZE / 2.0,
                            y: room.get_y() as f32 - Self::MAP_ROOM_SIZE / 2.0,
                        },
                        Self::MAP_ROOM_SIZE_AS_SIZE,
                        Self::MAP_ROOM_BORDER_RADIUS.into(),
                    );

                    frame.fill(&room_shape, DEFAULT_ROOM_COLOR);
                    frame.stroke(
                        &room_shape,
                        stroke::Stroke {
                            style: stroke::Style::Solid(Color::from_rgba8(0, 0, 0, 0.1)),
                            width: 2.0.into(),
                            line_cap: stroke::LineCap::Butt,
                            line_join: stroke::LineJoin::Round,
                            line_dash: LineDash {
                                segments: &[],
                                offset: 0,
                            },
                        },
                    );
                }

                // Draw player indicator
                if let Some(player_room_number) = player_room_number {
                    if let Some(room) = area.get_room(&player_room_number) {
                        let circle = canvas::Path::circle(
                            Point {
                                x: room.get_x() as f32,
                                y: room.get_y() as f32,
                            },
                            Self::MAP_PLAYER_INDICATOR_RADIUS,
                        );
                        frame.fill(&circle, Color::from_rgb8(0, 0, 255));
                    }
                }
            });
        }

        // draw dots for each integral coordinate in the area

        // Then, we produce the geometry
        vec![frame.into_geometry()]
    }
}
