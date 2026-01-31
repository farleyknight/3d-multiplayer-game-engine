//! Integration test for player movement with video recording.
//! Tests movement in all four directions and records video output.

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

/// Movement amount per step (units)
const MOVE_AMOUNT: f32 = 2.0;

/// Number of frames to capture per position for visible movement in video
const FRAMES_PER_POSITION: u32 = 15;

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

/// Moves a player in the specified direction based on their current yaw.
fn move_player(player: &mut PlayerState, direction: &str, amount: f32) {
    // Calculate forward and right vectors based on player yaw
    let forward = Vec3::new(
        -player.rotation_yaw.sin(),
        0.0,
        -player.rotation_yaw.cos(),
    );
    let right = Vec3::new(forward.z, 0.0, -forward.x);

    let movement = match direction {
        "forward" => forward * amount,
        "backward" => -forward * amount,
        "left" => -right * amount,
        "right" => right * amount,
        _ => Vec3::ZERO,
    };

    player.position += movement;
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
fn test_player_movement_records_video() {
    // Initialize headless render state
    let render_state = pollster::block_on(HeadlessRenderState::new(800, 600));

    let mut render_state = match render_state {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Skipping test_player_movement_records_video: {}", e);
            return;
        }
    };

    // Create initial player state at origin
    let mut player = PlayerState::new(1);
    player.position = Vec3::new(0.0, 0.0, 0.0);
    player.rotation_yaw = 0.0;

    // Video output path
    let video_path = Path::new("tests/videos/movement_test.mp4");

    // Create video recorder at 30fps
    let mut recorder = VideoRecorder::new(800, 600, 30, video_path);

    // Capture initial position frames
    println!("Recording initial position...");
    capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_POSITION)
        .expect("Failed to capture initial frames");

    // Move forward (W) and capture frames
    println!("Recording forward movement...");
    move_player(&mut player, "forward", MOVE_AMOUNT);
    capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_POSITION)
        .expect("Failed to capture forward frames");

    // Move left (A) and capture frames
    println!("Recording left movement...");
    move_player(&mut player, "left", MOVE_AMOUNT);
    capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_POSITION)
        .expect("Failed to capture left frames");

    // Move backward (S) and capture frames
    println!("Recording backward movement...");
    move_player(&mut player, "backward", MOVE_AMOUNT);
    capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_POSITION)
        .expect("Failed to capture backward frames");

    // Move right (D) and capture frames
    println!("Recording right movement...");
    move_player(&mut player, "right", MOVE_AMOUNT);
    capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_POSITION)
        .expect("Failed to capture right frames");

    // Verify we captured expected number of frames (5 positions * FRAMES_PER_POSITION)
    let expected_frames = 5 * FRAMES_PER_POSITION as usize;
    assert_eq!(
        recorder.frame_count(),
        expected_frames,
        "Expected {} frames, got {}",
        expected_frames,
        recorder.frame_count()
    );

    // Finish recording and save video
    println!("Encoding video to {:?}...", video_path);
    recorder.finish().expect("Failed to encode video");

    // Verify video file exists and has content
    assert!(video_path.exists(), "movement_test.mp4 was not created");
    let metadata = std::fs::metadata(video_path).expect("Failed to get video file metadata");
    assert!(metadata.len() > 0, "movement_test.mp4 is empty");

    println!("Movement video recorded successfully!");
    println!("  - Output: {:?}", video_path);
    println!("  - Frames: {}", expected_frames);
    println!("  - File size: {} bytes", metadata.len());
}
