type AreaId = [BigInt, BigInt];
type RoomNumber = number;
type ExitId = [BigInt, BigInt];
type LabelId = [BigInt, BigInt];
type ShapeId = [BigInt, BigInt];
interface CreateRoomParams {
    title?: string;
    description?: string;
    level?: number;
    x?: number;
    y?: number;
    color?: string;
}
declare const mapper: {
    createArea(name: string): Promise<Area>;
    setCurrentLocation(areaId: AreaId, roomNumber?: RoomNumber): void;
    readonly areas: Area[];
    getAreaById(id: AreaId): Area;
    listRoomsByTitleAndDescription(title: string, description: string): any;
    renameArea(area: Area | AreaId, name: string): void;
    setRoomTitle(area: Area | AreaId, room: Room | RoomNumber, title: string): void;
    setRoomDescription(area: Area | AreaId, room: Room | RoomNumber, description: string): void;
    setRoomColor(area: Area | AreaId, room: Room | RoomNumber, color: string): void;
    setRoomLevel(area: Area | AreaId, room: Room | RoomNumber, level: number): void;
    setRoomX(area: Area | AreaId, room: Room | RoomNumber, x: number): void;
    setRoomY(area: Area | AreaId, room: Room | RoomNumber, y: number): void;
    setRoomProperty(area: Area | AreaId, room: Room | RoomNumber, name: string, value: string): void;
    createRoom(area: Area | AreaId, params: CreateRoomParams): RoomNumber;
    createRoomExit(area: Area | AreaId, room: Room | RoomNumber, exit: ExitArgs): Promise<ExitId>;
    setRoomExit(area: Area | AreaId, room: Room | RoomNumber, exitId: ExitId, exit: ExitUpdates): Promise<ExitId>;
    deleteRoomExit(area: Area | AreaId, room: Room | RoomNumber, exitId: ExitId): Promise<ExitId>;
};
interface Exit {
    id: ExitId;
    from_direction: string;
    from_area_id: AreaId;
    from_room_number: RoomNumber;
    to_direction?: string;
    to_area_id?: AreaId;
    to_room_number?: RoomNumber;
    is_hidden: boolean;
    is_closed: boolean;
    is_locked: boolean;
    weight: number;
    command?: string;
}
type ExitArgs = Pick<Exit, "from_direction"> & Partial<Omit<Exit, "id" | "from_direction" | "from_area_id" | "from_room_number">>;
type ExitUpdates = Partial<Omit<Exit, "id">>;
declare class Area {
    #private;
    constructor(obj: any);
    get id(): AreaId;
    get name(): string;
    get room_numbers(): RoomNumber[];
    get next_room_number(): RoomNumber;
    room(roomNumber: number): Room | undefined;
    data(key: string): string | undefined;
    toString(): any;
}
declare class Room {
    #private;
    constructor(obj: any);
    get room_number(): RoomNumber;
    get area_id(): AreaId;
    get title(): String;
    get description(): String;
    get level(): number;
    get x(): number;
    get y(): number;
    get color(): string;
    get exits(): Exit[];
    data(key: string): string | undefined;
    toString(): any;
}
//# sourceMappingURL=mapper.d.ts.map