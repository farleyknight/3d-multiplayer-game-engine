use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::{Duration, Instant};

use game_engine::network::{
    deserialize_server_packet, serialize_client_packet, ClientPacket, ServerPacket, DEFAULT_PORT,
};
use std::io::ErrorKind;
use game_engine::{blocks::BlockType, raycast::raycast_voxel, render, types::{PlayerData, PlayerState}};
use glam::Vec3;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, Event, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, WindowBuilder};

/// Number of frames to render in headless mode before exiting
const HEADLESS_FRAME_COUNT: u32 = 10;

/// Target frame duration for 60fps
const FRAME_DURATION: Duration = Duration::from_micros(16_667);

/// Network update interval (20Hz = 50ms)
const NETWORK_UPDATE_INTERVAL: Duration = Duration::from_millis(50);

/// Server address for LAN multiplayer
const SERVER_ADDR: &str = "127.0.0.1:7878";

/// Mouse sensitivity for look controls
const MOUSE_SENSITIVITY: f32 = 0.003;

/// Camera distance behind player
const CAMERA_DISTANCE: f32 = 5.0;

/// Camera height above player
const CAMERA_HEIGHT: f32 = 2.0;

/// Movement speed in units per second
const MOVE_SPEED: f32 = 5.0;

/// Ground floor level - player cannot fall below this
const GROUND_Y: f32 = 0.0;

/// Gravity acceleration in units per second squared
/// Higher than real gravity (9.8) for snappier game feel
const GRAVITY: f32 = 20.0;

/// Jump velocity in units per second
/// Applied upward when player presses Space while on ground
const JUMP_VELOCITY: f32 = 8.0;

/// Maximum distance for block interaction (placement/destruction)
const BLOCK_REACH_DISTANCE: f32 = 5.0;

/// Eye height offset above player position for raycasting
const EYE_HEIGHT: f32 = 1.6;

/// Calculates camera position based on player state for third-person view.
/// Camera is positioned behind and above the player, following the player's yaw.
pub fn calculate_camera_from_player(player: &PlayerState) -> (Vec3, Vec3) {
    // Camera looks at player's head height (approximately 1 unit up from position)
    let target = player.position + Vec3::new(0.0, 1.0, 0.0);

    // Camera is behind player based on yaw
    // yaw = 0 means player faces +Z, so camera is at +Z relative to player
    let camera_offset = Vec3::new(
        player.rotation_yaw.sin() * CAMERA_DISTANCE,
        CAMERA_HEIGHT,
        player.rotation_yaw.cos() * CAMERA_DISTANCE,
    );
    let position = player.position + camera_offset;

    (position, target)
}

/// Parse command-line arguments for headless mode
fn parse_args() -> bool {
    std::env::args().any(|arg| arg == "--headless")
}

/// Run the client in headless mode (no window, offscreen rendering)
fn run_headless() {
    log::info!("Running in headless mode");

    // Create UDP socket with ephemeral port
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind UDP socket");
    socket
        .set_nonblocking(true)
        .expect("Failed to set socket to non-blocking");
    log::info!(
        "UDP socket bound to {}",
        socket.local_addr().expect("Failed to get local address")
    );

    // Initialize headless render state
    let mut render_state = match pollster::block_on(render::HeadlessRenderState::new(
        render::HEADLESS_WIDTH,
        render::HEADLESS_HEIGHT,
    )) {
        Ok(state) => state,
        Err(e) => {
            log::error!("Failed to initialize headless rendering: {}", e);
            return;
        }
    };

    log::info!(
        "Headless rendering initialized ({}x{})",
        render::HEADLESS_WIDTH,
        render::HEADLESS_HEIGHT
    );

    // Buffer for receiving network packets
    let mut recv_buf = [0u8; 1024];

    // Local player state - player_id 0 means not yet assigned by server
    let mut player = PlayerState::new(0);

    // Vertical velocity for gravity physics
    let mut velocity_y: f32 = 0.0;

    // Other players received from WorldState broadcasts
    let mut other_players: HashMap<u32, PlayerData> = HashMap::new();

    // Initialize camera to follow player from the start
    let (cam_pos, cam_target) = calculate_camera_from_player(&player);
    render_state.camera.position = cam_pos;
    render_state.camera.target = cam_target;

    let mut last_frame = Instant::now();
    let mut last_network_update = Instant::now();
    let mut frame_count = 0u32;

    // Simple loop without event loop
    while frame_count < HEADLESS_FRAME_COUNT {
        let now = Instant::now();
        let elapsed = now - last_frame;

        if elapsed >= FRAME_DURATION {
            let delta_time = elapsed.as_secs_f32();
            last_frame = now;
            frame_count += 1;

            // Apply gravity - accelerate downward
            velocity_y -= GRAVITY * delta_time;

            // Apply vertical velocity to position
            player.position.y += velocity_y * delta_time;

            // Floor collision - prevent falling below ground level
            if player.position.y < GROUND_Y {
                player.position.y = GROUND_Y;
                velocity_y = 0.0; // Reset velocity when hitting ground
            }

            // Send player update to server at 20Hz
            if now.duration_since(last_network_update) >= NETWORK_UPDATE_INTERVAL {
                let packet = ClientPacket::PlayerUpdate(player);
                if let Ok(bytes) = serialize_client_packet(&packet) {
                    match socket.send_to(&bytes, SERVER_ADDR) {
                        Ok(_) => {
                            log::debug!(
                                "Sent PlayerUpdate: pos={:?}, yaw={}",
                                player.position,
                                player.rotation_yaw
                            );
                        }
                        Err(e) => {
                            log::warn!("Failed to send packet: {}", e);
                        }
                    }
                }
                last_network_update = now;
            }

            // Receive and process server packets
            loop {
                match socket.recv_from(&mut recv_buf) {
                    Ok((len, _src_addr)) => {
                        match deserialize_server_packet(&recv_buf[..len]) {
                            Ok(packet) => match packet {
                                ServerPacket::Welcome { assigned_player_id } => {
                                    player.player_id = assigned_player_id;
                                    log::info!(
                                        "Received Welcome: assigned player_id={}",
                                        assigned_player_id
                                    );
                                }
                                ServerPacket::WorldState { players } => {
                                    other_players.clear();
                                    for p in players {
                                        if p.player_id != player.player_id {
                                            other_players.insert(p.player_id, p);
                                        }
                                    }
                                }
                                ServerPacket::PlayerLeft { player_id: left_id } => {
                                    log::info!("Player {} left the game", left_id);
                                    other_players.remove(&left_id);
                                }
                            },
                            Err(e) => {
                                log::warn!("Failed to deserialize server packet: {}", e);
                            }
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(e) => {
                        log::warn!("recv_from error: {}", e);
                        break;
                    }
                }
            }

            // Collect other players' positions for rendering
            let other_players_vec: Vec<(glam::Vec3, f32)> = other_players
                .values()
                .map(|p| (p.position, p.rotation_yaw))
                .collect();

            // Render to offscreen texture
            render_state.render_with_players(player.position, player.rotation_yaw, &other_players_vec);

            log::debug!("Headless frame {} rendered", frame_count);
        }

        // Sleep briefly to avoid busy-waiting
        std::thread::sleep(Duration::from_millis(1));
    }

    log::info!("Headless mode completed {} frames", frame_count);
}

fn main() {
    env_logger::init();
    log::info!("Starting client v{}...", game_engine::VERSION);

    // Check for headless mode
    if parse_args() {
        run_headless();
        return;
    }

    log::info!("Window title: {}", render::WINDOW_TITLE);
    log::info!("Connecting to server on port {}", DEFAULT_PORT);

    // Create UDP socket with ephemeral port
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind UDP socket");
    socket
        .set_nonblocking(true)
        .expect("Failed to set socket to non-blocking");
    log::info!(
        "UDP socket bound to {}",
        socket.local_addr().expect("Failed to get local address")
    );

    let event_loop = EventLoop::new().expect("Failed to create event loop");

    let window = Arc::new(
        WindowBuilder::new()
            .with_title(render::WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(800, 600))
            .build(&event_loop)
            .expect("Failed to create window"),
    );

    let mut render_state = pollster::block_on(render::RenderState::new(window.clone()));
    let mut last_frame = Instant::now();
    let mut last_network_update = Instant::now();

    // Buffer for receiving network packets
    let mut recv_buf = [0u8; 1024];

    // Local player state - player_id 0 means not yet assigned by server
    let mut player = PlayerState::new(0);

    // Vertical velocity for gravity physics
    let mut velocity_y: f32 = 0.0;

    // Other players received from WorldState broadcasts
    let mut other_players: HashMap<u32, PlayerData> = HashMap::new();

    // Initialize camera to follow player from the start
    let (cam_pos, cam_target) = calculate_camera_from_player(&player);
    render_state.camera.position = cam_pos;
    render_state.camera.target = cam_target;

    // Track whether cursor is captured
    let mut cursor_captured = false;

    // Track WASD key press state for movement
    let mut w_pressed = false;
    let mut a_pressed = false;
    let mut s_pressed = false;
    let mut d_pressed = false;
    let mut space_pressed = false;

    // Track camera pitch for vertical look (used for raycasting)
    let mut pitch: f32 = 0.0;

    // Track mouse button clicks for block interaction (single-shot flags)
    let mut left_mouse_clicked = false;
    let mut right_mouse_clicked = false;

    // Selected block type for placement (default: Cobblestone)
    let selected_block_type = BlockType::Cobblestone;

    log::info!("Window created and wgpu initialized");

    event_loop
        .run(move |event, elwt| {
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        log::info!("Close requested, exiting");
                        elwt.exit();
                    }
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key: PhysicalKey::Code(KeyCode::Escape),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    } => {
                        // Release cursor and exit
                        let _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                        cursor_captured = false;
                        log::info!("Escape pressed, exiting");
                        elwt.exit();
                    }
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key: PhysicalKey::Code(key_code),
                                state,
                                ..
                            },
                        ..
                    } => {
                        let pressed = state == ElementState::Pressed;
                        match key_code {
                            KeyCode::KeyW => w_pressed = pressed,
                            KeyCode::KeyA => a_pressed = pressed,
                            KeyCode::KeyS => s_pressed = pressed,
                            KeyCode::KeyD => d_pressed = pressed,
                            KeyCode::Space => space_pressed = pressed,
                            _ => {}
                        }
                    }
                    WindowEvent::Focused(focused) => {
                        if focused {
                            // Capture cursor on focus
                            // Try Locked first (best for FPS games), fall back to Confined
                            let grab_result = window
                                .set_cursor_grab(CursorGrabMode::Locked)
                                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));

                            if grab_result.is_ok() {
                                window.set_cursor_visible(false);
                                cursor_captured = true;
                                log::info!("Cursor captured");
                            } else {
                                log::warn!("Failed to capture cursor: {:?}", grab_result);
                            }
                        } else {
                            // Release cursor when window loses focus
                            let _ = window.set_cursor_grab(CursorGrabMode::None);
                            window.set_cursor_visible(true);
                            cursor_captured = false;
                            log::info!("Cursor released (window lost focus)");
                        }
                    }
                    WindowEvent::Resized(physical_size) => {
                        log::debug!("Resized to {:?}", physical_size);
                        render_state.resize(physical_size);
                    }
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button,
                        ..
                    } => {
                        if cursor_captured {
                            match button {
                                MouseButton::Left => {
                                    left_mouse_clicked = true;
                                }
                                MouseButton::Right => {
                                    right_mouse_clicked = true;
                                }
                                _ => {}
                            }
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let elapsed = now - last_frame;

                        if elapsed >= FRAME_DURATION {
                            let delta_time = elapsed.as_secs_f32();
                            last_frame = now;

                            // Process movement input
                            if cursor_captured {
                                // Calculate forward direction based on player yaw
                                // Player faces -Z when yaw = 0, so forward is negative of camera offset direction
                                let forward = Vec3::new(
                                    -player.rotation_yaw.sin(),
                                    0.0,
                                    -player.rotation_yaw.cos(),
                                );
                                // Right is perpendicular to forward (90 degrees clockwise in XZ plane)
                                let right = Vec3::new(forward.z, 0.0, -forward.x);

                                let mut movement = Vec3::ZERO;
                                if w_pressed {
                                    movement += forward;
                                }
                                if s_pressed {
                                    movement -= forward;
                                }
                                if a_pressed {
                                    movement -= right;
                                }
                                if d_pressed {
                                    movement += right;
                                }

                                // Normalize to prevent faster diagonal movement
                                if movement.length_squared() > 0.0 {
                                    movement = movement.normalize() * MOVE_SPEED * delta_time;
                                    player.position += movement;
                                }
                            }

                            // Jump when space is pressed and player is on ground
                            if space_pressed && player.position.y <= GROUND_Y {
                                velocity_y = JUMP_VELOCITY;
                            }

                            // Apply gravity - accelerate downward
                            velocity_y -= GRAVITY * delta_time;

                            // Apply vertical velocity to position
                            player.position.y += velocity_y * delta_time;

                            // Floor collision - prevent falling below ground level
                            if player.position.y < GROUND_Y {
                                player.position.y = GROUND_Y;
                                velocity_y = 0.0; // Reset velocity when hitting ground
                            }

                            // Handle block interaction on mouse click
                            if left_mouse_clicked || right_mouse_clicked {
                                // Calculate ray origin: player position + eye height offset
                                let ray_origin = player.position + Vec3::new(0.0, EYE_HEIGHT, 0.0);

                                // Calculate ray direction from yaw and pitch using spherical coordinates
                                // At yaw=0, pitch=0: looking toward -Z direction
                                let ray_direction = Vec3::new(
                                    -player.rotation_yaw.sin() * pitch.cos(),
                                    pitch.sin(),
                                    -player.rotation_yaw.cos() * pitch.cos(),
                                ).normalize();

                                // Perform raycast against voxel world
                                if let Some(hit) = raycast_voxel(
                                    &render_state.voxel_world,
                                    ray_origin,
                                    ray_direction,
                                    BLOCK_REACH_DISTANCE,
                                ) {
                                    let (bx, by, bz) = hit.block_pos;
                                    let (nx, ny, nz) = hit.face_normal;

                                    if left_mouse_clicked {
                                        // Left click: destroy block at hit position
                                        render_state.voxel_world.remove_block(bx, by, bz);
                                        render_state.rebuild_voxel_mesh();
                                        log::debug!(
                                            "Destroyed block at ({}, {}, {})",
                                            bx, by, bz
                                        );
                                    } else if right_mouse_clicked {
                                        // Right click: place block adjacent to hit face
                                        let (px, py, pz) = (bx + nx, by + ny, bz + nz);
                                        render_state.voxel_world.set_block(px, py, pz, selected_block_type);
                                        render_state.rebuild_voxel_mesh();
                                        log::debug!(
                                            "Placed {:?} block at ({}, {}, {})",
                                            selected_block_type,
                                            px, py, pz
                                        );
                                    }
                                }

                                // Clear click flags after processing
                                left_mouse_clicked = false;
                                right_mouse_clicked = false;
                            }

                            // Update camera to follow player
                            let (camera_pos, camera_target) =
                                calculate_camera_from_player(&player);
                            render_state.camera.position = camera_pos;
                            render_state.camera.target = camera_target;

                            // Send player update to server at 20Hz
                            if now.duration_since(last_network_update) >= NETWORK_UPDATE_INTERVAL {
                                let packet = ClientPacket::PlayerUpdate(player);
                                if let Ok(bytes) = serialize_client_packet(&packet) {
                                    match socket.send_to(&bytes, SERVER_ADDR) {
                                        Ok(_) => {
                                            log::debug!(
                                                "Sent PlayerUpdate: pos={:?}, yaw={}",
                                                player.position,
                                                player.rotation_yaw
                                            );
                                        }
                                        Err(e) => {
                                            log::warn!("Failed to send packet: {}", e);
                                        }
                                    }
                                }
                                last_network_update = now;
                            }

                            // Receive and process server packets
                            loop {
                                match socket.recv_from(&mut recv_buf) {
                                    Ok((len, _src_addr)) => {
                                        match deserialize_server_packet(&recv_buf[..len]) {
                                            Ok(packet) => match packet {
                                                ServerPacket::Welcome { assigned_player_id } => {
                                                    player.player_id = assigned_player_id;
                                                    log::info!(
                                                        "Received Welcome: assigned player_id={}",
                                                        assigned_player_id
                                                    );
                                                }
                                                ServerPacket::WorldState { players } => {
                                                    // Update other_players map with received world state
                                                    // Only include players that aren't the local player
                                                    // Track which players we knew about before
                                                    let known_ids: std::collections::HashSet<u32> =
                                                        other_players.keys().copied().collect();
                                                    other_players.clear();
                                                    for p in players {
                                                        if p.player_id != player.player_id {
                                                            // Log when a new player joins
                                                            if !known_ids.contains(&p.player_id) {
                                                                log::info!(
                                                                    "Player {} joined the game",
                                                                    p.player_id
                                                                );
                                                            }
                                                            log::debug!(
                                                                "Other player {}: pos=({:.2}, {:.2}, {:.2}), yaw={:.2}",
                                                                p.player_id,
                                                                p.position.x,
                                                                p.position.y,
                                                                p.position.z,
                                                                p.rotation_yaw
                                                            );
                                                            other_players.insert(p.player_id, p);
                                                        }
                                                    }
                                                }
                                                ServerPacket::PlayerLeft { player_id: left_id } => {
                                                    log::info!("Player {} left the game", left_id);
                                                    other_players.remove(&left_id);
                                                }
                                            },
                                            Err(e) => {
                                                log::warn!("Failed to deserialize server packet: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                                        // No more data available
                                        break;
                                    }
                                    Err(e) => {
                                        log::warn!("recv_from error: {}", e);
                                        break;
                                    }
                                }
                            }

                            // Collect other players' positions for rendering
                            let other_players_vec: Vec<(glam::Vec3, f32)> = other_players
                                .values()
                                .map(|p| (p.position, p.rotation_yaw))
                                .collect();

                            match render_state.render_with_players(
                                player.position,
                                player.rotation_yaw,
                                &other_players_vec,
                            ) {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost) => {
                                    render_state.resize(render_state.size);
                                }
                                Err(wgpu::SurfaceError::OutOfMemory) => {
                                    log::error!("Out of memory, exiting");
                                    elwt.exit();
                                }
                                Err(e) => {
                                    log::warn!("Render error: {:?}", e);
                                }
                            }
                        }

                        // Schedule next frame
                        let next_frame = last_frame + FRAME_DURATION;
                        elwt.set_control_flow(ControlFlow::WaitUntil(next_frame));
                        window.request_redraw();
                    }
                    _ => {}
                },
                Event::DeviceEvent {
                    event: DeviceEvent::MouseMotion { delta },
                    ..
                } => {
                    if cursor_captured {
                        // Update player yaw based on mouse X movement
                        player.rotation_yaw -= delta.0 as f32 * MOUSE_SENSITIVITY;

                        // Update pitch based on mouse Y movement (for raycasting)
                        // Clamp to ±89 degrees to prevent gimbal lock at poles
                        pitch -= delta.1 as f32 * MOUSE_SENSITIVITY;
                        pitch = pitch.clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());

                        // Update camera based on new player orientation
                        let (camera_pos, camera_target) = calculate_camera_from_player(&player);
                        render_state.camera.position = camera_pos;
                        render_state.camera.target = camera_target;
                    }
                }
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .expect("Event loop error");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_camera_position_at_zero_yaw() {
        let player = PlayerState::with_transform(1, Vec3::ZERO, 0.0);
        let (camera_pos, camera_target) = calculate_camera_from_player(&player);

        // At yaw = 0, camera should be behind player on +Z axis
        // sin(0) = 0, cos(0) = 1, so camera is at (0, 2, 5)
        assert!((camera_pos.x - 0.0).abs() < 0.001, "Camera X should be 0");
        assert!((camera_pos.y - CAMERA_HEIGHT).abs() < 0.001, "Camera Y should be CAMERA_HEIGHT");
        assert!((camera_pos.z - CAMERA_DISTANCE).abs() < 0.001, "Camera Z should be CAMERA_DISTANCE");

        // Target should be at player position + (0, 1, 0)
        assert_eq!(camera_target, Vec3::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn test_camera_position_at_90_degrees() {
        let player = PlayerState::with_transform(1, Vec3::ZERO, PI / 2.0);
        let (camera_pos, _) = calculate_camera_from_player(&player);

        // At yaw = PI/2, sin = 1, cos = 0, so camera is at (5, 2, 0)
        assert!((camera_pos.x - CAMERA_DISTANCE).abs() < 0.001, "Camera X should be CAMERA_DISTANCE");
        assert!((camera_pos.y - CAMERA_HEIGHT).abs() < 0.001, "Camera Y should be CAMERA_HEIGHT");
        assert!(camera_pos.z.abs() < 0.001, "Camera Z should be 0");
    }

    #[test]
    fn test_camera_position_at_180_degrees() {
        let player = PlayerState::with_transform(1, Vec3::ZERO, PI);
        let (camera_pos, _) = calculate_camera_from_player(&player);

        // At yaw = PI, sin = 0, cos = -1, so camera is at (0, 2, -5)
        assert!(camera_pos.x.abs() < 0.001, "Camera X should be 0");
        assert!((camera_pos.y - CAMERA_HEIGHT).abs() < 0.001, "Camera Y should be CAMERA_HEIGHT");
        assert!((camera_pos.z + CAMERA_DISTANCE).abs() < 0.001, "Camera Z should be -CAMERA_DISTANCE");
    }

    #[test]
    fn test_camera_follows_player_position() {
        let player = PlayerState::with_transform(1, Vec3::new(10.0, 0.0, -5.0), 0.0);
        let (camera_pos, camera_target) = calculate_camera_from_player(&player);

        // Camera should be offset from player position
        assert!((camera_pos.x - 10.0).abs() < 0.001, "Camera X should follow player");
        assert!((camera_pos.z - (-5.0 + CAMERA_DISTANCE)).abs() < 0.001, "Camera Z should follow player");

        // Target should be 1 unit above player position
        assert_eq!(camera_target, Vec3::new(10.0, 1.0, -5.0));
    }

    #[test]
    fn test_camera_height_is_constant() {
        let player1 = PlayerState::with_transform(1, Vec3::ZERO, 0.0);
        let player2 = PlayerState::with_transform(1, Vec3::ZERO, PI / 4.0);
        let player3 = PlayerState::with_transform(1, Vec3::ZERO, PI);

        let (pos1, _) = calculate_camera_from_player(&player1);
        let (pos2, _) = calculate_camera_from_player(&player2);
        let (pos3, _) = calculate_camera_from_player(&player3);

        // Camera height should be constant regardless of yaw
        assert!((pos1.y - CAMERA_HEIGHT).abs() < 0.001);
        assert!((pos2.y - CAMERA_HEIGHT).abs() < 0.001);
        assert!((pos3.y - CAMERA_HEIGHT).abs() < 0.001);
    }

    #[test]
    fn test_camera_distance_is_constant() {
        let player1 = PlayerState::with_transform(1, Vec3::ZERO, 0.0);
        let player2 = PlayerState::with_transform(1, Vec3::ZERO, PI / 3.0);
        let player3 = PlayerState::with_transform(1, Vec3::ZERO, 2.0 * PI / 3.0);

        let (pos1, _) = calculate_camera_from_player(&player1);
        let (pos2, _) = calculate_camera_from_player(&player2);
        let (pos3, _) = calculate_camera_from_player(&player3);

        // Horizontal distance should be constant regardless of yaw
        let dist1 = (pos1.x.powi(2) + pos1.z.powi(2)).sqrt();
        let dist2 = (pos2.x.powi(2) + pos2.z.powi(2)).sqrt();
        let dist3 = (pos3.x.powi(2) + pos3.z.powi(2)).sqrt();

        assert!((dist1 - CAMERA_DISTANCE).abs() < 0.001);
        assert!((dist2 - CAMERA_DISTANCE).abs() < 0.001);
        assert!((dist3 - CAMERA_DISTANCE).abs() < 0.001);
    }

    #[test]
    fn test_mouse_sensitivity_reasonable() {
        // Mouse sensitivity should be positive and small
        assert!(MOUSE_SENSITIVITY > 0.0);
        assert!(MOUSE_SENSITIVITY < 0.1);
    }

    #[test]
    fn test_move_speed_reasonable() {
        // Movement speed should be positive and reasonable
        assert!(MOVE_SPEED > 0.0);
        assert!(MOVE_SPEED < 100.0);
    }

    /// Helper to calculate forward direction from yaw (same logic as in game loop)
    fn forward_from_yaw(yaw: f32) -> Vec3 {
        Vec3::new(-yaw.sin(), 0.0, -yaw.cos())
    }

    /// Helper to calculate right direction from forward
    fn right_from_forward(forward: Vec3) -> Vec3 {
        Vec3::new(forward.z, 0.0, -forward.x)
    }

    #[test]
    fn test_forward_direction_at_zero_yaw() {
        // At yaw = 0, player faces -Z direction
        let forward = forward_from_yaw(0.0);
        assert!(forward.x.abs() < 0.001, "Forward X should be 0 at yaw 0");
        assert!(forward.y.abs() < 0.001, "Forward Y should be 0");
        assert!((forward.z + 1.0).abs() < 0.001, "Forward Z should be -1 at yaw 0");
    }

    #[test]
    fn test_forward_direction_at_90_degrees() {
        // At yaw = PI/2, player faces -X direction
        let forward = forward_from_yaw(PI / 2.0);
        assert!((forward.x + 1.0).abs() < 0.001, "Forward X should be -1 at yaw PI/2");
        assert!(forward.y.abs() < 0.001, "Forward Y should be 0");
        assert!(forward.z.abs() < 0.001, "Forward Z should be 0 at yaw PI/2");
    }

    #[test]
    fn test_forward_direction_at_180_degrees() {
        // At yaw = PI, player faces +Z direction
        let forward = forward_from_yaw(PI);
        assert!(forward.x.abs() < 0.001, "Forward X should be 0 at yaw PI");
        assert!(forward.y.abs() < 0.001, "Forward Y should be 0");
        assert!((forward.z - 1.0).abs() < 0.001, "Forward Z should be 1 at yaw PI");
    }

    #[test]
    fn test_right_is_perpendicular_to_forward() {
        for yaw in [0.0, PI / 4.0, PI / 2.0, PI, 3.0 * PI / 2.0] {
            let forward = forward_from_yaw(yaw);
            let right = right_from_forward(forward);

            // Right should be perpendicular to forward (dot product = 0)
            let dot = forward.dot(right);
            assert!(dot.abs() < 0.001, "Right should be perpendicular to forward at yaw {}", yaw);

            // Both should be unit vectors in XZ plane
            assert!((forward.length() - 1.0).abs() < 0.001);
            assert!((right.length() - 1.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_right_direction_at_zero_yaw() {
        // At yaw = 0, forward is (0, 0, -1), so right should be (-1, 0, 0)
        let forward = forward_from_yaw(0.0);
        let right = right_from_forward(forward);
        assert!((right.x + 1.0).abs() < 0.001, "Right X should be -1 at yaw 0");
        assert!(right.y.abs() < 0.001, "Right Y should be 0");
        assert!(right.z.abs() < 0.001, "Right Z should be 0 at yaw 0");
    }

    #[test]
    fn test_diagonal_movement_normalized() {
        let forward = forward_from_yaw(0.0);
        let right = right_from_forward(forward);

        // Moving forward + right should give a normalized diagonal
        let diagonal = (forward + right).normalize();
        assert!((diagonal.length() - 1.0).abs() < 0.001, "Diagonal movement should be normalized");
    }

    #[test]
    fn test_network_update_interval_is_50ms() {
        // 20Hz = 1000ms / 20 = 50ms
        assert_eq!(
            NETWORK_UPDATE_INTERVAL,
            Duration::from_millis(50),
            "Network update interval should be 50ms (20Hz)"
        );
    }

    #[test]
    fn test_server_addr_is_localhost() {
        assert_eq!(SERVER_ADDR, "127.0.0.1:7878");
    }

    #[test]
    fn test_gravity_constant_reasonable() {
        // Gravity should be positive and reasonable for game feel
        // Standard gravity is 9.8 m/s², games often use 10-30 for snappier feel
        assert!(GRAVITY > 0.0, "Gravity should be positive");
        assert!(GRAVITY >= 5.0, "Gravity should be at least 5.0 for noticeable effect");
        assert!(GRAVITY <= 50.0, "Gravity should not be excessively high");
    }

    #[test]
    fn test_gravity_velocity_increases_over_time() {
        // Simulate gravity for several frames
        let mut velocity_y: f32 = 0.0;
        let delta_time = 1.0 / 60.0; // 60 FPS

        // After one frame, velocity should be negative (falling)
        velocity_y -= GRAVITY * delta_time;
        assert!(velocity_y < 0.0, "After gravity, velocity should be negative");

        let velocity_after_one = velocity_y;

        // After another frame, velocity should be more negative (accelerating)
        velocity_y -= GRAVITY * delta_time;
        assert!(
            velocity_y < velocity_after_one,
            "Velocity should increase in magnitude over time"
        );
    }

    #[test]
    fn test_floor_collision_resets_velocity() {
        // Simulate a falling player hitting the ground
        let mut position_y = 1.0; // Start above ground
        let mut velocity_y: f32 = -10.0; // Falling downward
        let delta_time = 0.5; // Large dt to ensure we go below ground

        // Apply velocity
        position_y += velocity_y * delta_time;

        // Position should now be below GROUND_Y
        assert!(position_y < GROUND_Y, "Position should be below ground before collision check");

        // Apply floor collision
        if position_y < GROUND_Y {
            position_y = GROUND_Y;
            velocity_y = 0.0;
        }

        // Verify collision behavior
        assert_eq!(position_y, GROUND_Y, "Position should be clamped to GROUND_Y");
        assert_eq!(velocity_y, 0.0, "Velocity should be reset to 0 on collision");
    }

    #[test]
    fn test_ground_level_is_zero() {
        // GROUND_Y should be 0 so player stands at origin height
        assert_eq!(GROUND_Y, 0.0, "Ground level should be at Y=0");
    }

    /// Helper to calculate ray direction from yaw and pitch (same logic as in game loop)
    fn ray_direction_from_yaw_pitch(yaw: f32, pitch: f32) -> Vec3 {
        Vec3::new(
            -yaw.sin() * pitch.cos(),
            pitch.sin(),
            -yaw.cos() * pitch.cos(),
        ).normalize()
    }

    #[test]
    fn test_ray_direction_at_zero_yaw_zero_pitch() {
        // At yaw=0, pitch=0: looking toward -Z direction
        let dir = ray_direction_from_yaw_pitch(0.0, 0.0);
        assert!(dir.x.abs() < 0.001, "Ray X should be 0 at yaw=0, pitch=0");
        assert!(dir.y.abs() < 0.001, "Ray Y should be 0 at pitch=0");
        assert!((dir.z + 1.0).abs() < 0.001, "Ray Z should be -1 at yaw=0, pitch=0");
    }

    #[test]
    fn test_ray_direction_at_90_degrees_yaw() {
        // At yaw=PI/2, pitch=0: looking toward -X direction
        let dir = ray_direction_from_yaw_pitch(PI / 2.0, 0.0);
        assert!((dir.x + 1.0).abs() < 0.001, "Ray X should be -1 at yaw=PI/2, pitch=0");
        assert!(dir.y.abs() < 0.001, "Ray Y should be 0 at pitch=0");
        assert!(dir.z.abs() < 0.001, "Ray Z should be 0 at yaw=PI/2, pitch=0");
    }

    #[test]
    fn test_ray_direction_looking_up() {
        // At yaw=0, pitch=PI/4 (45 degrees up): looking upward at -Z direction
        let dir = ray_direction_from_yaw_pitch(0.0, PI / 4.0);
        assert!(dir.x.abs() < 0.001, "Ray X should be 0 at yaw=0");
        assert!(dir.y > 0.5, "Ray Y should be positive when looking up");
        assert!(dir.z < 0.0, "Ray Z should be negative at yaw=0");
        assert!((dir.length() - 1.0).abs() < 0.001, "Ray should be normalized");
    }

    #[test]
    fn test_ray_direction_looking_down() {
        // At yaw=0, pitch=-PI/4 (45 degrees down): looking downward at -Z direction
        let dir = ray_direction_from_yaw_pitch(0.0, -PI / 4.0);
        assert!(dir.x.abs() < 0.001, "Ray X should be 0 at yaw=0");
        assert!(dir.y < -0.5, "Ray Y should be negative when looking down");
        assert!(dir.z < 0.0, "Ray Z should be negative at yaw=0");
        assert!((dir.length() - 1.0).abs() < 0.001, "Ray should be normalized");
    }

    #[test]
    fn test_ray_direction_is_normalized() {
        // Test various yaw/pitch combinations produce unit vectors
        for yaw in [0.0, PI / 4.0, PI / 2.0, PI, 3.0 * PI / 2.0] {
            for pitch in [-PI / 3.0, -PI / 6.0, 0.0, PI / 6.0, PI / 3.0] {
                let dir = ray_direction_from_yaw_pitch(yaw, pitch);
                assert!(
                    (dir.length() - 1.0).abs() < 0.001,
                    "Ray direction should be normalized at yaw={}, pitch={}",
                    yaw, pitch
                );
            }
        }
    }

    #[test]
    fn test_pitch_clamping_bounds() {
        // Verify pitch clamping bounds are correct (±89 degrees)
        let max_pitch = 89.0_f32.to_radians();
        let min_pitch = -89.0_f32.to_radians();

        // Test that clamping produces expected results
        let over_max = 100.0_f32.to_radians();
        let under_min = -100.0_f32.to_radians();

        let clamped_over = over_max.clamp(min_pitch, max_pitch);
        let clamped_under = under_min.clamp(min_pitch, max_pitch);

        assert!((clamped_over - max_pitch).abs() < 0.001);
        assert!((clamped_under - min_pitch).abs() < 0.001);
    }

    #[test]
    fn test_block_placement_position_from_face_normal() {
        use glam::IVec3;

        // Simulate block placement: position = hit.block_pos + hit.face_normal
        let hit_block = IVec3::new(5, 3, 10);

        // Hitting top face (normal = +Y): place block above
        let top_normal = IVec3::new(0, 1, 0);
        let place_pos = hit_block + top_normal;
        assert_eq!(place_pos, IVec3::new(5, 4, 10));

        // Hitting bottom face (normal = -Y): place block below
        let bottom_normal = IVec3::new(0, -1, 0);
        let place_pos = hit_block + bottom_normal;
        assert_eq!(place_pos, IVec3::new(5, 2, 10));

        // Hitting north face (normal = -Z): place block in front
        let north_normal = IVec3::new(0, 0, -1);
        let place_pos = hit_block + north_normal;
        assert_eq!(place_pos, IVec3::new(5, 3, 9));

        // Hitting south face (normal = +Z): place block behind
        let south_normal = IVec3::new(0, 0, 1);
        let place_pos = hit_block + south_normal;
        assert_eq!(place_pos, IVec3::new(5, 3, 11));

        // Hitting west face (normal = -X): place block to west
        let west_normal = IVec3::new(-1, 0, 0);
        let place_pos = hit_block + west_normal;
        assert_eq!(place_pos, IVec3::new(4, 3, 10));

        // Hitting east face (normal = +X): place block to east
        let east_normal = IVec3::new(1, 0, 0);
        let place_pos = hit_block + east_normal;
        assert_eq!(place_pos, IVec3::new(6, 3, 10));
    }

    #[test]
    fn test_block_reach_distance_reasonable() {
        // Block reach should be positive and within reasonable bounds
        assert!(BLOCK_REACH_DISTANCE > 0.0, "Block reach should be positive");
        assert!(BLOCK_REACH_DISTANCE <= 10.0, "Block reach should not be too far");
        assert!(BLOCK_REACH_DISTANCE >= 3.0, "Block reach should allow at least 3 blocks");
    }

    #[test]
    fn test_eye_height_reasonable() {
        // Eye height should be positive and at reasonable human height
        assert!(EYE_HEIGHT > 0.0, "Eye height should be positive");
        assert!(EYE_HEIGHT <= 2.0, "Eye height should be at most 2 units");
        assert!(EYE_HEIGHT >= 1.0, "Eye height should be at least 1 unit");
    }
}
