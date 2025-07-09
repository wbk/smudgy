declare const Deno: {
    core: {
        ops: any
    };
};

const {
    op_smudgy_mapper_list_area_ids,
    op_smudgy_mapper_list_area_room_numbers,
    op_smudgy_mapper_create_area,
    op_smudgy_select_area,
    op_smudgy_mapper_rename_area,
    op_smudgy_mapper_get_area_by_id,
    op_smudgy_mapper_get_area_name,
    op_smudgy_mapper_get_area_id,
    op_smudgy_mapper_get_area_room_by_number,
    op_smudgy_mapper_get_area_property,
    op_smudgy_mapper_get_room_number,
    op_smudgy_mapper_get_room_title,
    op_smudgy_mapper_get_room_description,
    op_smudgy_mapper_get_room_level,
    op_smudgy_mapper_get_room_x,
    op_smudgy_mapper_get_room_y,
    op_smudgy_mapper_get_room_color,
    op_smudgy_mapper_get_room_property,
    op_smudgy_mapper_set_room_title,
    op_smudgy_mapper_set_room_description,
    op_smudgy_mapper_set_room_color,
    op_smudgy_mapper_set_room_level,
    op_smudgy_mapper_set_room_x,
    op_smudgy_mapper_set_room_y,
    op_smudgy_mapper_set_room_property

} = (Deno as any).core.ops;

import { core, primordials } from "ext:core/mod.js";

const { internalRidSymbol } = core;
const {
    ObjectDefineProperty,
    TypeError,
    SymbolFor,
} = primordials;

type AreaId = [BigInt, BigInt];
type RoomNumber = number;
type RoomKey = [AreaId, RoomNumber];
type ExitId = [BigInt, BigInt];
type LabelId = [BigInt, BigInt];
type ShapeId = [BigInt, BigInt];

interface CreateRoomParams {

}

const mapper = {
    selectArea(areaId: AreaId) {
        op_smudgy_select_area(areaId);
    },
    get areas(): Area[] {
        return op_smudgy_mapper_list_area_ids().map(id => new Area( op_smudgy_mapper_get_area_by_id(id)));
    },

    getAreaById(id: AreaId) {
        let area = op_smudgy_mapper_get_area_by_id(id);
        return new Area(area);
    },

    renameArea(area: Area | AreaId, name: string) {
        const areaId = area instanceof Area ? area.id : area;
        op_smudgy_mapper_rename_area(areaId, name);
    },

    setRoomTitle(area: Area | AreaId, room: Room | RoomNumber, title: string) {
        const areaId = area instanceof Area ? area.id : area;
        const roomNumber = room instanceof Room ? room.room_number : room;
        op_smudgy_mapper_set_room_title(areaId, roomNumber, title);
    },

    setRoomDescription(area: Area | AreaId, room: Room | RoomNumber, description: string) {
        const areaId = area instanceof Area ? area.id : area;
        const roomNumber = room instanceof Room ? room.room_number : room;
        op_smudgy_mapper_set_room_description(areaId, roomNumber, description);
    },

    setRoomColor(area: Area | AreaId, room: Room | RoomNumber, color: string) {
        const areaId = area instanceof Area ? area.id : area;
        const roomNumber = room instanceof Room ? room.room_number : room;
        op_smudgy_mapper_set_room_color(areaId, roomNumber, color);
    },

    setRoomLevel(area: Area | AreaId, room: Room | RoomNumber, level: number) {
        const areaId = area instanceof Area ? area.id : area;
        const roomNumber = room instanceof Room ? room.room_number : room;
        op_smudgy_mapper_set_room_level(areaId, roomNumber, level);
    },

    setRoomX(area: Area | AreaId, room: Room | RoomNumber, x: number) {
        const areaId = area instanceof Area ? area.id : area;
        const roomNumber = room instanceof Room ? room.room_number : room;
        op_smudgy_mapper_set_room_x(areaId, roomNumber, x);
    },

    setRoomY(area: Area | AreaId, room: Room | RoomNumber, y: number) {
        const areaId = area instanceof Area ? area.id : area;
        const roomNumber = room instanceof Room ? room.room_number : room;
        op_smudgy_mapper_set_room_y(areaId, roomNumber, y);
    },

    setRoomProperty(area: Area | AreaId, room: Room | RoomNumber, name: string, value: string) {
        const areaId = area instanceof Area ? area.id : area;
        const roomNumber = room instanceof Room ? room.room_number : room;
        op_smudgy_mapper_set_room_property(areaId, roomNumber, name, value);
    }
};

class Area {
    #obj: any;

    constructor(obj: any) {
        this.#obj = obj;
    }

    static async create(name: string) {
        const id = await op_smudgy_mapper_create_area(name);
        return new Area(id);
    }

    get id(): AreaId {
        return op_smudgy_mapper_get_area_id(this.#obj);
    }

    get name(): string {
        return op_smudgy_mapper_get_area_name(this.#obj);
    }

    get room_numbers(): RoomNumber[] {
        return op_smudgy_mapper_list_area_room_numbers(this.#obj) || [];
    }

    room(roomNumber: number): Room | undefined {
        const room: Room | undefined = op_smudgy_mapper_get_area_room_by_number(this.#obj, roomNumber);
        return room && new Room(room);
    }

    data(key: string): string | undefined {
        return op_smudgy_mapper_get_area_property(this.#obj, key);
    }

    toString() {
        return this.#obj.toString();
    }
}

class Room {
    #obj: any;

    constructor(obj: any) {
        this.#obj = obj;
    }

    get room_number(): RoomNumber {
        return op_smudgy_mapper_get_room_number(this.#obj);
    }

    get title(): String {
        return op_smudgy_mapper_get_room_title(this.#obj);
    }
    
    get description(): String {
        return op_smudgy_mapper_get_room_description(this.#obj);
    }
    
    get level(): number {
        return op_smudgy_mapper_get_room_level(this.#obj);
    }
    
    get x(): number {
        return op_smudgy_mapper_get_room_x(this.#obj);
    }

    get y(): number {
        return op_smudgy_mapper_get_room_y(this.#obj);
    }

    get color(): string {
        return op_smudgy_mapper_get_room_color(this.#obj);
    }

    data(key: string): string | undefined {
        return op_smudgy_mapper_get_room_property(this.#obj, key);
    }

    toString() {
        return this.#obj.toString();
    }
}

Object.defineProperty(globalThis, "mapper", { value: mapper });
Object.defineProperty(globalThis, "Area", { value: Area });
