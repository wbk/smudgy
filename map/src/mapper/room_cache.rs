use std::collections::HashMap;

use crate::{Exit, ExitId, RoomNumber, RoomUpdates, RoomWithDetails};

/// Room with all associated data
#[derive(Debug, Clone, Default)]

pub struct RoomCache {
    room_number: RoomNumber,
    title: String,
    description: String,
    title_and_description: String,
    level: i32,
    x: f32,
    y: f32,
    color: String,
    properties: HashMap<String, String>,
    exits: Vec<Exit>,
}

impl RoomCache {
    pub fn new(room_number: RoomNumber) -> Self {
        Self {
            room_number,
            ..Default::default()
        }
    }

    pub fn get_room_number(&self) -> RoomNumber {
        self.room_number
    }

    pub fn get_title_and_description(&self) -> &str {
        &self.title_and_description
    }

    pub fn get_title(&self) -> &str {
        &self.title
    }

    pub fn get_description(&self) -> &str {
        &self.description
    }

    pub fn get_level(&self) -> i32 {
        self.level
    }

    pub fn get_x(&self) -> f32 {
        self.x
    }

    pub fn get_y(&self) -> f32 {
        self.y
    }

    pub fn get_color(&self) -> &str {
        &self.color
    }

    pub fn get_exits(&self) -> &[Exit] {
        &self.exits
    }

    pub fn get_property(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(|p| p.as_str())
    }

    pub fn set_property(&self, name: String, value: String) -> Self {
        let mut new_properties = self.properties.clone();
        new_properties.insert(name, value);

        Self {
            properties: new_properties,
            ..self.clone()
        }
    }

    pub fn delete_property(&self, name: &str) -> Self {
        let mut new_properties = self.properties.clone();
        new_properties.remove(name);

        Self {
            properties: new_properties,
            ..self.clone()
        }
    }

    pub fn upsert_exit(&self, exit: Exit) -> Self {
        let mut new_exits = self.exits.clone();
        new_exits.retain(|e| e.id != exit.id);
        new_exits.push(exit);

        Self {
            exits: new_exits,
            ..self.clone()
        }
    }

    pub fn delete_exit(&self, exit_id: ExitId) -> Self {
        let mut new_exits = self.exits.clone();
        new_exits.retain(|e| e.id != exit_id);

        Self {
            exits: new_exits,
            ..self.clone()
        }
    }

    pub fn apply_updates(&self, updates: RoomUpdates) -> Self {
        let mut new_room = self.clone();

        if let Some(title) = &updates.title {
            new_room.title = title.clone();
        }
        if let Some(description) = &updates.description {
            new_room.description = description.clone();
        }

        if updates.title.is_some() || updates.description.is_some() {
            new_room.title_and_description = format!(
                "{}\r\n{}",
                new_room.title,
                new_room.description
            );
        }

        if let Some(x) = updates.x {
            new_room.x = x;
        }
        if let Some(y) = updates.y {
            new_room.y = y;
        }
        if let Some(level) = updates.level {
            new_room.level = level;
        }
        if let Some(color) = updates.color {
            new_room.color = color;
        }

        new_room
    }
}

impl From<RoomWithDetails> for RoomCache {
    fn from(room: RoomWithDetails) -> Self {
        Self {
            room_number: room.room_number,
            title_and_description: format!("{}\r\n{}", room.title, room.description),
            title: room.title,
            description: room.description,
            level: room.level,
            x: room.x,
            y: room.y,
            color: room.color,
            properties: room
                .properties
                .into_iter()
                .map(|p| (p.name, p.value))
                .collect(),
            exits: room.exits,
        }
    }
}
