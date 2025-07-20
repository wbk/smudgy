use std::{borrow::Cow, cell::RefCell, ffi::CStr, rc::Rc, sync::Arc};

use deno_core::{
    GarbageCollected, JsBuffer, OpState, Resource, ResourceId,
    cppgc::Ptr,
    error::AnyError,
    op2, thiserror,
    v8::{self, cppgc::Member},
};
use serde::{Deserialize, Serialize};
use smudgy_map::{
    Area, AreaId, CreateAreaRequest, ExitArgs, ExitDirection, ExitId, ExitUpdates, Mapper,
    RoomNumber, RoomUpdates, Uuid,
    mapper::{RoomKey, area_cache::AreaCache, room_cache::RoomCache},
};

deno_core::extension!(
  smudgy_mapper,
  ops = [
      op_smudgy_mapper_list_area_ids,
      op_smudgy_mapper_create_area,
      op_smudgy_mapper_get_area_by_id,
      op_smudgy_mapper_get_area_name,
      op_smudgy_mapper_get_area_id,
      op_smudgy_mapper_rename_area,
      op_smudgy_mapper_list_area_room_numbers,
      op_smudgy_mapper_list_rooms_by_title_and_description,
      op_smudgy_mapper_get_area_room_by_number,
      op_smudgy_mapper_get_area_property,
      op_smudgy_mapper_get_area_next_room_number,
      op_smudgy_mapper_get_room_area_id,
      op_smudgy_mapper_get_room_number,
      op_smudgy_mapper_get_room_title,
      op_smudgy_mapper_get_room_description,
      op_smudgy_mapper_get_room_level,
      op_smudgy_mapper_get_room_x,
      op_smudgy_mapper_get_room_y,
      op_smudgy_mapper_get_room_color,
      op_smudgy_mapper_get_room_property,
      op_smudgy_mapper_get_room_exits,
      op_smudgy_mapper_set_room_title,
      op_smudgy_mapper_set_room_description,
      op_smudgy_mapper_set_room_color,
      op_smudgy_mapper_set_room_level,
      op_smudgy_mapper_set_room_x,
      op_smudgy_mapper_set_room_y,
      op_smudgy_mapper_set_room_property,
      op_smudgy_mapper_create_room,
      op_smudgy_mapper_create_room_exit,
      op_smudgy_mapper_set_room_exit,
      op_smudgy_mapper_delete_room,
      op_smudgy_mapper_delete_room_exit,
      ],
  esm_entry_point = "ext:smudgy_mapper/mapper.ts",
  esm = [ dir "src/session/runtime/script_engine/mapper", "mapper.ts" ],
  options = {
    mapper: Option<Mapper>,
  },
  state = |state, options| {
    if let Some(mapper) = options.mapper {
        state.put::<Mapper>(mapper);
    }
  },
);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum MapperError {
    #[class(generic)]
    #[error("Mapper not enabled in this session")]
    MapperNotEnabled,
    #[class(generic)]
    #[error("Area not found")]
    AreaNotFound,
    #[class(generic)]
    #[error("Invalid UUID")]
    InvalidUuid,
    #[class(generic)]
    #[error("Failed to create map: {0}")]
    FailedToCreate(String),
}

#[op2]
#[serde]
fn op_smudgy_mapper_list_area_ids(state: &mut OpState) -> Vec<(u64, u64)> {
    let mapper = state.try_borrow::<Mapper>();

    if let Some(mapper) = mapper {
        let atlas = mapper.get_current_atlas();

        atlas
            .areas()
            .map(|map| map.get_id().0.as_u64_pair())
            .collect::<Vec<_>>()
    } else {
        vec![]
    }
}

#[op2(async)]
#[cppgc]
async fn op_smudgy_mapper_create_area(
    state: Rc<RefCell<OpState>>,
    #[string] name: String,
) -> Result<JSArea, MapperError> {
    let mapper = {
        let state = state.borrow();
        let mapper = state.try_borrow::<Mapper>();
        mapper.cloned()
    };

    if let Some(mapper) = mapper {
        let id = mapper
            .create_area(name)
            .await
            .map_err(|e| MapperError::FailedToCreate(e.to_string()))?;

        return mapper
            .get_current_atlas()
            .get_area(&id)
            .map(|area| JSArea(area.clone()))
            .ok_or(MapperError::AreaNotFound);
    }

    Err(MapperError::MapperNotEnabled)
}

pub struct JSArea(pub Arc<AreaCache>);

impl GarbageCollected for JSArea {
    fn get_name(&self) -> &'static std::ffi::CStr {
        c"Area"
    }
}

pub struct JSRoom(pub Arc<RoomCache>, pub AreaId);

impl GarbageCollected for JSRoom {
    fn get_name(&self) -> &'static std::ffi::CStr {
        c"Room"
    }
}

#[op2]
#[cppgc]
fn op_smudgy_mapper_get_area_by_id(
    state: Rc<RefCell<OpState>>,
    #[serde] id: (u64, u64),
) -> Result<JSArea, MapperError> {
    let atlas = {
        let state = state.borrow();
        let mapper = state.try_borrow::<Mapper>();
        mapper.map(|mapper| mapper.get_current_atlas())
    };

    if let Some(atlas) = atlas {
        let id = AreaId(Uuid::from_u64_pair(id.0, id.1));
        if let Some(area) = atlas.get_area(&id) {
            return Ok(JSArea(area.clone()));
        } else {
            return Err(MapperError::AreaNotFound);
        }
    }

    Err(MapperError::MapperNotEnabled)
}

#[op2]
fn op_smudgy_mapper_rename_area(
    state: &OpState,
    #[serde] area_id: (u64, u64),
    #[string] name: String,
) -> Result<(), MapperError> {
    let mapper = state.try_borrow::<Mapper>();

    if let Some(mapper) = mapper {
        let id = AreaId(Uuid::from_u64_pair(area_id.0, area_id.1));
        mapper.rename_area(id.clone(), name.as_str());
        Ok(())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

/// AREA WRAPPER METHODS
///
#[op2]
fn op_smudgy_mapper_get_area_name<'a>(
    scope: &'a mut v8::HandleScope,
    #[cppgc] area_wrapper: &JSArea,
) -> v8::Local<'a, v8::String> {
    v8::String::new(scope, area_wrapper.0.get_name())
        .unwrap_or_else(|| v8::String::new(scope, "unknown").expect("Failed to create string"))
}

#[op2]
#[serde]
fn op_smudgy_mapper_get_area_id(#[cppgc] area_wrapper: &JSArea) -> (u64, u64) {
    area_wrapper.0.get_id().0.as_u64_pair()
}
#[op2]
#[serde]
fn op_smudgy_mapper_list_area_room_numbers(#[cppgc] area_wrapper: &JSArea) -> Vec<i32> {
    area_wrapper
        .0
        .get_rooms()
        .iter()
        .map(|room| room.get_room_number().0)
        .collect()
}

#[op2]
#[serde]
fn op_smudgy_mapper_list_rooms_by_title_and_description(
    state: &OpState,
    #[string] title: &str,
    #[string] description: &str,
) -> Vec<((u64, u64), i32)> {
    let mapper = state.try_borrow::<Mapper>();

    if let Some(mapper) = mapper {
        let atlas = mapper.get_current_atlas();
        let rooms = atlas.get_rooms_by_title_and_description(title, description);
        rooms.map(|(area_id, room)| (area_id.0.as_u64_pair(), room.get_room_number().0)).collect()
    } else {
        vec![]
    }
}

#[op2]
#[cppgc]
fn op_smudgy_mapper_get_area_room_by_number(
    #[cppgc] area_wrapper: &JSArea,
    room_number: i32,
) -> Option<JSRoom> {
    area_wrapper
        .0
        .get_room(&RoomNumber(room_number))
        .map(|room| JSRoom(room.clone(), area_wrapper.0.get_id().clone()))
}

#[op2]
fn op_smudgy_mapper_get_area_property<'a>(
    scope: &'a mut v8::HandleScope,
    #[cppgc] area_wrapper: &JSArea,
    #[string] name: String,
) -> v8::Local<'a, v8::Value> {
    match area_wrapper.0.get_property(&name) {
        Some(property) => v8::String::new(scope, property)
            .expect("Invalid property")
            .into(),
        None => v8::undefined(scope).into(),
    }
}

#[op2(fast)]
#[smi]
fn op_smudgy_mapper_get_area_next_room_number(
    #[cppgc] area_wrapper: &JSArea,
) -> i32 {
    area_wrapper.0.get_max_room_number().0 + 1
}

/// ROOM WRAPPER METHODS
///
///
#[op2]
#[serde]
fn op_smudgy_mapper_get_room_area_id(#[cppgc] room_wrapper: &JSRoom) -> (u64, u64) {
    room_wrapper.1.0.as_u64_pair()
}

#[op2(fast)]
#[smi]
fn op_smudgy_mapper_get_room_number(#[cppgc] room_wrapper: &JSRoom) -> i32 {
    room_wrapper.0.get_room_number().0
}

#[op2]
fn op_smudgy_mapper_get_room_title<'a>(
    scope: &'a mut v8::HandleScope,
    #[cppgc] room_wrapper: &JSRoom,
) -> v8::Local<'a, v8::String> {
    v8::String::new(scope, room_wrapper.0.get_title()).expect("Failed to create string")
}

#[op2]
fn op_smudgy_mapper_get_room_description<'a>(
    scope: &'a mut v8::HandleScope,
    #[cppgc] room_wrapper: &JSRoom,
) -> v8::Local<'a, v8::String> {
    v8::String::new(scope, room_wrapper.0.get_description()).expect("Failed to create string")
}

#[op2]
fn op_smudgy_mapper_get_room_color<'a>(
    scope: &'a mut v8::HandleScope,
    #[cppgc] room_wrapper: &JSRoom,
) -> v8::Local<'a, v8::String> {
    v8::String::new(scope, room_wrapper.0.get_color()).expect("Failed to create string")
}

#[op2(fast)]
#[smi]
fn op_smudgy_mapper_get_room_level(#[cppgc] room_wrapper: &JSRoom) -> i32 {
    room_wrapper.0.get_level()
}

#[op2(fast)]
fn op_smudgy_mapper_get_room_x(#[cppgc] room_wrapper: &JSRoom) -> f32 {
    room_wrapper.0.get_x()
}

#[op2(fast)]
fn op_smudgy_mapper_get_room_y(#[cppgc] room_wrapper: &JSRoom) -> f32 {
    room_wrapper.0.get_y()
}

#[op2]
fn op_smudgy_mapper_get_room_property<'a>(
    scope: &'a mut v8::HandleScope,
    #[cppgc] room_wrapper: &JSRoom,
    #[string] name: String,
) -> v8::Local<'a, v8::Value> {
    match room_wrapper.0.get_property(&name) {
        Some(property) => v8::String::new(scope, property)
            .expect("Invalid property")
            .into(),
        None => v8::undefined(scope).into(),
    }
}

#[derive(Debug, Serialize)]
struct JSExit {
    id: (u64, u64),
    from_direction: String,
    from_area_id: (u64, u64),
    from_room_number: i32,
    to_direction: Option<String>,
    to_area_id: Option<(u64, u64)>,
    to_room_number: Option<i32>,
    is_hidden: bool,
    is_closed: bool,
    is_locked: bool,
    weight: f32,
    command: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JSExitCreateParams {
    from_direction: ExitDirection,
    to_direction: Option<ExitDirection>,
    to_area_id: Option<(u64, u64)>,
    to_room_number: Option<i32>,
    is_hidden: Option<bool>,
    is_closed: Option<bool>,
    is_locked: Option<bool>,
    weight: Option<f32>,
    command: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JSExitUpdateParams {
    from_direction: Option<ExitDirection>,
    to_direction: Option<ExitDirection>,
    to_area_id: Option<(u64, u64)>,
    to_room_number: Option<i32>,
    is_hidden: Option<bool>,
    is_closed: Option<bool>,
    is_locked: Option<bool>,
    weight: Option<f32>,
    command: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JSRoomParams {
    title: Option<String>,
    description: Option<String>,
    color: Option<String>,
    level: Option<i32>,
    x: Option<f32>,
    y: Option<f32>,
}

#[op2]
#[serde]
fn op_smudgy_mapper_get_room_exits<'a>(#[cppgc] room_wrapper: &JSRoom) -> Vec<JSExit> {
    room_wrapper
        .0
        .get_exits()
        .iter()
        .map(|exit| JSExit {
            id: exit.id.0.as_u64_pair(),
            from_direction: exit.from_direction.to_string(),
            from_area_id: room_wrapper.1.0.as_u64_pair(),
            from_room_number: room_wrapper.0.get_room_number().0,
            to_direction: exit.to_direction.map(|direction| direction.to_string()),
            to_area_id: exit.to_area_id.map(|area_id| area_id.0.as_u64_pair()),
            to_room_number: exit.to_room_number.map(|room_number| room_number.0),
            is_hidden: exit.is_hidden,
            is_closed: exit.is_closed,
            is_locked: exit.is_locked,
            weight: exit.weight,
            command: exit.command.clone(),
        })
        .collect()
}

/// ROOM SETTER METHODS
///
#[op2]
fn op_smudgy_mapper_set_room_title(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    room_number: i32,
    #[string] title: String,
) -> Result<(), MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>() {
        let area_id = AreaId(Uuid::from_u64_pair(area_id.0, area_id.1));
        mapper.upsert_room(
            RoomKey {
                area_id,
                room_number: RoomNumber(room_number),
            },
            RoomUpdates {
                title: Some(title),
                ..Default::default()
            },
        );
        Ok(())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

#[op2]
fn op_smudgy_mapper_set_room_description(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    room_number: i32,
    #[string] description: String,
) -> Result<(), MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>() {
        let area_id = AreaId(Uuid::from_u64_pair(area_id.0, area_id.1));
        mapper.upsert_room(
            RoomKey {
                area_id,
                room_number: RoomNumber(room_number),
            },
            RoomUpdates {
                description: Some(description),
                ..Default::default()
            },
        );
        Ok(())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

#[op2]
fn op_smudgy_mapper_set_room_color(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    room_number: i32,
    #[string] color: String,
) -> Result<(), MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>() {
        let area_id = AreaId(Uuid::from_u64_pair(area_id.0, area_id.1));
        mapper.upsert_room(
            RoomKey {
                area_id,
                room_number: RoomNumber(room_number),
            },
            RoomUpdates {
                color: Some(color),
                ..Default::default()
            },
        );
        Ok(())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

#[op2]
fn op_smudgy_mapper_set_room_level(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    room_number: i32,
    level: i32,
) -> Result<(), MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>() {
        let area_id = AreaId(Uuid::from_u64_pair(area_id.0, area_id.1));
        mapper.upsert_room(
            RoomKey {
                area_id,
                room_number: RoomNumber(room_number),
            },
            RoomUpdates {
                level: Some(level),
                ..Default::default()
            },
        );
        Ok(())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

#[op2]
fn op_smudgy_mapper_set_room_x(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    room_number: i32,
    x: f32,
) -> Result<(), MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>() {
        let area_id = AreaId(Uuid::from_u64_pair(area_id.0, area_id.1));
        mapper.upsert_room(
            RoomKey {
                area_id,
                room_number: RoomNumber(room_number),
            },
            RoomUpdates {
                x: Some(x),
                ..Default::default()
            },
        );
        Ok(())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

#[op2]
fn op_smudgy_mapper_set_room_y(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    room_number: i32,
    y: f32,
) -> Result<(), MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>() {
        let area_id = AreaId(Uuid::from_u64_pair(area_id.0, area_id.1));
        mapper.upsert_room(
            RoomKey {
                area_id,
                room_number: RoomNumber(room_number),
            },
            RoomUpdates {
                y: Some(y),
                ..Default::default()
            },
        );
        Ok(())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

#[op2]
fn op_smudgy_mapper_set_room_property(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    room_number: i32,
    #[string] name: String,
    #[string] value: String,
) -> Result<(), MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>() {
        let area_id = AreaId(Uuid::from_u64_pair(area_id.0, area_id.1));
        mapper.set_room_property(
            RoomKey {
                area_id,
                room_number: RoomNumber(room_number),
            },
            name,
            value,
        );
        Ok(())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

#[op2]
#[smi]
fn op_smudgy_mapper_create_room(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    #[serde] params: JSRoomParams,
) -> Result<i32, MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>() {
        let area_id = AreaId(Uuid::from_u64_pair(area_id.0, area_id.1));
        let current_atlas = mapper.get_current_atlas();
        let area = current_atlas.get_area(&area_id);

        if let Some(area) = area {
            let room_number = area.get_max_room_number().0 + 1;

            mapper.upsert_room(
                RoomKey {
                    area_id,
                    room_number: RoomNumber(room_number),
                },
                RoomUpdates {
                    title: params.title,
                    description: params.description,
                    color: params.color,
                    level: params.level,
                    x: params.x,
                    y: params.y,
                    ..Default::default()
                },
            );

            Ok(room_number)
        } else {
            Err(MapperError::AreaNotFound)
        }
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

#[op2(async)]
#[serde]
async fn op_smudgy_mapper_create_room_exit(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    room_number: i32,
    #[serde] params: JSExitCreateParams,
) -> Result<(u64, u64), MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>().cloned() {
        drop(state);

        let id = mapper
            .create_exit(
                RoomKey {
                    area_id: AreaId(Uuid::from_u64_pair(area_id.0, area_id.1)),
                    room_number: RoomNumber(room_number),
                },
                ExitArgs {
                    from_direction: params.from_direction,
                    to_direction: params.to_direction,
                    to_area_id: params
                        .to_area_id
                        .map(|area_id| AreaId(Uuid::from_u64_pair(area_id.0, area_id.1))),
                    to_room_number: params
                        .to_room_number
                        .map(|room_number| RoomNumber(room_number)),
                    is_hidden: params.is_hidden.unwrap_or(false),
                    is_closed: params.is_closed.unwrap_or(false),
                    is_locked: params.is_locked.unwrap_or(false),
                    weight: params.weight.unwrap_or(1.0),
                    command: params.command,
                    path: None,
                },
            )
            .await
            .map_err(|e| MapperError::FailedToCreate(e.to_string()))?;

        Ok(id.0.as_u64_pair())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

#[op2]
fn op_smudgy_mapper_set_room_exit(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    room_number: i32,
    #[serde] exit_id: (u64, u64),
    #[serde] params: JSExitUpdateParams,
) -> Result<(), MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>().cloned() {
        mapper.update_exit(
            RoomKey {
                area_id: AreaId(Uuid::from_u64_pair(area_id.0, area_id.1)),
                room_number: RoomNumber(room_number),
            },
            ExitId(Uuid::from_u64_pair(exit_id.0, exit_id.1)),
            ExitUpdates {
                from_direction: params.from_direction,
                to_direction: params.to_direction,
                to_area_id: params
                    .to_area_id
                    .map(|area_id| AreaId(Uuid::from_u64_pair(area_id.0, area_id.1))),
                to_room_number: params
                    .to_room_number
                    .map(|room_number| RoomNumber(room_number)),
                is_hidden: params.is_hidden,
                is_closed: params.is_closed,
                is_locked: params.is_locked,
                weight: params.weight,
                command: params.command,
                path: None,
            },
        );

        Ok(())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

#[op2]
fn op_smudgy_mapper_delete_room(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    room_number: i32,
) -> Result<(), MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>().cloned() {
        mapper.delete_room(RoomKey {
            area_id: AreaId(Uuid::from_u64_pair(area_id.0, area_id.1)),
            room_number: RoomNumber(room_number),
        });
        Ok(())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}

#[op2]
fn op_smudgy_mapper_delete_room_exit(
    state: Rc<RefCell<OpState>>,
    #[serde] area_id: (u64, u64),
    room_number: i32,
    #[serde] exit_id: (u64, u64)
) -> Result<(), MapperError> {
    let state = state.borrow();
    if let Some(mapper) = state.try_borrow::<Mapper>().cloned() {
        mapper.delete_exit(RoomKey {
            area_id: AreaId(Uuid::from_u64_pair(area_id.0, area_id.1)),
            room_number: RoomNumber(room_number),
        }, ExitId(Uuid::from_u64_pair(exit_id.0, exit_id.1)));
        Ok(())
    } else {
        Err(MapperError::MapperNotEnabled)
    }
}
