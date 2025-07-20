use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
    sync::Arc,
};

use crate::{
    AreaId, AreaWithDetails, Exit, ExitId, ExitUpdates, Label, LabelId, MapError, MapResult, Room,
    RoomNumber, RoomUpdates, Shape, ShapeId,
    mapper::{
        RoomKey,
        room_cache::RoomCache,
        room_connection::{RoomConnection, RoomConnectionEnd},
    },
};

#[derive(Debug, Clone)]
pub struct AreaCache {
    id: AreaId,
    name: String,
    rev: i64,
    rooms_by_number: HashMap<RoomNumber, Arc<RoomCache>>,
    rooms: Vec<Arc<RoomCache>>,
    room_connections: Vec<RoomConnection>,
    properties: HashMap<String, String>,
    labels: Vec<Label>,
    shapes: Vec<Shape>,
    max_room_number: RoomNumber,
}

impl AreaCache {
    pub(super) fn new_with_area(area: AreaWithDetails) -> Self {
        let max_room_number = area
            .rooms
            .iter()
            .map(|r| r.room_number)
            .max()
            .unwrap_or(RoomNumber(0));

        let rooms: Vec<Arc<RoomCache>> = area
            .rooms
            .iter()
            .map(|r| Arc::new(r.clone().into()))
            .collect();
        let rooms_by_number = rooms
            .iter()
            .map(|r| (r.get_room_number(), r.clone()))
            .collect();
        let properties = area
            .properties
            .iter()
            .map(|p| (p.name.clone(), p.value.clone()))
            .collect();

        let room_connections = Self::build_room_connections(&area.area.id, &rooms_by_number);

        Self {
            id: area.area.id,
            name: area.area.name,
            rev: area.area.rev,
            rooms,
            rooms_by_number,
            max_room_number,
            properties,
            labels: area.labels,
            shapes: area.shapes,
            room_connections,
        }
    }

    fn build_room_connections(
        area_id: &AreaId,
        rooms_by_number: &HashMap<RoomNumber, Arc<RoomCache>>,
    ) -> Vec<RoomConnection> {
        let mut skip_exit_ids = HashSet::new();

        let mut room_connections = Vec::new();

        for room in rooms_by_number.values() {
            for from_exit in room.get_exits() {
                if skip_exit_ids.contains(&from_exit.id) {
                    continue;
                }

                // let's see if we have a matching exit coming back
                let paired_room: Option<&Arc<RoomCache>> =
                    if from_exit.to_area_id.as_ref() == Some(area_id) {
                        from_exit
                            .to_room_number
                            .and_then(|ref n| rooms_by_number.get(n))
                    } else {
                        // if the exit is in a different area, let's say it isn't bidirectional for simplicy's sake
                        // (this field is meant primarily for the graphical mapper, which will only show one area at a time)
                        None
                    };

                let mut is_bidirectional = false;

                paired_room.map(|paired_room| {
                    for paired_exit in paired_room.get_exits() {
                        if paired_exit.to_area_id.as_ref() == Some(area_id)
                            && paired_exit.to_room_number == Some(room.get_room_number())
                            && paired_exit.to_direction == Some(from_exit.from_direction)
                            && Some(paired_exit.from_direction) == from_exit.to_direction
                        {
                            is_bidirectional = true;
                            skip_exit_ids.insert(paired_exit.id);
                        }
                    }
                });

                if let Some(paired_room) = paired_room {
                    if paired_room.get_level() == room.get_level() {
                        room_connections.push(RoomConnection {
                            from_level: room.get_level(),
                            from_x: room.get_x(),
                            from_y: room.get_y(),
                            from_direction: from_exit.from_direction,
                            room: room.clone(),
                            is_bidirectional,
                            to: RoomConnectionEnd::Normal {
                                x: paired_room.get_x(),
                                y: paired_room.get_y(),
                                direction: from_exit.to_direction.unwrap_or_default(),
                                room: paired_room.clone(),
                            },
                        });
                    } else {
                        room_connections.push(RoomConnection {
                            from_level: room.get_level(),
                            from_x: room.get_x(),
                            from_y: room.get_y(),
                            from_direction: from_exit.from_direction,
                            room: room.clone(),
                            is_bidirectional,
                            to: RoomConnectionEnd::ToLevel {
                                level: paired_room.get_level(),
                                x: paired_room.get_x(),
                                y: paired_room.get_y(),
                                direction: from_exit.to_direction.unwrap_or_default(),
                                room: paired_room.clone(),
                            },
                        });
                        room_connections.push(RoomConnection {
                            from_level: paired_room.get_level(),
                            from_x: paired_room.get_x(),
                            from_y: paired_room.get_y(),
                            from_direction: from_exit.to_direction.unwrap_or_default(),
                            room: paired_room.clone(),
                            is_bidirectional,
                            to: RoomConnectionEnd::ToLevel {
                                level: room.get_level(),
                                x: room.get_x(),
                                y: room.get_y(),
                                direction: from_exit.from_direction,
                                room: room.clone(),
                            },
                        });
                    }
                } else if let Some(area_id) = from_exit.to_area_id {
                    room_connections.push(RoomConnection {
                        from_level: room.get_level(),
                        from_x: room.get_x(),
                        from_y: room.get_y(),
                        from_direction: from_exit.from_direction,
                        room: room.clone(),
                        is_bidirectional,
                        to: RoomConnectionEnd::External {
                            area_id,
                        },
                    });
                } else {
                    room_connections.push(RoomConnection {
                        from_level: room.get_level(),
                        from_x: room.get_x(),
                        from_y: room.get_y(),
                        from_direction: from_exit.from_direction,
                        room: room.clone(),
                        is_bidirectional,
                        to: RoomConnectionEnd::None,
                    });
                }
            }
        }

        room_connections
    }

    pub fn get_id(&self) -> &AreaId {
        &self.id
    }

    pub fn get_room(&self, room_number: &RoomNumber) -> Option<&Arc<RoomCache>> {
        self.rooms_by_number.get(room_number)
    }

    pub fn get_rooms(&self) -> &[Arc<RoomCache>] {
        &self.rooms
    }

    pub fn get_name(&self) -> &str {
        self.name.as_str()
    }

    #[must_use]
    pub(super) fn rename(&self, name: &str) -> Self {
        Self {
            name: name.to_string(),
            rev: self.rev + 1,
            ..self.clone()
        }
    }

    pub fn get_property(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(|p| p.as_str())
    }

    pub(super) fn set_property(&self, name: String, value: String) -> Self {
        let mut new_properties = self.properties.clone();
        new_properties.insert(name, value);

        Self {
            properties: new_properties,
            rev: self.rev + 1,
            ..self.clone()
        }
    }

    pub(super) fn delete_property(&self, name: &str) -> Self {
        let mut new_properties = self.properties.clone();
        new_properties.remove(name);

        Self {
            properties: new_properties,
            rev: self.rev + 1,
            ..self.clone()
        }
    }

    pub(super) fn upsert_room(&self, room_number: RoomNumber, updates: RoomUpdates) -> Self {
        let room = if let Some(room) = self.rooms_by_number.get(&room_number) {
            Arc::new(room.apply_updates(updates))
        } else {
            Arc::new(RoomCache::new(room_number).apply_updates(updates))
        };

        self.upsert_room_cache(room_number, room)
    }

    fn upsert_room_cache(&self, room_number: RoomNumber, room: Arc<RoomCache>) -> Self {
        let mut new_rooms_by_number = self.rooms_by_number.clone();
        let mut new_rooms = self.rooms.clone();

        new_rooms_by_number.insert(room_number, room.clone());
        new_rooms.retain(|r| r.get_room_number() != room_number);
        new_rooms.push(room);
        let max_room_number = RoomNumber(room_number.0.max(self.max_room_number.0));

        Self {
            rev: self.rev + 1,
            room_connections: Self::build_room_connections(&self.id, &new_rooms_by_number),
            rooms_by_number: new_rooms_by_number,
            rooms: new_rooms,
            max_room_number,
            ..self.clone()
        }
    }

    pub(super) fn delete_room(&self, room_number: RoomNumber) -> Self {
        let mut new_rooms_by_number = self.rooms_by_number.clone();
        new_rooms_by_number.remove(&room_number);
        let mut new_rooms = self.rooms.clone();
        new_rooms.retain(|r| r.get_room_number() != room_number);

        let max_room_number = if self.max_room_number == room_number {
            new_rooms
                .iter()
                .map(|r| r.get_room_number())
                .max()
                .unwrap_or(RoomNumber(0))
        } else {
            self.max_room_number
        };

        Self {
            rev: self.rev + 1,
            room_connections: Self::build_room_connections(&self.id, &new_rooms_by_number),
            rooms_by_number: new_rooms_by_number,
            rooms: new_rooms,
            max_room_number,
            ..self.clone()
        }
    }

    pub(super) fn set_room_property(
        &self,
        room_number: RoomNumber,
        name: String,
        value: String,
    ) -> MapResult<Self> {
        let room = self.rooms_by_number.get(&room_number);

        if let Some(room) = room {
            let room = room.set_property(name, value);
            return Ok(self.upsert_room_cache(room_number, Arc::new(room)));
        } else {
            return Err(MapError::RoomNotFound(RoomKey {
                area_id: self.id.clone(),
                room_number,
            }));
        }
    }

    pub(super) fn delete_room_property(
        &self,
        room_number: RoomNumber,
        name: &str,
    ) -> MapResult<Self> {
        let room = self.rooms_by_number.get(&room_number);

        if let Some(room) = room {
            let room = room.delete_property(name);
            return Ok(self.upsert_room_cache(room_number, Arc::new(room)));
        } else {
            return Err(MapError::RoomNotFound(RoomKey {
                area_id: self.id.clone(),
                room_number,
            }));
        }
    }

    pub(super) fn upsert_exit(&self, room_number: RoomNumber, exit: Exit) -> MapResult<Self> {
        let room = self.rooms_by_number.get(&room_number);

        if let Some(room) = room {
            let room = room.upsert_exit(exit);
            return Ok(self.upsert_room_cache(room_number, Arc::new(room)));
        } else {
            return Err(MapError::RoomNotFound(RoomKey {
                area_id: self.id.clone(),
                room_number,
            }));
        }
    }

    pub(super) fn delete_exit(&self, room_number: RoomNumber, exit_id: ExitId) -> MapResult<Self> {
        let room = self.rooms_by_number.get(&room_number);

        if let Some(room) = room {
            let room = room.delete_exit(exit_id);
            return Ok(self.upsert_room_cache(room_number, Arc::new(room)));
        } else {
            return Err(MapError::RoomNotFound(RoomKey {
                area_id: self.id.clone(),
                room_number,
            }));
        }
    }

    pub(super) fn upsert_label(&self, label_id: LabelId, label: Label) -> Self {
        let mut new_labels = self.labels.clone();
        new_labels.retain(|l| l.id != label_id);
        new_labels.push(label);

        Self {
            rev: self.rev + 1,
            labels: new_labels,
            ..self.clone()
        }
    }

    pub(super) fn delete_label(&self, label_id: LabelId) -> Self {
        let mut new_labels = self.labels.clone();
        new_labels.retain(|l| l.id != label_id);
        Self {
            rev: self.rev + 1,
            labels: new_labels,
            ..self.clone()
        }
    }

    pub(super) fn upsert_shape(&self, shape_id: ShapeId, shape: Shape) -> Self {
        let mut new_shapes = self.shapes.clone();
        new_shapes.retain(|s| s.id != shape_id);
        new_shapes.push(shape);
        Self {
            rev: self.rev + 1,
            shapes: new_shapes,
            ..self.clone()
        }
    }

    pub(super) fn delete_shape(&self, shape_id: ShapeId) -> Self {
        let mut new_shapes = self.shapes.clone();
        new_shapes.retain(|s| s.id != shape_id);
        Self {
            rev: self.rev + 1,
            shapes: new_shapes,
            ..self.clone()
        }
    }

    pub fn get_max_room_number(&self) -> RoomNumber {
        self.max_room_number
    }

    pub fn get_room_connections(&self) -> &[RoomConnection] {
        &self.room_connections
    }
}
