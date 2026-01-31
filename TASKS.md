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
- [x] End-to-end test: two clients see each other move → Two clients on same machine can see each other's movement

## Cross-Platform & CI
- [x] Add GitHub Actions workflow for Mac and Windows builds → Push to repo, both builds succeed in Actions

## Local Build & Testing Infrastructure
- [x] Add: Document local compilation steps in CONTEXT.md → Commands section includes `cargo build` prerequisites and platform-specific notes
- [x] Add: Create `tests/screenshots/` directory with .gitkeep → `ls tests/screenshots/` shows the directory exists
- [x] Add: Screenshot capture utility function in test helpers → Function takes wgpu surface/texture and saves PNG to specified path

## Single-Player Integration Tests with Screenshots
- [x] Add: Headless rendering mode for client → Client can run with `--headless` flag without opening a window, using wgpu's offscreen rendering
- [x] Add: Integration test for player movement → Test spawns client, moves WASD, captures screenshots at each position in `tests/screenshots/movement_*.png`
- [x] Add: Integration test for player jumping → Test spawns client, presses jump, captures screenshots showing player airborne in `tests/screenshots/jump_*.png`
- [x] Add: Integration test for box interaction → Test spawns client, moves boxes, captures screenshots showing boxes in new positions in `tests/screenshots/box_*.png`
- [x] Add: Integration test runner script → `cargo test --test integration` runs all visual tests and generates screenshots

## Documentation & Deployment
- [x] Add: Create README.md with project overview → README.md exists with build instructions, features, and screenshots section
- [x] Add: Push repository to GitHub → `git remote -v` shows GitHub origin and `git push` succeeds
- [x] Verify: GitHub Actions builds the game correctly → GitHub Actions CI shows green checkmarks on both macOS and Windows

## 3D World - Floor & Physics
- [ ] Add: Ground floor for player to stand and jump from → Player spawns on a solid floor at y=0, cannot fall through
- [ ] Add: Basic gravity system → Player falls when not on ground, velocity increases over time
- [ ] Add: Player jumping mechanic → Space key makes player jump, player returns to ground via gravity
- [ ] Add: Floor collision detection → Player stops at y=0 (floor level), cannot fall below ground

## 3D World - Textures & Lighting
- [ ] Add: Texture loading system for PNG files → Load textures from `textures/default-textures/textures/blocks/` directory
- [ ] Add: UV mapping for block faces → Each face of a cube can have a texture applied
- [ ] Add: Textured block rendering → Blocks render with Minecraft textures instead of solid colors
- [ ] Add: Basic directional lighting → Scene has a sun-like light source casting basic shadows/shading
- [ ] Add: Ambient lighting → Objects have base illumination even in shadow

## 3D World - Minecraft-like Environment
- [ ] Add: Block type system → Define block types (grass, dirt, stone, cobblestone, wood) with associated textures
- [ ] Add: Voxel world generation → Generate a simple flat world with grass on top, dirt below, stone deeper
- [ ] Add: Block placement/destruction → Player can place and break blocks (left click destroy, right click place)
- [ ] Add: Block face culling → Don't render faces between adjacent solid blocks (performance optimization)
- [ ] Add: Chunk-based world storage → World divided into 16x16x16 chunks for efficient rendering/storage

## Integration Tests - Video Recording
- [ ] Add: Video recording capability for tests → Tests can record gameplay to MP4/WebM files in `tests/videos/`
- [ ] Add: Frame capture to video encoder → Capture rendered frames and encode to video format
- [ ] Add: Integration test with video output for movement → `tests/videos/movement_test.mp4` shows player WASD movement
- [ ] Add: Integration test with video output for jumping → `tests/videos/jump_test.mp4` shows player jumping
- [ ] Add: Integration test with video output for block interaction → `tests/videos/block_interaction.mp4` shows block place/destroy
