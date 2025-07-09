use std::sync::{Arc, Weak};

use crate::{AreaId, ExitDirection, ExitId, RoomNumber, mapper::room_cache::RoomCache};

#[derive(Debug, Clone)]
pub struct RoomConnection {
    pub from_level: i32,
    pub from_x: f32,
    pub from_y: f32,
    pub from_direction: ExitDirection,
    pub room: Arc<RoomCache>,
    pub to: RoomConnectionEnd,
    pub is_bidirectional: bool,
}

#[derive(Debug, Clone)]
pub enum RoomConnectionEnd {
    External {
        area_id: AreaId,
    },
    ToLevel {
        level: i32,
        direction: ExitDirection,
        x: f32,
        y: f32,
        room: Arc<RoomCache>,
    },
    Normal {
        direction: ExitDirection,
        x: f32,
        y: f32,
        room: Arc<RoomCache>,
    },
}
