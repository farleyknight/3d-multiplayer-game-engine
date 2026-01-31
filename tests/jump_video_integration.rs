//! Integration test for player jumping with video recording.
//! Tests jump physics (vertical velocity + gravity) and records video output.

use game_engine::render::HeadlessRenderState;
use game_engine::types::PlayerState;
use glam::Vec3;
use std::path::Path;

#[cfg(feature = "test-helpers")]
use game_engine::test_helpers::VideoRecorder;

/// Camera distance behind player (matches client.rs)
const CAMERA_DISTANCE: f32 = 5.0;

/// Camera height above player (matches client.rs)
const CAMERA_HEIGHT: f32 = 2.0;

/// Jump physics constants
const JUMP_VELOCITY: f32 = 8.0; // Initial upward velocity (units/sec)
const GRAVITY: f32 = -20.0; // Downward acceleration (units/sec²)
const GROUND_Y: f32 = 0.0; // Ground level

/// Time step for physics simulation (seconds)
const PHYSICS_DT: f32 = 1.0 / 60.0;

/// Number of frames to capture while on ground before/after jump
const GROUND_FRAMES: u32 = 15;

/// Calculates camera position based on player state for third-person view.
/// Camera is positioned behind and above the player, following the player's yaw.
fn calculate_camera_from_player(player: &PlayerState) -> (Vec3, Vec3) {
    // Camera looks at player's head height (approximately 1 unit up from position)
    let target = player.position + Vec3::new(0.0, 1.0, 0.0);

    // Camera is behind player based on yaw
    let camera_offset = Vec3::new(
        player.rotation_yaw.sin() * CAMERA_DISTANCE,
        CAMERA_HEIGHT,
        player.rotation_yaw.cos() * CAMERA_DISTANCE,
    );
    let position = player.position + camera_offset;

    (position, target)
}

/// Simulates one physics step for jump: applies velocity and gravity.
/// Returns true if player is on the ground.
fn step_jump_physics(player: &mut PlayerState, velocity_y: &mut f32, dt: f32) -> bool {
    // Apply gravity to velocity
    *velocity_y += GRAVITY * dt;

    // Apply velocity to position
    player.position.y += *velocity_y * dt;

    // Check for ground collision
    if player.position.y <= GROUND_Y {
        player.position.y = GROUND_Y;
        *velocity_y = 0.0;
        return true;
    }

    false
}

/// Helper function to render and capture a video frame.
#[cfg(feature = "test-helpers")]
fn render_and_capture_frame(
    render_state: &mut HeadlessRenderState,
    player: &PlayerState,
    recorder: &mut VideoRecorder,
) -> Result<(), String> {
    // Update camera position
    let (camera_pos, camera_target) = calculate_camera_from_player(player);
    render_state.camera.position = camera_pos;
    render_state.camera.target = camera_target;

    // Render (mutable borrow ends here when render_with_players returns)
    render_state.render_with_players(player.position, player.rotation_yaw, &[]);

    // Capture frame to video recorder
    recorder.capture_frame(
        &render_state.device,
        &render_state.queue,
        render_state.render_texture(),
    )
}

/// Captures multiple frames at the current position for video visibility.
#[cfg(feature = "test-helpers")]
fn capture_frames_at_position(
    render_state: &mut HeadlessRenderState,
    player: &PlayerState,
    recorder: &mut VideoRecorder,
    frame_count: u32,
) -> Result<(), String> {
    for _ in 0..frame_count {
        render_and_capture_frame(render_state, player, recorder)?;
    }
    Ok(())
}

#[test]
#[cfg(feature = "test-helpers")]
fn test_player_jump_records_video() {
    // Initialize headless render state
    let render_state = pollster::block_on(HeadlessRenderState::new(800, 600));

    let mut render_state = match render_state {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Skipping test_player_jump_records_video: {}", e);
            return;
        }
    };

    // Create initial player state at origin, on the ground
    let mut player = PlayerState::new(1);
    player.position = Vec3::new(0.0, GROUND_Y, 0.0);
    player.rotation_yaw = 0.0;

    // Track vertical velocity (not part of PlayerState, tracked locally)
    let mut velocity_y: f32 = 0.0;

    // Video output path
    let video_path = Path::new("tests/videos/jump_test.mp4");

    // Create video recorder at 30fps
    let mut recorder = VideoRecorder::new(800, 600, 30, video_path);

    // Capture initial frames on ground
    println!("Recording initial position on ground...");
    capture_frames_at_position(&mut render_state, &player, &mut recorder, GROUND_FRAMES)
        .expect("Failed to capture initial frames");

    // Initiate jump
    println!("Initiating jump...");
    velocity_y = JUMP_VELOCITY;

    // Physics loop: step and capture frame each step until landed
    let mut frame_count = 0;
    let max_frames = 1000; // Safety limit

    while frame_count < max_frames {
        // Step physics
        let landed = step_jump_physics(&mut player, &mut velocity_y, PHYSICS_DT);

        // Capture frame
        render_and_capture_frame(&mut render_state, &player, &mut recorder)
            .expect("Failed to capture jump frame");
        frame_count += 1;

        // Stop when landed
        if landed {
            println!(
                "Landed after {} physics steps, position.y = {}",
                frame_count, player.position.y
            );
            break;
        }
    }

    // Capture final frames after landing
    println!("Recording final position after landing...");
    capture_frames_at_position(&mut render_state, &player, &mut recorder, GROUND_FRAMES)
        .expect("Failed to capture final frames");

    // Calculate total frames
    let total_frames = recorder.frame_count();
    println!("Total frames recorded: {}", total_frames);

    // Finish recording and save video
    println!("Encoding video to {:?}...", video_path);
    recorder.finish().expect("Failed to encode video");

    // Verify video file exists and has content
    assert!(video_path.exists(), "jump_test.mp4 was not created");
    let metadata = std::fs::metadata(video_path).expect("Failed to get video file metadata");
    assert!(metadata.len() > 0, "jump_test.mp4 is empty");

    println!("Jump video recorded successfully!");
    println!("  - Output: {:?}", video_path);
    println!("  - Frames: {}", total_frames);
    println!("  - File size: {} bytes", metadata.len());
}
