use std::fmt;

use crate::{mapper::RoomKey, AreaId, ExitId, LabelId, ShapeId};

/// Result type alias for map operations
pub type MapResult<T> = Result<T, MapError>;

/// Error types for map operations
#[derive(Debug, Clone)]
pub enum MapError {
    /// Area not found
    AreaNotFound(AreaId),
    
    /// Room not found
    RoomNotFound (RoomKey),
    
    /// Exit not found
    ExitNotFound (ExitId),
    
    /// Label not found
    LabelNotFound (LabelId),
    
    /// Shape not found
    ShapeNotFound (ShapeId),
    
    /// Property not found
    PropertyNotFound {
        entity_type: String,
        entity_id: String,
        property_name: String,
    },
    
    /// Invalid input data
    InvalidInput(String),
    
    /// Database error
    DatabaseError(String),
    
    /// Network/HTTP error
    NetworkError(String),
    
    /// Serialization error
    SerializationError(String),
    
    /// Authentication error
    AuthenticationError(String),
    
    /// Permission denied
    PermissionDenied(String),
    
    /// Internal error
    InternalError(String),

    /// PendingOperations
    PendingOperations(String)
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::AreaNotFound(id) => write!(f, "Area not found: {}", id),
            MapError::RoomNotFound(room_key)=> {
                write!(f, "Room {} not found in area {}", room_key.room_number, room_key.area_id)
            }
            MapError::ExitNotFound(id) => write!(f, "Exit not found: {}", id),
            MapError::LabelNotFound(id) => write!(f, "Label not found: {}", id),
            MapError::ShapeNotFound(id) => write!(f, "Shape not found: {}", id),
            MapError::PropertyNotFound { entity_type, entity_id, property_name } => {
                write!(f, "Property '{}' not found on {} {}", property_name, entity_type, entity_id)
            }
            MapError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            MapError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            MapError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            MapError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            MapError::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            MapError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            MapError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            MapError::PendingOperations(msg) => write!(f, "Pending operations: {}", msg),
        }
    }
}

impl std::error::Error for MapError {}

// Conversion from common error types
impl From<serde_json::Error> for MapError {
    fn from(err: serde_json::Error) -> Self {
        MapError::SerializationError(err.to_string())
    }
}

impl From<reqwest::Error> for MapError {
    fn from(err: reqwest::Error) -> Self {
        MapError::NetworkError(err.to_string())
    }
}

// Note: sqlx conversion removed to avoid dependency conflicts with workspace
// Will be added back when LocalMapper is implemented with proper dependency resolution

impl From<uuid::Error> for MapError {
    fn from(err: uuid::Error) -> Self {
        MapError::InvalidInput(format!("Invalid UUID: {}", err))
    }
} 