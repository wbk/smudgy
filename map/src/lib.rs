pub mod backends;
pub mod error;
pub mod mapper;

use derive_more::{Add, Display, From, Into};
// Re-export core types
pub use error::{MapError, MapResult};
pub use backends::{MapperBackend, CloudMapper};
pub use mapper::Mapper;

// Re-export data structures that match the backend API
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

/// Exit direction enum matching the backend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display)]
pub enum ExitDirection {
    North,
    East,
    South,
    West,
    Up,
    Down,
    Northeast,
    Northwest,
    Southeast,
    Southwest,
    In,
    Out,
    Special,
    #[default]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShapeType {
    #[default]
    Rectangle,
    RoundedRectangle
}

/// Horizontal alignment enum for labels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HorizontalAlignment {
    Left,
    #[default]
    Center,
    Right,
}

/// Vertical alignment enum for labels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VerticalAlignment {
    Top,
    #[default]
    Center,
    Bottom,
}

/// Share type enum for permissions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShareType {
    Read,
    Write,
    Owner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, Display, Copy)]
#[serde(transparent)]
pub struct AreaId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, Display, Copy)]
#[serde(transparent)]
pub struct ExitId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, Display, Copy)]
#[serde(transparent)]
pub struct AtlasId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, Display, Copy)]
#[serde(transparent)]
pub struct LabelId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, Display, Copy)]
#[serde(transparent)]
pub struct ShapeId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord, Copy, Display, Add, From, Into, Default)]
#[serde(transparent)]
pub struct RoomNumber(pub i32);
/// Atlas model for grouping areas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Atlas {
    pub id: AtlasId,
    pub user_id: Option<Uuid>,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// Area model (formerly Map)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Area {
    pub id: AreaId,
    pub user_id: Option<Uuid>,
    pub atlas_id: Option<AtlasId>,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub rev: i64,
}

/// Complete area with all associated data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AreaWithDetails {
    #[serde(flatten)]
    pub area: Area,
    pub properties: Vec<Property>,
    pub rooms: Vec<RoomWithDetails>,
    pub labels: Vec<Label>,
    pub shapes: Vec<Shape>,
}

/// Room within an area
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub area_id: AreaId,
    pub room_number: RoomNumber,
    pub title: String,
    pub description: String,
    pub level: i32,
    pub x: f32,
    pub y: f32,
    pub color: String,
    pub created_at: DateTime<Utc>,
}

/// Room with all associated data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomWithDetails {
    pub room_number: RoomNumber,
    pub title: String,
    pub description: String,
    pub level: i32,
    pub x: f32,
    pub y: f32,
    pub color: String,
    pub properties: Vec<Property>,
    pub exits: Vec<Exit>,
}

/// Exit connecting rooms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exit {
    pub id: ExitId,
    pub from_direction: ExitDirection,
    pub to_area_id: Option<AreaId>,
    pub to_room_number: Option<RoomNumber>,
    pub to_direction: Option<ExitDirection>,
    pub path: Option<String>,
    pub is_hidden: bool,
    pub is_closed: bool,
    pub is_locked: bool,
    pub weight: f32,
    pub command: Option<String>,
}

/// Text label on area
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: LabelId,
    pub level: i32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub horizontal_alignment: HorizontalAlignment,
    pub vertical_alignment: VerticalAlignment,
    pub text: String,
    pub color: String,
    pub background_color: String,
    pub font_size: i32,
    pub font_weight: i32,
}

/// Graphical shape on area
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shape {
    pub id: ShapeId,
    pub level: i32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub background_color: Option<String>,
    pub stroke_color: Option<String>,
    pub shape_type: ShapeType,
    pub border_radius: f32,
    pub stroke_width: f32,
}

/// Simple property key-value pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    pub name: String,
    pub value: String,
}

/// Room creation/update data
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RoomUpdates {
    pub title: Option<String>,
    pub description: Option<String>,
    pub level: Option<i32>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub color: Option<String>,
}

/// Exit creation/update data
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ExitArgs {
    pub from_direction: ExitDirection,
    pub to_area_id: Option<AreaId>,
    pub to_room_number: Option<RoomNumber>,
    pub to_direction: Option<ExitDirection>,
    pub path: Option<String>,
    pub is_hidden: bool,
    pub is_closed: bool,
    pub is_locked: bool,
    pub weight: f32,
    pub command: Option<String>,
}

/// Exit creation/update data
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ExitUpdates {
    pub from_direction: Option<ExitDirection>,
    pub to_area_id: Option<AreaId>,
    pub to_room_number: Option<RoomNumber>,
    pub to_direction: Option<ExitDirection>,
    pub path: Option<String>,
    pub is_hidden: Option<bool>,
    pub is_closed: Option<bool>,
    pub is_locked: Option<bool>,
    pub weight: Option<f32>,
    pub command: Option<String>,
}

impl ExitUpdates {
    pub fn apply(self, exit: &Exit) -> Exit {
        Exit {
            id: exit.id.clone(),
            from_direction: self.from_direction.unwrap_or(exit.from_direction.clone()),
            to_area_id: self.to_area_id.clone(),
            to_room_number: self.to_room_number.clone(),
            to_direction: self.to_direction.clone(),
            path: self.path.clone(),
            is_hidden: self.is_hidden.unwrap_or(exit.is_hidden),
            is_closed: self.is_closed.unwrap_or(exit.is_closed),
            is_locked: self.is_locked.unwrap_or(exit.is_locked),
            weight: self.weight.unwrap_or(exit.weight),
            command: self.command.clone(),
        }
    }
}
/// Label creation/update data
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LabelArgs {
    pub level: i32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub horizontal_alignment: HorizontalAlignment,
    pub vertical_alignment: VerticalAlignment,
    pub text: String,
    pub color: String,
    pub background_color: Option<String>,
    pub font_size: i32,
    pub font_weight: i32,
}

/// Label creation/update data
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LabelUpdates {
    pub level: Option<i32>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub horizontal_alignment: Option<HorizontalAlignment>,
    pub vertical_alignment: Option<VerticalAlignment>,
    pub text: Option<String>,
    pub color: Option<String>,
    pub background_color: Option<String>,
    pub font_size: Option<i32>,
    pub font_weight: Option<i32>,
}

/// Shape creation/update data
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ShapeArgs {
    pub level: i32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub background_color: Option<String>,
    pub stroke_color: Option<String>,
    pub shape_type: ShapeType,
    pub border_radius: f32,
    pub stroke_width: Option<f32>,
}

/// Shape creation/update data
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ShapeUpdates {
    pub level: Option<i32>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub background_color: Option<String>,
    pub stroke_color: Option<String>,
    pub shape_type: Option<ShapeType>,
    pub border_radius: Option<f32>,
    pub stroke_width: Option<f32>,
}

/// Area creation data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAreaRequest {
    pub name: String,
    pub atlas_id: Option<AtlasId>,
}

/// Area update data
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AreaUpdates {
    pub name: Option<String>,
    pub atlas_id: Option<Option<AtlasId>>,
}

// This is meant to represent a doubly or singly connected exit pair of exits between two rooms
// for use by the map view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomConnector {
    pub from_room: Room,
    pub from: Exit,
    pub to_room: Option<Room>,
    pub to: Option<Exit>,
}