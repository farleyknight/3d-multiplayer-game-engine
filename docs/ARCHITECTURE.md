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

## Input Handling Architecture

This section documents how keyboard and mouse inputs are captured and translated into player movement, jumping, and block interaction.

### Input Capture (winit Event Loop)

All input handling occurs within the winit event loop in `src/bin/client.rs:313-651`. The event loop processes:

- **WindowEvent::KeyboardInput** - WASD/Space key presses (line 321-355)
- **WindowEvent::MouseInput** - Left/Right click for block interaction (line 383-399)
- **DeviceEvent::MouseMotion** - Mouse delta for camera/look controls (line 627-644)

### Key State Tracking

Key states are tracked using boolean flags that persist between frames:

```rust
// Track WASD key press state for movement (src/bin/client.rs:295-299)
let mut w_pressed = false;
let mut a_pressed = false;
let mut s_pressed = false;
let mut d_pressed = false;
let mut space_pressed = false;
```

On each `KeyboardInput` event, the flag is set `true` when pressed and `false` when released:

```rust
let pressed = state == ElementState::Pressed;
match key_code {
    KeyCode::KeyW => w_pressed = pressed,
    KeyCode::KeyA => a_pressed = pressed,
    KeyCode::KeyS => s_pressed = pressed,
    KeyCode::KeyD => d_pressed = pressed,
    KeyCode::Space => space_pressed = pressed,
    _ => {}
}
```

Reference: `src/bin/client.rs:346-354`

### Key Mappings

| Key | Action | State Variable |
|-----|--------|----------------|
| W | Move forward | `w_pressed` |
| A | Move left (strafe) | `a_pressed` |
| S | Move backward | `s_pressed` |
| D | Move right (strafe) | `d_pressed` |
| Space | Jump | `space_pressed` |
| Escape | Exit game | (immediate exit) |
| Left Mouse | Destroy block | `left_mouse_clicked` |
| Right Mouse | Place block | `right_mouse_clicked` |

### Movement Processing

Movement is calculated per frame when the cursor is captured. The process:

1. **Calculate forward direction** from player yaw using spherical coordinates:
   ```rust
   // Player faces -Z when yaw = 0 (src/bin/client.rs:412-416)
   let forward = Vec3::new(
       -player.rotation_yaw.sin(),
       0.0,
       -player.rotation_yaw.cos(),
   );
   ```

2. **Calculate right direction** perpendicular to forward in the XZ plane:
   ```rust
   // Right is 90 degrees clockwise from forward (src/bin/client.rs:418)
   let right = Vec3::new(forward.z, 0.0, -forward.x);
   ```

3. **Accumulate movement** based on pressed keys:
   ```rust
   let mut movement = Vec3::ZERO;
   if w_pressed { movement += forward; }
   if s_pressed { movement -= forward; }
   if a_pressed { movement -= right; }
   if d_pressed { movement += right; }
   ```

4. **Normalize and apply** to prevent faster diagonal movement:
   ```rust
   if movement.length_squared() > 0.0 {
       movement = movement.normalize() * MOVE_SPEED * delta_time;
       player.position += movement;
   }
   ```

Reference: `src/bin/client.rs:408-438`

### Movement Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MOVE_SPEED` | 5.0 | Units per second |
| `MOUSE_SENSITIVITY` | 0.003 | Radians per pixel of mouse movement |

### Jump Mechanics

Jumping uses a simple velocity-based system with ground detection:

1. **Ground check**: Player is on ground when `position.y <= GROUND_Y`
2. **Jump trigger**: When Space is pressed AND player is on ground
3. **Apply jump**: Set `velocity_y = JUMP_VELOCITY` (8.0 units/s upward)
4. **Gravity**: Each frame: `velocity_y -= GRAVITY * delta_time`
5. **Position update**: `position.y += velocity_y * delta_time`
6. **Floor collision**: If `position.y < GROUND_Y`, clamp to ground and reset velocity

```rust
// Jump when space pressed and on ground (src/bin/client.rs:441-444)
if space_pressed && player.position.y <= GROUND_Y {
    velocity_y = JUMP_VELOCITY;
}

// Apply gravity (src/bin/client.rs:446-447)
velocity_y -= GRAVITY * delta_time;
position.y += velocity_y * delta_time;

// Floor collision (src/bin/client.rs:452-456)
if player.position.y < GROUND_Y {
    player.position.y = GROUND_Y;
    velocity_y = 0.0;
}
```

### Physics Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `GROUND_Y` | 0.0 | Floor level (Y coordinate) |
| `GRAVITY` | 20.0 | Downward acceleration (units/s²) |
| `JUMP_VELOCITY` | 8.0 | Initial upward velocity when jumping (units/s) |

### Mouse Input (Look Controls)

Mouse movement controls the player's horizontal look direction (yaw) and vertical look angle (pitch):

1. **Yaw update**: Horizontal mouse movement rotates the player:
   ```rust
   // Negative because moving mouse right should rotate clockwise
   player.rotation_yaw -= delta.0 as f32 * MOUSE_SENSITIVITY;
   ```

2. **Pitch update**: Vertical mouse movement tilts the view (used for block raycasting):
   ```rust
   pitch -= delta.1 as f32 * MOUSE_SENSITIVITY;
   pitch = pitch.clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());
   ```

3. **Cursor capture**: Mouse look only works when the window is focused and cursor is captured (`cursor_captured = true`).

Reference: `src/bin/client.rs:627-644`

### Block Interaction (Mouse Clicks)

Block placement and destruction use raycasting from the player's eye position:

1. **Ray origin**: Player position + eye height offset (`EYE_HEIGHT = 1.6`)
2. **Ray direction**: Calculated from yaw and pitch using spherical coordinates:
   ```rust
   let ray_direction = Vec3::new(
       -player.rotation_yaw.sin() * pitch.cos(),
       pitch.sin(),
       -player.rotation_yaw.cos() * pitch.cos(),
   ).normalize();
   ```

3. **Raycast**: Check for block intersection within `BLOCK_REACH_DISTANCE` (5.0 units)
4. **Left click**: Destroy the hit block
5. **Right click**: Place a block adjacent to the hit face (using face normal)

Reference: `src/bin/client.rs:459-505`

### Block Interaction Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `BLOCK_REACH_DISTANCE` | 5.0 | Maximum distance for block interaction (units) |
| `EYE_HEIGHT` | 1.6 | Height offset from player position for raycasting (units) |

### Input Flow Summary

```
Keyboard Input                    Mouse Input
      │                                │
      ▼                                ▼
WindowEvent::KeyboardInput    DeviceEvent::MouseMotion
      │                                │
      ▼                                ▼
Update boolean flags           Update yaw/pitch
(w/a/s/d/space_pressed)              │
      │                                │
      ▼                                ▼
Calculate forward/right ◄──────── Player yaw
      │
      ▼
Normalize movement vector
      │
      ▼
Apply: position += movement * MOVE_SPEED * dt
      │
      ▼
Jump check → Apply gravity → Floor collision
      │
      ▼
Update camera position to follow player
```
