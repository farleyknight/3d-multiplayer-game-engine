# Architecture

This document describes the internal architecture and game state representation for the 3D multiplayer voxel game engine.

## Game State Overview

The game state is built from three core types:
- **PlayerState** - Tracks individual player position and orientation
- **VoxelWorld** - Contains all block data organized into chunks
- **Chunk** - A 16x16x16 unit of block storage

## PlayerState

Represents a player's current state in the game world. Defined in `src/lib.rs` within the `types` module.

```rust
pub struct PlayerState {
    /// Unique identifier assigned by the server
    pub player_id: u32,
    /// 3D position in world space
    pub position: Vec3,
    /// Rotation around the Y axis in radians
    pub rotation_yaw: f32,
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `player_id` | `u32` | Unique identifier assigned by the server when a player connects |
| `position` | `Vec3` | 3D position in world space (x, y, z coordinates using glam) |
| `rotation_yaw` | `f32` | Rotation around the Y axis in radians (horizontal look direction) |

The `position` field uses glam's `Vec3` type which provides x, y, z float components. The engine uses a Y-up coordinate system where:
- X axis: left/right
- Y axis: up/down
- Z axis: forward/back

## Player Location Tracking

This section documents how player position is stored, updated, and synchronized across the network.

### Position Storage

Player position is stored in the `position` field of `PlayerState` as a `Vec3` (glam's 3D vector type with x, y, z float components). The engine uses a Y-up coordinate system.

### Movement Update (Client-Side)

Player movement is processed in `src/bin/client.rs` using WASD keys:

```rust
// Key constants (src/bin/client.rs)
const MOVE_SPEED: f32 = 5.0;  // units per second
```

Movement processing:
1. Calculate forward direction from player yaw: `forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos())`
2. Calculate right direction perpendicular to forward
3. Accumulate movement based on pressed keys (W: +forward, S: -forward, A: -right, D: +right)
4. Normalize movement to prevent faster diagonal movement
5. Apply: `position += movement.normalize() * MOVE_SPEED * delta_time`

### Physics Update (Client-Side)

Physics is handled client-side in `src/bin/client.rs`:

```rust
// Physics constants (src/bin/client.rs)
const GROUND_Y: f32 = 0.0;      // floor level
const GRAVITY: f32 = 20.0;      // units/s² (higher than real for snappy feel)
const JUMP_VELOCITY: f32 = 8.0; // units/s (applied when Space pressed on ground)
```

Physics loop per frame:
1. **Jump**: If Space pressed AND on ground → set `velocity_y = JUMP_VELOCITY`
2. **Gravity**: `velocity_y -= GRAVITY * delta_time`
3. **Apply**: `position.y += velocity_y * delta_time`
4. **Floor collision**: If `position.y < GROUND_Y` → clamp to `GROUND_Y`, reset `velocity_y = 0`

### Network Synchronization

Position data flows through the network as follows:

```
Client Input → Local Position Update → PlayerUpdate Packet → Server → WorldState Broadcast → Other Clients
```

**Client sends position (20Hz):**
- Interval: 50ms (`NETWORK_UPDATE_INTERVAL`)
- Packet: `ClientPacket::PlayerUpdate(PlayerState)` containing position and rotation_yaw
- Serialization: bincode over UDP
- Reference: `src/bin/client.rs:157-175`

**Server receives and stores:**
- Updates `ConnectedClient.state.position` and `rotation_yaw`
- Reference: `src/bin/server.rs:97-127`

**Server broadcasts to all clients (20Hz):**
- Function: `broadcast_world_state()`
- Packet: `ServerPacket::WorldState { players: Vec<PlayerData> }`
- Contains all connected players' positions and rotations
- Reference: `src/bin/server.rs:144-162`

### Data Flow Summary

| Step | Location | Data |
|------|----------|------|
| Input | Client game loop | WASD/Space keys |
| Movement | `src/bin/client.rs:408-438` | Position += movement * speed * dt |
| Physics | `src/bin/client.rs:441-456` | Gravity, jumping, floor collision |
| Send | `src/bin/client.rs:157-175` | PlayerUpdate packet at 20Hz |
| Receive | `src/bin/server.rs:97-127` | Server stores position |
| Broadcast | `src/bin/server.rs:144-162` | WorldState to all clients |
| Render | Other clients | Display remote player positions |

## VoxelWorld

The world container that stores all block data organized into chunks. Defined in `src/lib.rs` within the `voxel` module.

```rust
pub struct VoxelWorld {
    /// Chunk data stored by chunk position (chunk_x, chunk_y, chunk_z)
    pub chunks: HashMap<(i32, i32, i32), Chunk>,
}
```

### Chunk Storage

Chunks are stored in a `HashMap` keyed by their chunk coordinates `(i32, i32, i32)`. This allows:
- Sparse world storage (only chunks with blocks are stored)
- Efficient lookup by chunk position
- Support for negative coordinates

### Coordinate Conversion

World coordinates convert to chunk coordinates using Euclidean division:
- Chunk position: `world_coord.div_euclid(16)`
- Local position within chunk: `world_coord.rem_euclid(16)`

This ensures correct handling of negative world coordinates.

## Chunk

A 16x16x16 unit of block storage. Defined in `src/lib.rs` within the `voxel` module.

```rust
pub const CHUNK_SIZE: usize = 16;

pub struct Chunk {
    /// Block data stored as a 3D array [x][y][z]
    /// Each element is an Option<BlockType> - None means air/empty
    blocks: [[[Option<BlockType>; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE],
}
```

### Block Storage

- Each chunk contains a fixed 16x16x16 3D array
- Array indexing: `blocks[x][y][z]`
- `Option<BlockType>` where `None` represents air (empty space)
- Local coordinates range from 0-15 for each axis

### Key Methods

| Method | Description |
|--------|-------------|
| `get_block(x, y, z)` | Get the block at a local position |
| `set_block(x, y, z, block_type)` | Place a block at a local position |
| `remove_block(x, y, z)` | Remove a block (set to air) |
| `block_count()` | Count non-air blocks in the chunk |
| `iter_blocks()` | Iterate over all blocks with positions |

## BlockType

Enum representing the different types of blocks in the world. Defined in `src/lib.rs` within the `blocks` module.

```rust
pub enum BlockType {
    Grass,       // Green top, dirt bottom, grass sides
    Dirt,        // Same texture on all sides
    Stone,       // Same texture on all sides
    Cobblestone, // Same texture on all sides
    Wood,        // Oak planks, same texture on all sides
}
```

Each block type has associated face textures for rendering, loaded from the Minecraft Bedrock texture pack in `textures/default-textures/textures/blocks/`.

## Data Flow

```
VoxelWorld
    └── HashMap<(i32, i32, i32), Chunk>
            └── Chunk
                    └── [[[Option<BlockType>; 16]; 16]; 16]
                            └── BlockType (Grass, Dirt, Stone, etc.)
```

1. **VoxelWorld** contains multiple **Chunks** indexed by chunk coordinates
2. Each **Chunk** contains a 16x16x16 array of optional **BlockTypes**
3. **PlayerState** exists separately and tracks player position/rotation within this world
