use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    mapper::RoomKey, Area, AreaId, AreaUpdates, AreaWithDetails, CreateAreaRequest, Exit, ExitArgs, ExitId, ExitUpdates, Label, LabelArgs, LabelId, LabelUpdates, MapResult, Room, RoomUpdates, Shape, ShapeArgs, ShapeId, ShapeUpdates
};

pub mod cloud;

pub use cloud::CloudMapper;

/// Core trait defining all mapping operations
#[async_trait]
pub trait MapperBackend: Send + Sync {
    // ===== AREA OPERATIONS =====
    
    async fn create_area(&self, request: CreateAreaRequest) -> MapResult<Area>;
    
    async fn list_areas(&self) -> MapResult<Vec<Area>>;
    
    async fn get_area(&self, area_id: &AreaId) -> MapResult<AreaWithDetails>;
    
    async fn update_area(&self, area_id: &AreaId, updates: AreaUpdates) -> MapResult<()>;
    
    async fn delete_area(&self, area_id: &AreaId) -> MapResult<()>;
    
    // ===== AREA PROPERTIES =====
    
    async fn set_area_property(&self, area_id: &AreaId, name: &str, value: &str) -> MapResult<()>;
    
    async fn delete_area_property(&self, area_id: &AreaId, name: &str) -> MapResult<()>;
    
    // ===== ROOM OPERATIONS =====
    
    async fn update_room(&self, room_key: &RoomKey, updates: RoomUpdates) -> MapResult<Room>;
    
    async fn delete_room(&self, room_key: &RoomKey) -> MapResult<()>;
    
    // ===== ROOM PROPERTIES =====
    
    async fn set_room_property(&self, room_key: &RoomKey, name: &str, value: &str) -> MapResult<()>;
    
    async fn delete_room_property(&self, room_key: &RoomKey, name: &str) -> MapResult<()>;
    
    // ===== EXIT OPERATIONS =====
    
    async fn create_room_exit(&self, room_key: &RoomKey, exit_data: ExitArgs) -> MapResult<Exit>;
    
    async fn update_exit(&self, area_id: &AreaId, exit_id: &ExitId, updates: ExitUpdates) -> MapResult<()>;
    
    async fn delete_exit(&self, area_id: &AreaId, exit_id: &ExitId) -> MapResult<()>;
    
    // ===== LABEL OPERATIONS =====
    
    async fn create_label(&self, area_id: &AreaId, label_data: LabelArgs) -> MapResult<Label>;
    
    async fn update_label(&self, area_id: &AreaId, label_id: &LabelId, updates: LabelUpdates) -> MapResult<()>;
    
    async fn delete_label(&self, area_id: &AreaId, label_id: &LabelId) -> MapResult<()>;
    
    // ===== SHAPE OPERATIONS =====
    
    async fn create_shape(&self, area_id: &AreaId, shape_data: ShapeArgs) -> MapResult<Shape>;
    
    async fn update_shape(&self, area_id: &AreaId, shape_id: &ShapeId, updates: ShapeUpdates) -> MapResult<()>;
    
    async fn delete_shape(&self, area_id: &AreaId, shape_id: &ShapeId) -> MapResult<()>;
} 