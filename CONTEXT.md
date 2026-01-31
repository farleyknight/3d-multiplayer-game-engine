# Minimal 3D Multiplayer Engine (Rust)

## What We're Building
A simple 3D multiplayer game where two players can move around a shared space and see each other. Third-person camera view with humanoid characters that show facing direction. LAN-based multiplayer using raw UDP.

## Tech Stack
- **Language**: Rust
- **Rendering**: wgpu (cross-platform GPU abstraction)
- **Windowing/Input**: winit
- **Networking**: Raw UDP sockets (std::net::UdpSocket)
- **Math**: glam (3D math library)
- **Serialization**: bincode (for network packets)

## Project Structure
```
3d-multiplayer-game-engine/
├── Cargo.toml          # Workspace manifest
├── src/
│   ├── lib.rs          # Shared code (rendering, networking, types)
│   ├── bin/
│   │   ├── client.rs   # Client binary
│   │   └── server.rs   # Server binary
├── specs/              # Technical specifications
└── .github/workflows/  # CI configuration
```

## Commands

### Prerequisites
- **Rust**: Install via [rustup](https://rustup.rs/) - run `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Rust version**: Stable toolchain (run `rustup default stable`)

### Platform-Specific Notes
- **macOS**: Requires Xcode Command Line Tools (`xcode-select --install`) - provides Metal SDK for GPU rendering
- **Windows**: Requires Visual Studio Build Tools or Visual Studio with "Desktop development with C++" workload - provides compiler and DX12/Vulkan support

### Build & Run
- Build: `cargo build --release`
- Run Server: `cargo run --bin server`
- Run Client: `cargo run --bin client`
- Test: `cargo test`

## Key Decisions
- **No collision detection** - players walk through objects (future milestone)
- **No textures** - solid colors only
- **No physics engine** - direct position updates from input
- **No authentication** - any client can connect
- **No lag compensation** - simple position broadcast
- **LAN only** - no NAT traversal or internet play

## Platform Targets
- macOS: Metal backend (wgpu auto-selects)
- Windows: DX12 or Vulkan backend (wgpu auto-selects)

## References
- specs/NETWORK_PROTOCOL.md - UDP packet formats and message types
- specs/RENDERING.md - Scene structure and character model
