# Tasks

## Project Setup
- [x] Initialize Cargo workspace with dependencies (wgpu, winit, glam, bincode) → `cargo check` succeeds with no errors
- [x] Create basic project structure with lib.rs and bin stubs → `cargo build` compiles both `client` and `server` binaries

## Core Infrastructure
- [x] Implement shared types (PlayerState, packet types, constants) → `cargo test` passes for serialization round-trip tests
- [x] Implement network packet serialization/deserialization → Unit tests verify encode/decode of all packet types → Note: needed glam "serde" feature

## Server Implementation
- [x] Create UDP server that listens and tracks connected clients → Server starts, logs "Listening on 0.0.0.0:7878"
- [x] Implement player state management (join, update, timeout) → Integration test: fake client sends packets, server tracks player
- [x] Implement world state broadcast to all clients → Server logs broadcast activity when clients connected

## Client Rendering Foundation
- [x] Set up wgpu + winit window with basic render loop → Window opens, clears to sky blue, runs at 60fps
- [x] Implement perspective camera with view/projection matrices → Cube rendered at origin appears in 3D perspective
- [x] Create simple colored mesh rendering (cube primitive) → Colored cube renders correctly on screen → Note: was implemented with camera task

## Client Scene
- [x] Render ground plane at y=0 → Gray 50×50 quad visible in scene
- [x] Render humanoid character from basic shapes → Multi-part character (head, torso, arms, legs) visible
- [x] Add static environment objects (cubes, walls) → 6+ static objects scattered around the scene

## Client Input & Movement
- [x] Implement mouse capture and look controls → Mouse captured, moving mouse rotates camera/player yaw
- [x] Implement WASD movement relative to facing direction → Character moves forward/back/strafe based on input

## Client Camera
- [x] Implement third-person camera following player → Camera stays 5 units behind, 2 units up, follows rotation

## Client Networking
- [x] Send PlayerUpdate packets to server at 20Hz → Wireshark/server logs show packets arriving
- [x] Receive and parse WorldState packets from server → Console logs other player positions when received
- [x] Render other players from received world state → Second client's character visible and moves correctly

## Integration
- [x] Handle player join/leave (character appears/disappears) → Start/stop second client, character appears/disappears on first
- [ ] End-to-end test: two clients see each other move → Two clients on same machine can see each other's movement

## Cross-Platform & CI
- [ ] Add GitHub Actions workflow for Mac and Windows builds → Push to repo, both builds succeed in Actions
