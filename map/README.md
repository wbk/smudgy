# Smudgy Map Crate

A high-performance mapping library for Smudgy terminal application, designed for lock-free multi-threaded access with eventual consistency.

## Architecture Philosophy

**Simple, Fast, Eventually Consistent**

This crate embraces simplicity over complex synchronization. Both UI and JavaScript threads share the same high-performance cache, with instant reads and fire-and-forget writes. Backend synchronization happens asynchronously without blocking either thread.

### Core Components

- **`MapperTrait`**: Core trait defining all map operations (CRUD for areas, rooms, exits, labels, shapes, properties)
- **`CloudMapper`**: HTTP client implementation connecting to existing REST API
- **`SharedMapCache`**: DashMap-based cache providing lock-free access from any thread
- **Comprehensive data structures**: `Area`, `Room`, `Exit`, `Label`, `Shape` with Arc-based sharing

### Key Design Principles

1. **Lock-Free Reads**: Instant data access from any thread using DashMap
2. **Fire-and-Forget Writes**: Updates happen immediately in cache, sync to backend async
3. **Single Cache Implementation**: Both UI and JavaScript threads use identical interface
4. **Arc-Based Sharing**: Zero-copy data sharing between threads
5. **Eventual Consistency**: Simplicity over complex rollback mechanisms

## Current Status

✅ **Completed**:
- Core data structures matching existing API
- `CloudMapper` implementation for HTTP API integration
- Comprehensive error handling with `MapResult<T>`
- Updated to use "areas" terminology to match backend changes

🚧 **Next Steps**:
- `SharedMapCache` implementation with DashMap
- Spatial and property indices for fast queries
- Integration examples
- `LocalMapper` with SQLite backend

⏳ **Future**:
- Performance benchmarks and optimizations
- Cache eviction policies for memory management
- Advanced spatial indexing

## Usage

### Basic CloudMapper

```rust
use smudgy_map::{CloudMapper, MapperBackend};

// Create a cloud mapper instance
let mapper = CloudMapper::new(
    "https://api.smudgy.example.com".to_string(),
    "your-api-key".to_string(),
);

// List all areas
let areas = mapper.list_areas().await?;

// Get detailed area data  
let area_details = mapper.get_area(&area_id).await?;

// Create a new room
let room_updates = RoomUpdates {
    title: Some("Entrance Hall".to_string()),
    description: Some("A grand entrance hall".to_string()),
    level: Some(0),
    x: Some(0.0),
    y: Some(0.0),
    color: Some("#ffffff".to_string()),
    ..Default::default()
};
let room = mapper.update_room(&area_id, 1, room_updates).await?;
```

### Shared Cache (Planned Implementation)

```rust
use smudgy_map::{CloudMapper, Mapper};
use std::sync::Arc;

// Create backend and shared cache
let backend = Arc::new(CloudMapper::new(base_url, api_key));
let cache = Mapper::new(backend);

// Clone cache for use in different threads - zero cost!
let ui_cache = cache.clone();
let js_cache = cache.clone();

// UI Thread: Instant reads for rendering
let areas = ui_cache.get_all_areas();
let rooms_on_level = ui_cache.get_rooms_by_level(&area_id, 0);

// JavaScript Thread: Same interface, instant reads
let search_results = js_cache.search_rooms_by_title("Entrance Hall");

// Both threads: Fire-and-forget updates
ui_cache.update_room(area_id, 1, room_updates); // Returns immediately
js_cache.set_area_property(area_id, "name", "Dungeon"); // Returns immediately

// Backend sync happens automatically in background
```

### Multi-Thread Architecture

```rust
use std::sync::Arc;
use std::thread;

let cache = Arc::new(Mapper::new(backend));

// UI Thread
let ui_cache = cache.clone();
thread::spawn(move || {
    loop {
        // Instant access to area data for rendering
        let rooms = ui_cache.get_rooms_by_level(&area_id, current_level);
        render_rooms(rooms);
        std::thread::sleep(Duration::from_millis(16)); // 60 FPS
    }
});

// JavaScript Thread  
let js_cache = cache.clone();
thread::spawn(move || {
    // Script operations update cache immediately
    js_cache.set_room_property(area_id, room_id, "visited", "true");
    
    // Queries are instant - no waiting for synchronization
    let visited_rooms = js_cache.search_rooms_by_property("visited", "true");
    execute_script_with_data(visited_rooms);
});

// Background sync happens automatically - no thread management needed
```

## API Structure

The crate mirrors the existing HTTP API structure:

- **Areas**: Create, read, update, delete map areas (formerly "maps")
- **Area Properties**: Key-value metadata  
- **Rooms**: Individual locations with spatial data
- **Room Properties**: Custom metadata per room
- **Exits**: Connections between rooms (can span areas)
- **Labels**: Text annotations with positioning and alignment
- **Shapes**: Graphical elements (rectangles, rounded rectangles, etc.)

## Data Flow Design

```
UI Thread                  Shared Cache              JavaScript Thread
    |                     (DashMap-based)                   |
    | Instant Reads            |              Instant Reads |
    |<-------------------------|----------------------------->|
    |                          |                             |
    | Fire-and-Forget Updates  |   Fire-and-Forget Updates  |
    |------------------------->|<----------------------------|
    |                          |                             |
    |                     Background Sync                    |
    |                          |                             |
    |                          v                             |
    |                    Backend Storage                     |
    |                    (HTTP/SQLite)                       |
```

**Key Benefits**:
- No thread blocking - all operations return immediately
- No complex synchronization logic - DashMap handles concurrency
- No duplicate cache implementations - same code for all threads
- No MPSC channels - direct shared access
- Eventual consistency - simple and reliable

## Performance Characteristics

- **Read Operations**: O(1) hash table lookup with no locks
- **Write Operations**: O(1) cache update + async backend sync
- **Search Operations**: O(n) with pre-built indices (planned)
- **Memory Sharing**: Zero-copy via Arc<> references
- **Thread Contention**: Minimized by DashMap's sharding

## Dependencies

- **`dashmap`**: Lock-free concurrent HashMap for cache
- **`reqwest`**: HTTP client for CloudMapper
- **`uuid`**: Area and entity identification  
- **`chrono`**: Timestamp handling
- **`serde`**: JSON serialization
- **`async-trait`**: Async trait support

## Development

### Testing
```bash
cargo test
```

### Linting
```bash
cargo clippy --all-features -- -D warnings
```

### Documentation
```bash
cargo doc --open
```

## Integration Points

This crate is designed for:

1. **UI Thread**: Instant area data access for 60fps rendering
2. **JavaScript Runtime**: Script-driven map operations without blocking
3. **Existing API**: Seamless integration with current HTTP backend
4. **Future Local Storage**: SQLite backend for offline usage

The simplified architecture eliminates complex channel systems while providing better performance and maintainability than the previous design.

## Recent Changes

**v0.1.0 - Backend Reconciliation Update**:
- Updated terminology from "maps" to "areas" to match backend API changes
- Added support for atlas hierarchy (areas can belong to atlases)
- Enhanced labels with width, height, and alignment properties
- Added weight and command fields to exits
- Updated shapes to use border_radius instead of radius
- Improved type safety with proper enum usage for directions and alignment
- All API endpoints now use `/areas` instead of `/maps`
