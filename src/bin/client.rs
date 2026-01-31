use std::sync::Arc;
use std::time::{Duration, Instant};

use game_engine::{network, render, types::PlayerState};
use glam::Vec3;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, WindowBuilder};

/// Target frame duration for 60fps
const FRAME_DURATION: Duration = Duration::from_micros(16_667);

/// Mouse sensitivity for look controls
const MOUSE_SENSITIVITY: f32 = 0.003;

/// Camera distance behind player
const CAMERA_DISTANCE: f32 = 5.0;

/// Camera height above player
const CAMERA_HEIGHT: f32 = 2.0;

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
    log::info!("Connecting to server on port {}", network::DEFAULT_PORT);

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

    // Local player state - player_id 0 means not yet assigned by server
    let mut player = PlayerState::new(0);

    // Track whether cursor is captured
    let mut cursor_captured = false;

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
                            last_frame = now;

                            match render_state.render() {
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
}
