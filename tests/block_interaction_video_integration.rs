//! Integration test for block interaction with video recording.
//! Tests block destruction and placement and records video output.

use game_engine::blocks::BlockType;
use game_engine::raycast::raycast_voxel;
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

/// Eye height above player position (for raycasting)
const EYE_HEIGHT: f32 = 1.6;

/// Maximum distance for block interaction raycasting
const BLOCK_REACH_DISTANCE: f32 = 5.0;

/// Number of frames to capture per action for visible changes in video
const FRAMES_PER_ACTION: u32 = 30;

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
fn test_block_interaction_records_video() {
    // Initialize headless render state
    let render_state = pollster::block_on(HeadlessRenderState::new(800, 600));

    let mut render_state = match render_state {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Skipping test_block_interaction_records_video: {}", e);
            return;
        }
    };

    // Create player state - position player above the ground looking down at a block
    // Player stands at y=1 (above ground level y=0 which has grass blocks)
    let mut player = PlayerState::new(1);
    player.position = Vec3::new(2.0, 1.0, 2.0);
    player.rotation_yaw = 0.0; // Looking in -Z direction

    // Video output path
    let video_path = Path::new("tests/videos/block_interaction.mp4");

    // Create video recorder at 30fps
    let mut recorder = VideoRecorder::new(800, 600, 30, video_path);

    // Capture initial frames showing the world with blocks
    println!("Recording initial state...");
    capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_ACTION)
        .expect("Failed to capture initial frames");

    // === BLOCK DESTRUCTION ===
    // Calculate ray from player eye position looking downward at the block
    let eye_pos = player.position + Vec3::new(0.0, EYE_HEIGHT, 0.0);
    // Look slightly down and forward to hit a block on the ground
    let look_direction = Vec3::new(0.0, -0.5, -1.0).normalize();

    // First, let's place a target block above ground for better visibility
    println!("Placing a cobblestone block for destruction test...");
    render_state.voxel_world.set_block(2, 1, 0, BlockType::Cobblestone);
    render_state.rebuild_voxel_mesh();

    // Capture frames showing the placed block
    capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_ACTION)
        .expect("Failed to capture frames after placing target block");

    // Now raycast to find the block
    if let Some(hit) = raycast_voxel(&render_state.voxel_world, eye_pos, look_direction, BLOCK_REACH_DISTANCE) {
        let (bx, by, bz) = hit.block_pos;
        println!("Destroying block at ({}, {}, {})", bx, by, bz);

        // Destroy the block
        render_state.voxel_world.remove_block(bx, by, bz);
        render_state.rebuild_voxel_mesh();

        // Capture frames showing the destroyed block
        capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_ACTION)
            .expect("Failed to capture frames after destruction");
    } else {
        println!("No block hit for destruction (looking at: {:?}, dir: {:?})", eye_pos, look_direction);
        // Still capture frames even if no block was hit
        capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_ACTION)
            .expect("Failed to capture frames");
    }

    // === BLOCK PLACEMENT ===
    // Reposition player to look at the ground for placement
    player.position = Vec3::new(4.0, 1.0, 4.0);
    let eye_pos = player.position + Vec3::new(0.0, EYE_HEIGHT, 0.0);
    // Look down at the ground
    let look_direction = Vec3::new(0.0, -1.0, 0.0).normalize();

    println!("Recording before placement...");
    capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_ACTION)
        .expect("Failed to capture pre-placement frames");

    // Raycast to find where to place
    if let Some(hit) = raycast_voxel(&render_state.voxel_world, eye_pos, look_direction, BLOCK_REACH_DISTANCE) {
        let (bx, by, bz) = hit.block_pos;
        let (nx, ny, nz) = hit.face_normal;
        let (px, py, pz) = (bx + nx, by + ny, bz + nz);

        println!("Placing stone block at ({}, {}, {})", px, py, pz);

        // Place the block
        render_state.voxel_world.set_block(px, py, pz, BlockType::Stone);
        render_state.rebuild_voxel_mesh();

        // Capture frames showing the placed block
        capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_ACTION)
            .expect("Failed to capture frames after placement");
    } else {
        println!("No block hit for placement (looking at: {:?}, dir: {:?})", eye_pos, look_direction);
        // Still capture frames even if no block was hit
        capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_ACTION)
            .expect("Failed to capture frames");
    }

    // === SECOND PLACEMENT ===
    // Place another block at a different location for variety
    player.position = Vec3::new(0.0, 1.0, 0.0);
    let eye_pos = player.position + Vec3::new(0.0, EYE_HEIGHT, 0.0);
    let look_direction = Vec3::new(1.0, -0.3, 0.0).normalize(); // Look right and slightly down

    println!("Recording second placement...");
    capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_ACTION / 2)
        .expect("Failed to capture pre-second-placement frames");

    if let Some(hit) = raycast_voxel(&render_state.voxel_world, eye_pos, look_direction, BLOCK_REACH_DISTANCE) {
        let (bx, by, bz) = hit.block_pos;
        let (nx, ny, nz) = hit.face_normal;
        let (px, py, pz) = (bx + nx, by + ny, bz + nz);

        println!("Placing dirt block at ({}, {}, {})", px, py, pz);

        render_state.voxel_world.set_block(px, py, pz, BlockType::Dirt);
        render_state.rebuild_voxel_mesh();

        capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_ACTION)
            .expect("Failed to capture frames after second placement");
    } else {
        capture_frames_at_position(&mut render_state, &player, &mut recorder, FRAMES_PER_ACTION)
            .expect("Failed to capture frames");
    }

    // Calculate total frames
    let total_frames = recorder.frame_count();
    println!("Total frames recorded: {}", total_frames);

    // Finish recording and save video
    println!("Encoding video to {:?}...", video_path);
    recorder.finish().expect("Failed to encode video");

    // Verify video file exists and has content
    assert!(video_path.exists(), "block_interaction.mp4 was not created");
    let metadata = std::fs::metadata(video_path).expect("Failed to get video file metadata");
    assert!(metadata.len() > 0, "block_interaction.mp4 is empty");

    println!("Block interaction video recorded successfully!");
    println!("  - Output: {:?}", video_path);
    println!("  - Frames: {}", total_frames);
    println!("  - File size: {} bytes", metadata.len());
}
