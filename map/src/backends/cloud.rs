use async_trait::async_trait;
use log::info;
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

use crate::{
    mapper::RoomKey, Area, AreaId, AreaUpdates, AreaWithDetails, CreateAreaRequest, Exit, ExitArgs, ExitId, ExitUpdates, Label, LabelArgs, LabelId, LabelUpdates, MapError, MapResult, Room, RoomUpdates, Shape, ShapeArgs, ShapeId, ShapeUpdates
};
use super::MapperBackend;

/// HTTP client for the cloud-based map API
#[derive(Debug)]
pub struct CloudMapper {
    client: Client,
    base_url: String,
    api_key: String,
}

impl CloudMapper {
    /// Create a new CloudMapper instance
    pub fn new(base_url: String, api_key: String) -> Self {
        let client = Client::new();
        
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }
    
    /// Helper method to get authorization header
    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }
    
    /// Helper method to make GET requests
    async fn get<T>(&self, path: &str) -> MapResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url, path);
        
        info!("GET {}", url);

        let response = self
            .client
            .get(&url)
            .header("authorization", self.auth_header())
            .header("content-type", "application/json")
            .send()
            .await?;
            
        if response.status().is_success() {
            let json: serde_json::Value = response.json().await?;
            
            // Extract data field from API response
            if let Some(data) = json.get("data") {
                let result: T = serde_json::from_value(data.clone())?;
                Ok(result)
            } else {
                Err(MapError::SerializationError("Missing data field in response".to_string()))
            }
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(MapError::NetworkError(format!("HTTP {}: {}", status, error_text)))
        }
    }
    
    /// Helper method to make POST requests
    async fn post<T, B>(&self, path: &str, body: &B) -> MapResult<T>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        let url = format!("{}{}", self.base_url, path);
        
        info!("POST {}", url);
        info!("Body: {:?}", serde_json::to_string_pretty(body));
        
        let response = self
            .client
            .post(&url)
            .header("authorization", self.auth_header())
            .header("content-type", "application/json")
            .json(body)
            .send()
            .await?;
            
        if response.status().is_success() {
            let json: serde_json::Value = response.json().await?;
            
            // Extract data field from API response
            if let Some(data) = json.get("data") {
                let result: T = serde_json::from_value(data.clone())?;
                Ok(result)
            } else {
                Err(MapError::SerializationError("Missing data field in response".to_string()))
            }
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(MapError::NetworkError(format!("HTTP {}: {}", status, error_text)))
        }
    }
    
    /// Helper method to make PUT requests
    async fn put<T, B>(&self, path: &str, body: &B) -> MapResult<T>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        let url = format!("{}{}", self.base_url, path);
        
        info!("PUT {}", url);
        info!("Body: {:?}", serde_json::to_string_pretty(body));
        
        let response = self
            .client
            .put(&url)
            .header("authorization", self.auth_header())
            .header("content-type", "application/json")
            .json(body)
            .send()
            .await?;
            
        if response.status().is_success() {
            let json: serde_json::Value = response.json().await?;
            
            // Extract data field from API response
            if let Some(data) = json.get("data") {
                let result: T = serde_json::from_value(data.clone())?;
                Ok(result)
            } else {
                Err(MapError::SerializationError("Missing data field in response".to_string()))
            }
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(MapError::NetworkError(format!("HTTP {}: {}", status, error_text)))
        }
    }
    
    /// Helper method to make PUT requests without expecting response data
    async fn put_no_response<B>(&self, path: &str, body: &B) -> MapResult<()>
    where
        B: serde::Serialize,
    {
        let url = format!("{}{}", self.base_url, path);
        
        info!("PUT {}", url);
        info!("Body: {:?}", serde_json::to_string_pretty(body));
        
        let response = self
            .client
            .put(&url)
            .header("authorization", self.auth_header())
            .header("content-type", "application/json")
            .json(body)
            .send()
            .await?;
            
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(MapError::NetworkError(format!("HTTP {}: {}", status, error_text)))
        }
    }
    
    /// Helper method to make DELETE requests
    async fn delete(&self, path: &str) -> MapResult<()> {
        let url = format!("{}{}", self.base_url, path);
        
        info!("DELETE {}", url);

        let response = self
            .client
            .delete(&url)
            .header("authorization", self.auth_header())
            .send()
            .await?;
            
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(MapError::NetworkError(format!("HTTP {}: {}", status, error_text)))
        }
    }
}

#[async_trait]
impl MapperBackend for CloudMapper {
    // ===== AREA OPERATIONS =====
    
    async fn create_area(&self, request: CreateAreaRequest) -> MapResult<Area> {
        self.post("/areas", &request).await
    }
    
    async fn list_areas(&self) -> MapResult<Vec<Area>> {
        self.get("/areas").await
    }
    
    async fn get_area(&self, area_id: &AreaId) -> MapResult<AreaWithDetails> {
        self.get(&format!("/areas/{}", area_id)).await
    }
    
    async fn update_area(&self, area_id: &AreaId, updates: AreaUpdates) -> MapResult<()> {
        self.put_no_response(&format!("/areas/{}", area_id), &updates).await
    }
    
    async fn delete_area(&self, area_id: &AreaId) -> MapResult<()> {
        self.delete(&format!("/areas/{}", area_id)).await
    }
    
    // ===== AREA PROPERTIES =====
    
    async fn set_area_property(&self, area_id: &AreaId, name: &str, value: &str) -> MapResult<()> {
        let body = json!({ "value": value });
        self.put_no_response(&format!("/areas/{}/properties/{}", area_id, name), &body).await
    }
    
    async fn delete_area_property(&self, area_id: &AreaId, name: &str) -> MapResult<()> {
        self.delete(&format!("/areas/{}/properties/{}", area_id, name)).await
    }
    
    // ===== ROOM OPERATIONS =====
    
    async fn update_room(&self, room_key: &RoomKey, updates: RoomUpdates) -> MapResult<Room> {
        self.put(&format!("/areas/{}/{}", room_key.area_id, room_key.room_number), &updates).await
    }
    
    async fn delete_room(&self, room_key: &RoomKey) -> MapResult<()> {
        self.delete(&format!("/areas/{}/rooms/{}", room_key.area_id, room_key.room_number)).await
    }
    
    // ===== ROOM PROPERTIES =====
    
    async fn set_room_property(&self, room_key: &RoomKey, name: &str, value: &str) -> MapResult<()> {
        let body = json!({ "value": value });
        self.put_no_response(&format!("/areas/{}/rooms/{}/properties/{}", room_key.area_id, room_key.room_number, name), &body).await
    }
    
    async fn delete_room_property(&self, room_key: &RoomKey, name: &str) -> MapResult<()> {
        self.delete(&format!("/areas/{}/rooms/{}/properties/{}", room_key.area_id, room_key.room_number, name)).await
    }
    
    // ===== EXIT OPERATIONS =====
    
    async fn create_room_exit(&self, room_key: &RoomKey, exit_data: ExitArgs) -> MapResult<Exit> {
        self.post(&format!("/areas/{}/rooms/{}/exits", room_key.area_id, room_key.room_number), &exit_data).await
    }
    
    async fn update_exit(&self, area_id: &AreaId, exit_id: &ExitId, updates: ExitUpdates) -> MapResult<()> {
        self.put_no_response(&format!("/areas/{}/exits/{}", area_id, exit_id), &updates).await
    }
    
    async fn delete_exit(&self, area_id: &AreaId, exit_id: &ExitId) -> MapResult<()> {
        self.delete(&format!("/areas/{}/exits/{}", area_id, exit_id)).await
    }
    
    // ===== LABEL OPERATIONS =====
    
    async fn create_label(&self, area_id: &AreaId, label_data: LabelArgs) -> MapResult<Label> {
        self.post(&format!("/areas/{}/labels", area_id), &label_data).await
    }
    
    async fn update_label(&self, area_id: &AreaId, label_id: &LabelId, updates: LabelUpdates) -> MapResult<()> {
        self.put_no_response(&format!("/areas/{}/labels/{}", area_id, label_id), &updates).await
    }
    
    async fn delete_label(&self, area_id: &AreaId, label_id: &LabelId) -> MapResult<()> {
        self.delete(&format!("/areas/{}/labels/{}", area_id, label_id)).await
    }
    
    // ===== SHAPE OPERATIONS =====
    
    async fn create_shape(&self, area_id: &AreaId, shape_data: ShapeArgs) -> MapResult<Shape> {
        self.post(&format!("/areas/{}/shapes", area_id), &shape_data).await
    }
    
    async fn update_shape(&self, area_id: &AreaId, shape_id: &ShapeId, updates: ShapeUpdates) -> MapResult<()> {
        self.put_no_response(&format!("/areas/{}/shapes/{}", area_id, shape_id), &updates).await
    }
    
    async fn delete_shape(&self, area_id: &AreaId, shape_id: &ShapeId) -> MapResult<()> {
        self.delete(&format!("/areas/{}/shapes/{}", area_id, shape_id)).await
    }
} 