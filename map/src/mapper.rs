use crate::backends::MapperBackend;
use crate::error::MapResult;
use crate::mapper::area_cache::AreaCache;
use crate::{
    Area, AreaId, AreaUpdates, AreaWithDetails, Atlas, AtlasId, CreateAreaRequest, Exit, ExitArgs,
    ExitId, ExitUpdates, LabelArgs, LabelId, LabelUpdates, MapError, Room, RoomNumber, RoomUpdates,
    ShapeArgs, ShapeId, ShapeUpdates,
};

use arc_swap::{ArcSwap};
use log::warn;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::task::JoinHandle;
use uuid::Uuid;

pub mod area_cache;
pub mod atlas_cache;
pub mod room_cache;
pub mod room_connection;

pub use atlas_cache::AtlasCache;

/// Composite key for room lookups
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoomKey {
    pub area_id: AreaId,
    pub room_number: RoomNumber,
}

impl RoomKey {
    #[must_use]
    pub fn new(area_id: AreaId, room_number: RoomNumber) -> Self {
        Self {
            area_id,
            room_number,
        }
    }
}

#[derive(Debug)]
enum AreaSyncOperation {
    RenameArea(AreaId, String),
    DeleteArea(AreaId),
    SetAreaProperty(AreaId, String, String),
    DeleteAreaProperty(AreaId, String),
    UpdateRoom(RoomKey, RoomUpdates),
    DeleteRoom(RoomKey),
    SetRoomProperty(RoomKey, String, String),
    DeleteRoomProperty(RoomKey, String),
    UpdateExit(AreaId, ExitId, ExitUpdates),
    DeleteExit(AreaId, ExitId),
    UpdateLabel(AreaId, LabelId, LabelUpdates),
    DeleteLabel(AreaId, LabelId),
    UpdateShape(AreaId, ShapeId, ShapeUpdates),
    DeleteShape(AreaId, ShapeId),
}

/// Sync statistics for diagnostics
#[derive(Debug, Default)]
pub struct SyncStats {
    pub operations_sent: AtomicU64,
    pub operations_succeeded: AtomicU64,
    pub operations_failed: AtomicU64,
}

impl SyncStats {
    #[must_use]
    pub fn operations_sent(&self) -> u64 {
        self.operations_sent.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn operations_succeeded(&self) -> u64 {
        self.operations_succeeded.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn operations_failed(&self) -> u64 {
        self.operations_failed.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn pending_operations(&self) -> u64 {
        self.operations_sent() - self.operations_succeeded() - self.operations_failed()
    }
}

#[derive(Clone)]
pub struct Mapper {
    inner: Arc<Inner>,
}
pub struct Inner {
    atlas_id: ArcSwap<Option<AtlasId>>,
    atlas_cache: ArcSwap<AtlasCache>,

    backend: Arc<dyn MapperBackend + Send + Sync>,

    // Background sync channel
    sync_sender: tokio::sync::mpsc::UnboundedSender<AreaSyncOperation>,

    // Sync diagnostics
    sync_stats: Arc<SyncStats>,
}

impl std::fmt::Debug for Mapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Mapper]")
    }
}


impl Mapper {
    pub fn new(backend: Arc<dyn MapperBackend + Send + Sync>) -> Self {
        let (sync_sender, sync_receiver) = tokio::sync::mpsc::unbounded_channel();

        let cache = AtlasCache::new_with_areas(HashMap::new());

        let inner = Inner {
            atlas_id: ArcSwap::from_pointee(None),
            atlas_cache: ArcSwap::from_pointee(cache.clone()),
            backend: backend.clone(),
            sync_sender: sync_sender,
            sync_stats: Arc::new(SyncStats::default()),
        };

        inner.spawn_sync_task(sync_receiver, inner.sync_stats.clone());

        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn get_current_atlas(&self) -> Arc<AtlasCache> {
        self.inner.get_current_atlas()
    }

    pub fn create_area(&self, name: String) -> impl Future<Output = MapResult<AreaId>> {
        self.inner.create_area(name)
    }

    pub fn load_all_areas(&self) -> impl Future<Output = MapResult<()>> {
        self.inner.load_all_areas()
    }

    pub fn rename_area(&self, area_id: AreaId, name: &str) {
        self.inner.rename_area(area_id, name);
    }

    pub fn set_area_property(&self, area_id: AreaId, name: String, value: String) {
        self.inner.set_area_property(area_id, name, value);
    }

    pub fn delete_area_property(&self, area_id: AreaId, name: String) {
        self.inner.delete_area_property(area_id, name);
    }

    pub fn upsert_room(&self, room_key: RoomKey, updates: RoomUpdates) {
        self.inner.upsert_room(room_key, updates);
    }

    pub fn delete_room(&self, room_key: RoomKey) {
        self.inner.delete_room(room_key);
    }

    pub fn set_room_property(&self, room_key: RoomKey, name: String, value: String) {
        self.inner.set_room_property(room_key, name, value);
    }

    pub fn delete_room_property(&self, room_key: RoomKey, name: String) {
        self.inner.delete_room_property(room_key, name);
    }

    pub fn update_exit(&self, room_key: RoomKey, exit_id: ExitId, updates: ExitUpdates) {
        self.inner.update_exit(room_key, exit_id, updates);
    }

    pub fn delete_exit(&self, room_key: RoomKey, exit_id: ExitId) {
        self.inner.delete_exit(room_key, exit_id);
    }

    pub fn create_exit(&self, room_key: RoomKey, args: ExitArgs) -> impl Future<Output = MapResult<ExitId>> {
        self.inner.create_exit(room_key, args)
    }

    pub fn create_label(&self, area_id: AreaId, args: LabelArgs) -> impl Future<Output = MapResult<LabelId>> {
        self.inner.create_label(area_id, args)
    }

    pub fn create_shape(&self, area_id: AreaId, args: ShapeArgs) -> impl Future<Output = MapResult<ShapeId>> {
        self.inner.create_shape(area_id, args)
    }

    pub fn wait_for_sync_completion(&self, timeout_secs: u64) -> impl Future<Output = Result<bool, ()>> {
        self.inner.wait_for_sync_completion(timeout_secs)
    }

    pub fn get_sync_stats(&self) -> &SyncStats {
        self.inner.sync_stats()
    }
}


impl Inner {
    /// Create a new shared cache with the given backend

    /// Get sync statistics for diagnostics
    #[must_use]
    pub fn sync_stats(&self) -> &SyncStats {
        &self.sync_stats
    }

    /// Wait for all sync operations to complete
    ///
    /// # Arguments
    /// * `timeout_secs` - Maximum time to wait in seconds (0 = no timeout)
    ///
    /// # Returns
    /// * `Ok(true)` if all operations completed successfully
    /// * `Ok(false)` if timeout was reached with pending operations
    /// * `Err(())` if there were failed operations
    pub async fn wait_for_sync_completion(&self, timeout_secs: u64) -> Result<bool, ()> {
        let start_time = std::time::Instant::now();

        loop {
            let stats = &self.sync_stats;
            let pending = stats.pending_operations();
            let failed = stats.operations_failed();

            // Check if we're done
            if pending == 0 {
                return if failed > 0 { Err(()) } else { Ok(true) };
            }

            // Check for timeout
            if timeout_secs > 0 && start_time.elapsed().as_secs() >= timeout_secs {
                return Ok(false);
            }

            // Short sleep to avoid busy waiting
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    /// Load all areas from backend into cache
    /// # Errors
    /// Returns error if backend operations fail
    pub async fn load_all_areas(&self) -> MapResult<()> {
        let areas = self.backend.list_areas().await?;

        let mut new_cache = HashMap::with_capacity(areas.len());

        for area in areas {
            // Load detailed area data
            if let Ok(details) = self.backend.get_area(&area.id).await {
                let cache = Arc::new(AreaCache::new_with_area(details));
                new_cache.insert(area.id, cache);
            }
        }

        let new_cache = Arc::new(AtlasCache::new_with_areas(new_cache));

        self.atlas_cache.store(new_cache);

        Ok(())
    }

    // === READ OPERATIONS (Instant, Lock-Free) ===

    #[must_use]
    pub fn get_current_atlas(&self) -> Arc<AtlasCache> {
        self.atlas_cache.load().clone()
    }

    /// Create a new area (waits for backend to assign ID)
    pub async fn create_area(&self, name: String) -> MapResult<AreaId> {
        let atlas_id = Option::<&AtlasId>::cloned(self.atlas_id.load().as_ref().as_ref());

        let request = CreateAreaRequest { name, atlas_id };

        // Create area on backend first to get the real ID
        let backend_area = self.backend.create_area(request).await?;
        let area_id = backend_area.id.clone();

        self.atlas_cache.rcu(|cache| {
            Arc::new(cache.add_area(
                area_id.clone(),
                Arc::new(AreaCache::new_with_area(AreaWithDetails {
                    area: backend_area.clone(),
                    properties: vec![],
                    rooms: vec![],
                    labels: vec![],
                    shapes: vec![],
                })),
            ))
        });

        Ok(area_id)
    }

    pub fn delete_area(&self, area_id: AreaId) {
        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&area_id)
                .map(|_area| {
                    Arc::new(cache.delete_area(area_id.clone()))
                })
                .unwrap_or_else(|| cache.clone())
        });
        self.send_sync_operation(AreaSyncOperation::DeleteArea(area_id));
    }

    pub fn rename_area(&self, area_id: AreaId, name: &str) {
        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&area_id)
                .map(|area| {
                    Arc::new(cache.insert_area(area_id.clone(), Arc::new(area.rename(name))))
                })
                .unwrap_or_else(|| cache.clone())
        });
        self.send_sync_operation(AreaSyncOperation::RenameArea(area_id, name.to_string()));
    }

    pub fn set_area_property(&self, area_id: AreaId, name: String, value: String) {
        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&area_id)
                .map(|area| {
                    Arc::new(cache.insert_area(
                        area_id.clone(),
                        Arc::new(area.set_property(name.clone(), value.clone())),
                    ))
                })
                .unwrap_or_else(|| cache.clone())
        });

        self.send_sync_operation(AreaSyncOperation::SetAreaProperty(area_id, name, value));
    }

    pub fn delete_area_property(&self, area_id: AreaId, name: String) {
        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&area_id)
                .map(|area| {
                    Arc::new(cache.insert_area(
                        area_id.clone(),
                        Arc::new(area.delete_property(name.as_str())),
                    ))
                })
                .unwrap_or_else(|| cache.clone())
        });

        self.send_sync_operation(AreaSyncOperation::DeleteAreaProperty(area_id, name));
    }

    pub fn upsert_room(&self, room_key: RoomKey, updates: RoomUpdates) {
        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&room_key.area_id)
                .map(|area| {
                    Arc::new(cache.insert_area(
                        area.get_id().clone(),
                        Arc::new(area.upsert_room(room_key.room_number, updates.clone())),
                    ))
                })
                .unwrap_or_else(|| cache.clone())
        });

        self.send_sync_operation(AreaSyncOperation::UpdateRoom(room_key, updates));
    }

    pub fn delete_room(&self, room_key: RoomKey) {
        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&room_key.area_id)
                .map(|area| {
                    Arc::new(cache.insert_area(
                        area.get_id().clone(),
                        Arc::new(area.delete_room(room_key.room_number)),
                    ))
                })
                .unwrap_or_else(|| cache.clone())
        });

        self.send_sync_operation(AreaSyncOperation::DeleteRoom(room_key));
    }

    pub fn set_room_property(&self, room_key: RoomKey, name: String, value: String) {
        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&room_key.area_id)
                .map(|area| {
                    area.set_room_property(room_key.room_number, name.clone(), value.clone())
                        .ok()
                })
                .flatten()
                .map(|area| Arc::new(cache.insert_area(area.get_id().clone(), Arc::new(area))))
                .unwrap_or_else(|| cache.clone())
        });


        self.send_sync_operation(AreaSyncOperation::SetRoomProperty(room_key, name, value));
    }

    pub fn delete_room_property(&self, room_key: RoomKey, name: String) {
        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&room_key.area_id)
                .map(|area| {
                    area.delete_room_property(room_key.room_number, name.as_str())
                        .ok()
                })
                .flatten()
                .map(|area| Arc::new(cache.insert_area(area.get_id().clone(), Arc::new(area))))
                .unwrap_or_else(|| cache.clone())
        });


        self.send_sync_operation(AreaSyncOperation::DeleteRoomProperty(room_key, name));
    }

    pub fn update_exit(&self, room_key: RoomKey, exit_id: ExitId, updates: ExitUpdates) {
        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&room_key.area_id)
                .map(|area| {
                    area.get_room(&room_key.room_number)
                        .map(|room| room.get_exits().iter().find(|e| e.id == exit_id))
                        .flatten()
                        .map(|exit| (area.clone(), updates.clone().apply(exit)))
                })
                .flatten()
                .map(|(area, new_exit)| area.upsert_exit(room_key.room_number, new_exit).ok())
                .flatten()
                .map(|area| Arc::new(cache.insert_area(area.get_id().clone(), Arc::new(area))))
                .unwrap_or_else(|| cache.clone())
        });

        self.send_sync_operation(AreaSyncOperation::UpdateExit(
            room_key.area_id,
            exit_id.clone(),
            updates,
        ));
    }

    pub fn delete_exit(&self, room_key: RoomKey, exit_id: ExitId) {
        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&room_key.area_id)
                .map(|area| {
                    area.delete_exit(room_key.room_number, exit_id.clone())
                        .ok()
                })
                .flatten()
                .map(|area| Arc::new(cache.insert_area(area.get_id().clone(), Arc::new(area))))
                .unwrap_or_else(|| cache.clone())
        });

        self.send_sync_operation(AreaSyncOperation::DeleteExit(room_key.area_id, exit_id));
    }
    // === SLOW CREATE OPERATIONS (Wait for Backend ID) ===

    /// Create exit (waits for backend to assign ID)
    /// # Errors
    /// Returns error if backend operations fail
    pub async fn create_exit(&self, room_key: RoomKey, args: ExitArgs) -> MapResult<ExitId> {
        // Create on backend first to get the real ID and data
        let backend_exit = self
            .backend
            .create_room_exit(&room_key, args.clone())
            .await?;
        let exit_id = backend_exit.id.clone();

        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&room_key.area_id)
                .map(|area| {
                    area.upsert_exit(room_key.room_number, backend_exit.clone())
                        .ok()
                })
                .flatten()
                .map(|area| Arc::new(cache.insert_area(area.get_id().clone(), Arc::new(area))))
                .unwrap_or_else(|| cache.clone())
        });

        Ok(exit_id)
    }

    /// Create label (waits for backend to assign ID)
    /// # Errors
    /// Returns error if backend operations fail
    pub async fn create_label(&self, area_id: AreaId, args: LabelArgs) -> MapResult<LabelId> {
        // Create on backend first to get the real ID and data
        let backend_label = self.backend.create_label(&area_id, args).await?;
        let label_id = backend_label.id.clone();

        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&area_id)
                .map(|area| {
                    Arc::new(cache.insert_area(
                        area.get_id().clone(),
                        Arc::new(area.upsert_label(label_id.clone(), backend_label.clone())),
                    ))
                })
                .unwrap_or_else(|| cache.clone())
        });

        Ok(label_id)
    }

    /// Create shape (waits for backend to assign ID)
    /// # Errors
    /// Returns error if backend operations fail
    pub async fn create_shape(&self, area_id: AreaId, args: ShapeArgs) -> MapResult<ShapeId> {
        // Create on backend first to get the real ID and data
        let backend_shape = self.backend.create_shape(&area_id, args).await?;
        let shape_id = backend_shape.id.clone();

        self.atlas_cache.rcu(|cache| {
            cache
                .get_area(&area_id)
                .map(|area| {
                    Arc::new(cache.insert_area(
                        area.get_id().clone(),
                        Arc::new(area.upsert_shape(shape_id.clone(), backend_shape.clone())),
                    ))
                })
                .unwrap_or_else(|| cache.clone())
        });

        Ok(shape_id)
    }

    pub fn get_selected_atlas_id(&self) -> Option<Uuid> {
        None
    }

    // === INTERNAL SYNC HELPERS ===

    /// Send sync operation with tracking
    fn send_sync_operation(&self, operation: AreaSyncOperation) {
        self.sync_stats
            .operations_sent
            .fetch_add(1, Ordering::Relaxed);

        if let Err(e) = self.sync_sender.send(operation) {
            self.sync_stats
                .operations_failed
                .fetch_add(1, Ordering::Relaxed);
            warn!("Failed to send sync operation: {e}");
        }
    }

    // === INDEX MANAGEMENT ===

    /// Spawn background sync task
    fn spawn_sync_task(
        &self,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<AreaSyncOperation>,
        stats: Arc<SyncStats>,
    ) -> JoinHandle<()> {
        let backend = self.backend.clone();

        tokio::spawn(async move {
            while let Some(operation) = receiver.recv().await {
                match Self::handle_sync_operation(&*backend, operation).await {
                    Ok(()) => {
                        stats.operations_succeeded.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        stats.operations_failed.fetch_add(1, Ordering::Relaxed);
                        warn!("Failed to handle sync operation: {e}");
                    }
                }
            }
        })
    }

    /// Handle individual sync operations
    async fn handle_sync_operation(
        backend: &dyn MapperBackend,
        operation: AreaSyncOperation,
    ) -> MapResult<()> {
        match operation {
            AreaSyncOperation::RenameArea(area_id, name) => {
                backend
                    .update_area(
                        &area_id,
                        AreaUpdates {
                            name: Some(name),
                            atlas_id: None,
                        },
                    )
                    .await?;
                Ok(())
            }
            AreaSyncOperation::SetAreaProperty(area_id, name, value) => {
                backend.set_area_property(&area_id, &name, &value).await?;
                Ok(())
            }
            AreaSyncOperation::UpdateRoom(room_key, updates) => {
                backend.update_room(&room_key, updates).await?;
                Ok(())
            }
            AreaSyncOperation::SetRoomProperty(room_key, name, value) => {
                backend.set_room_property(&room_key, &name, &value).await?;
                Ok(())
            }
            AreaSyncOperation::DeleteArea(area_id) => {
                backend.delete_area(&area_id).await?;
                Ok(())
            }
            AreaSyncOperation::DeleteAreaProperty(area_id, name) => {
                backend.delete_area_property(&area_id, &name).await?;
                Ok(())
            }
            AreaSyncOperation::DeleteRoom(room_key) => {
                backend.delete_room(&room_key).await?;
                Ok(())
            }
            AreaSyncOperation::DeleteRoomProperty(room_key, name) => {
                backend.delete_room_property(&room_key, &name).await?;
                Ok(())
            }
            AreaSyncOperation::UpdateExit(area_id, exit_id, updates) => {
                backend.update_exit(&area_id, &exit_id, updates).await?;
                Ok(())
            }
            AreaSyncOperation::DeleteExit(area_id, exit_id) => {
                backend.delete_exit(&area_id, &exit_id).await?;
                Ok(())
            }
            AreaSyncOperation::UpdateLabel(area_id, label_id, updates) => {
                backend.update_label(&area_id, &label_id, updates).await?;
                Ok(())
            }
            AreaSyncOperation::DeleteLabel(area_id, label_id) => {
                backend.delete_label(&area_id, &label_id).await?;
                Ok(())
            }
            AreaSyncOperation::UpdateShape(area_id, shape_id, updates) => {
                backend.update_shape(&area_id, &shape_id, updates).await?;
                Ok(())
            }
            AreaSyncOperation::DeleteShape(area_id, shape_id) => {
                backend.delete_shape(&area_id, &shape_id).await?;
                Ok(())
            }
        }
    }
}
