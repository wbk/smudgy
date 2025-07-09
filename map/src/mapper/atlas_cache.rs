use std::{collections::HashMap, sync::Arc};

use crate::{mapper::{area_cache::AreaCache, room_cache::RoomCache, RoomKey}, AreaId};

static EMPTY_ROOMS_LOOKUP_VEC: Vec<(AreaId, Arc<RoomCache>)> = Vec::new();
#[derive(Clone)]
pub struct AtlasCache {
    areas: HashMap<AreaId, Arc<AreaCache>>,
    rooms_by_title_and_description: HashMap<String, Vec<(AreaId, Arc<RoomCache>)>>,
    rooms_by_title: HashMap<String, Vec<(AreaId, Arc<RoomCache>)>>,
    rooms_by_description: HashMap<String, Vec<(AreaId, Arc<RoomCache>)>>,
    rooms: HashMap<RoomKey, Arc<RoomCache>>,
}

impl AtlasCache {
    pub(super) fn new_with_areas(areas: HashMap<AreaId, Arc<AreaCache>>) -> Self {
        let rooms_by_title_and_description = Self::build_rooms_by_title_and_description(&areas);
        let rooms_by_title = Self::build_rooms_by_title(&areas);
        let rooms_by_description = Self::build_rooms_by_description(&areas);
        let rooms = Self::build_rooms(&areas);

        Self { areas, rooms_by_title_and_description, rooms_by_title, rooms_by_description, rooms }
    }

    fn build_rooms_by_title_and_description(
        areas: &HashMap<AreaId, Arc<AreaCache>>,
    ) -> HashMap<String, Vec<(AreaId, Arc<RoomCache>)>> {
        let mut ret = HashMap::new();
        for (area_id, area) in areas.iter() {
            for room in area.get_rooms().iter() {
                ret.entry(room.get_title_and_description().to_string())
                    .or_insert(Vec::new())
                    .push((area_id.clone(), room.clone()));
            }
        }
        ret
    }

    fn build_rooms_by_title(
        areas: &HashMap<AreaId, Arc<AreaCache>>,
    ) -> HashMap<String, Vec<(AreaId, Arc<RoomCache>)>> {
        let mut ret = HashMap::new();
        for (area_id, area) in areas.iter() {
            for room in area.get_rooms().iter() {
                ret.entry(room.get_title().to_string() )
                    .or_insert(Vec::new())
                    .push((area_id.clone(), room.clone()));
            }
        }
        ret
    }

    fn build_rooms_by_description(
        areas: &HashMap<AreaId, Arc<AreaCache>>,
    ) -> HashMap<String, Vec<(AreaId, Arc<RoomCache>)>> {
        let mut ret = HashMap::new();
        for (area_id, area) in areas.iter() {
            for room in area.get_rooms().iter() {
                ret.entry(room.get_description().to_string() )
                    .or_insert(Vec::new())
                    .push((area_id.clone(), room.clone()));
            }
        }
        ret
    }

    fn build_rooms(
        areas: &HashMap<AreaId, Arc<AreaCache>>,
    ) -> HashMap<RoomKey, Arc<RoomCache>> {
        let mut ret = HashMap::new();
        for (area_id, area) in areas.iter() {
            for room in area.get_rooms().iter() {
                ret.insert(RoomKey::new(area_id.clone(), room.get_room_number()), room.clone());
            }
        }
        ret
    }

    #[must_use]
    pub(super) fn add_area(&self, area_id: AreaId, area: Arc<AreaCache>) -> Self {
        let mut new_areas = self.areas.clone();
        new_areas.insert(area_id, area);

        Self::new_with_areas(new_areas)
    }

    #[must_use]
    pub(super) fn insert_area(&self, area_id: AreaId, area: Arc<AreaCache>) -> Self {
        let mut new_areas = self.areas.clone();
        new_areas.remove(&area_id);
        new_areas.insert(area_id, area);

        Self::new_with_areas(new_areas)
    }

    #[must_use]
    pub(super) fn delete_area(&self, area_id: AreaId) -> Self {
        let mut new_areas = self.areas.clone();
        new_areas.remove(&area_id);

        Self::new_with_areas(new_areas)
    }

    pub fn areas(&self) -> impl ExactSizeIterator<Item = Arc<AreaCache>> {
        self.areas.values().cloned()
    }

    pub fn get_area(&self, area_id: &AreaId) -> Option<Arc<AreaCache>> {
        self.areas.get(area_id).cloned()
    }

    pub fn get_rooms_by_title_and_description(&self, title: &str, description: &str) -> impl ExactSizeIterator<Item = (AreaId, Arc<RoomCache>)> {
        self.rooms_by_title_and_description.get(&format!("{}\r\n{}", title, description)).unwrap_or(&EMPTY_ROOMS_LOOKUP_VEC).iter().cloned()
    }

    pub fn get_rooms_by_title(&self, title: &str) -> impl ExactSizeIterator<Item = (AreaId, Arc<RoomCache>)> {
        self.rooms_by_title.get(&title.to_string()).unwrap_or(&EMPTY_ROOMS_LOOKUP_VEC).iter().cloned()
    }

    pub fn get_rooms_by_description(&self, description: &str) -> impl ExactSizeIterator<Item = (AreaId, Arc<RoomCache>)> {
        self.rooms_by_description.get(&description.to_string()).unwrap_or(&EMPTY_ROOMS_LOOKUP_VEC).iter().cloned()
    }

    pub fn get_room(&self, room_key: &RoomKey) -> Option<Arc<RoomCache>> {
        self.rooms.get(room_key).cloned()
    }
}