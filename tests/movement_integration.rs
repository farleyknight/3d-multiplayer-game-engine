//! Integration test for player movement using WASD controls.
//! Tests movement in all four directions and captures screenshots at each position.

use game_engine::render::HeadlessRenderState;
use game_engine::types::PlayerState;
use glam::Vec3;
use std::path::Path;

#[cfg(feature = "test-helpers")]
use game_engine::test_helpers::capture_screenshot;

/// Camera distance behind player (matches client.rs)
const CAMERA_DISTANCE: f32 = 5.0;

/// Camera height above player (matches client.rs)
const CAMERA_HEIGHT: f32 = 2.0;

/// Movement amount per step (units)
const MOVE_AMOUNT: f32 = 2.0;

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

/// Helper function to render and capture a screenshot.
/// Separates the mutable borrow (render) from immutable borrows (capture).
#[cfg(feature = "test-helpers")]
fn render_and_capture(
    render_state: &mut HeadlessRenderState,
    player: &PlayerState,
    output_path: &Path,
) {
    // Update camera position
    let (camera_pos, camera_target) = calculate_camera_from_player(player);
    render_state.camera.position = camera_pos;
    render_state.camera.target = camera_target;

    // Render (mutable borrow ends here when render_with_players returns)
    render_state.render_with_players(player.position, player.rotation_yaw, &[]);

    // Now capture using render_texture() which only borrows immutably
    let width = render_state.width;
    let height = render_state.height;
    capture_screenshot(
        &render_state.device,
        &render_state.queue,
        render_state.render_texture(),
        width,
        height,
        output_path,
    )
    .expect(&format!("Failed to capture screenshot: {:?}", output_path));
}

#[test]
#[cfg(feature = "test-helpers")]
fn test_player_movement_captures_screenshots() {
    // Initialize headless render state
    let render_state = pollster::block_on(HeadlessRenderState::new(800, 600));

    let mut render_state = match render_state {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Skipping test_player_movement_captures_screenshots: {}", e);
            return;
        }
    };

    // Create initial player state at origin
    let mut player = PlayerState::new(1);
    player.position = Vec3::new(0.0, 0.0, 0.0);
    player.rotation_yaw = 0.0;

    // Screenshot output paths
    let screenshots_dir = Path::new("tests/screenshots");
    let initial_path = screenshots_dir.join("movement_initial.png");
    let forward_path = screenshots_dir.join("movement_forward.png");
    let left_path = screenshots_dir.join("movement_left.png");
    let back_path = screenshots_dir.join("movement_back.png");
    let right_path = screenshots_dir.join("movement_right.png");

    // Capture initial position screenshot
    render_and_capture(&mut render_state, &player, &initial_path);

    // Move forward (W) and capture
    move_player(&mut player, "forward", MOVE_AMOUNT);
    render_and_capture(&mut render_state, &player, &forward_path);

    // Move left (A) and capture
    move_player(&mut player, "left", MOVE_AMOUNT);
    render_and_capture(&mut render_state, &player, &left_path);

    // Move backward (S) and capture
    move_player(&mut player, "backward", MOVE_AMOUNT);
    render_and_capture(&mut render_state, &player, &back_path);

    // Move right (D) and capture
    move_player(&mut player, "right", MOVE_AMOUNT);
    render_and_capture(&mut render_state, &player, &right_path);

    // Verify all screenshot files exist
    assert!(initial_path.exists(), "movement_initial.png was not created");
    assert!(forward_path.exists(), "movement_forward.png was not created");
    assert!(left_path.exists(), "movement_left.png was not created");
    assert!(back_path.exists(), "movement_back.png was not created");
    assert!(right_path.exists(), "movement_right.png was not created");

    println!("All movement screenshots captured successfully!");
    println!("  - {:?}", initial_path);
    println!("  - {:?}", forward_path);
    println!("  - {:?}", left_path);
    println!("  - {:?}", back_path);
    println!("  - {:?}", right_path);
}
