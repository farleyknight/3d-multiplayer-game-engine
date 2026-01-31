use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::{Duration, Instant};

use game_engine::network::{
    deserialize_server_packet, serialize_client_packet, ClientPacket, ServerPacket, DEFAULT_PORT,
};
use std::io::ErrorKind;
use game_engine::{render, types::{PlayerData, PlayerState}};
use glam::Vec3;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, WindowBuilder};

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

fn main() {
    env_logger::init();
    log::info!("Starting client v{}...", game_engine::VERSION);
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

                                    // Update camera to follow player
                                    let (camera_pos, camera_target) =
                                        calculate_camera_from_player(&player);
                                    render_state.camera.position = camera_pos;
                                    render_state.camera.target = camera_target;
                                }
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
}
