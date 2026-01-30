# Network Protocol Specification

## Overview
Simple UDP-based protocol for position synchronization. Server acts as relay/broadcast hub.

## Connection Model
- Server listens on configurable port (default: 7878)
- Clients send packets to server address
- Server tracks clients by source address (no explicit handshake)
- Client considered "connected" on first packet received
- Client considered "disconnected" after 5 seconds of no packets

## Packet Types

### Client → Server

#### PlayerUpdate (0x01)
Sent every frame (or at fixed rate ~20Hz) with player state.
```
[1 byte]  packet_type = 0x01
[4 bytes] player_id (u32, assigned by server on first contact, 0 if new)
[12 bytes] position (3x f32: x, y, z)
[4 bytes] rotation_yaw (f32, radians)
```

### Server → Client

#### Welcome (0x10)
Sent once when server first sees a new client.
```
[1 byte]  packet_type = 0x10
[4 bytes] assigned_player_id (u32)
```

#### WorldState (0x11)
Broadcast to all clients at fixed rate (~20Hz).
```
[1 byte]  packet_type = 0x11
[1 byte]  player_count (u8)
[N × PlayerData] players

PlayerData:
  [4 bytes] player_id (u32)
  [12 bytes] position (3x f32)
  [4 bytes] rotation_yaw (f32)
```

#### PlayerLeft (0x12)
Broadcast when a player disconnects.
```
[1 byte]  packet_type = 0x12
[4 bytes] player_id (u32)
```

## Server Behavior
1. Receive PlayerUpdate from client
2. If new source address → assign player_id, send Welcome, add to player list
3. Update stored position/rotation for that player
4. Every 50ms: broadcast WorldState to all connected clients
5. Every 1 second: check for timeouts, broadcast PlayerLeft for disconnected

## Client Behavior
1. On start: begin sending PlayerUpdate (with player_id=0 initially)
2. On Welcome: store assigned player_id, use in future updates
3. On WorldState: update positions of all other players
4. On PlayerLeft: remove that player from local render list

## Serialization
All multi-byte values are little-endian. Use `bincode` with default settings for convenience, or manual serialization for packet types.
