**Project: Minimal 3D Multiplayer Engine (Rust)**

Create a simple 3D multiplayer game where two players can move around a shared space and see each other.

**Tech stack:**
- Rust
- wgpu for rendering
- winit for windowing/input
- Raw UDP for networking

**Milestone 1 deliverable:**

A single Rust project with two binaries: `client` and `server`.

**Client:**
- Opens a window with a 3D scene
- Renders a ground plane (flat grid or solid color)
- Renders each player as a simple humanoid or distinct shape (not just a cube) so players can see which direction others are facing
- Local player: WASD to move, mouse to look around
- Sends player position/rotation to server via UDP
- Receives other players' positions/rotations from server and renders them facing the correct direction
- Third-person camera behind the player so you can see your own character

**Server:**
- Headless (no window)
- Listens for UDP packets from clients
- Broadcasts each player's position/rotation to all other players
- Handles connect/disconnect (player joins, character appears; player leaves, character disappears)
- Can run locally (127.0.0.1) for solo testing or on a LAN IP for local multiplayer

**Testing scenarios to support:**
- Solo: run server + one client on same Mac
- Local co-op: server on Mac, two clients (Mac + Mac, or Mac + Windows)
- Just needs LAN play, no internet/port forwarding required for now

**Environment:**
- Add some static objects to the world: a few scattered cubes, walls, or simple structures the players can walk around
- No collision yet, players can walk through things (that's a later milestone)

**Keep it minimal:**
- No textures, just solid colors
- No physics, just direct movement
- No authentication or security
- No lag compensation yet

**Cross-platform:**
- Must compile on Mac (Metal backend) and Windows (DX12/Vulkan backend)
- Include a GitHub Actions workflow that builds both platforms on push
