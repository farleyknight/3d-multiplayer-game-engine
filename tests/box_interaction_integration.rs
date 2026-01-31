//! Integration test for box interaction (player pushing/moving boxes).
//! Tests spawning dynamic boxes near the player, simulating pushing them,
//! and capturing screenshots showing boxes in different positions.

use game_engine::render::{DynamicBox, HeadlessRenderState};
use game_engine::types::PlayerState;
use glam::Vec3;
use std::path::Path;

#[cfg(feature = "test-helpers")]
use game_engine::test_helpers::capture_screenshot;

/// Camera distance behind player (matches client.rs)
const CAMERA_DISTANCE: f32 = 5.0;

/// Camera height above player (matches client.rs)
const CAMERA_HEIGHT: f32 = 2.0;

/// Box color: Orange (#FF8800)
const BOX_ORANGE: [f32; 3] = [1.0, 0.533, 0.0];

/// Box color: Purple (#8800FF)
const BOX_PURPLE: [f32; 3] = [0.533, 0.0, 1.0];

/// Box color: Cyan (#00FFFF)
const BOX_CYAN: [f32; 3] = [0.0, 1.0, 1.0];

/// Push distance when player interacts with a box
const PUSH_DISTANCE: f32 = 1.5;

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

/// Simulates pushing a box away from the player.
/// When the player "pushes" a box, it moves in the direction away from the player.
fn push_box(player: &PlayerState, dynamic_box: &mut DynamicBox) {
    // Calculate direction from player to box (horizontal only)
    let to_box = Vec3::new(
        dynamic_box.position.x - player.position.x,
        0.0,
        dynamic_box.position.z - player.position.z,
    );

    // Only push if player is close enough (within 2 units)
    let distance = to_box.length();
    if distance < 2.0 && distance > 0.01 {
        // Push box away from player
        let push_dir = to_box.normalize();
        dynamic_box.position += push_dir * PUSH_DISTANCE;
    }
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

/// Helper function to render and capture a screenshot with dynamic boxes.
/// Separates the mutable borrow (render) from immutable borrows (capture).
#[cfg(feature = "test-helpers")]
fn render_and_capture_with_boxes(
    render_state: &mut HeadlessRenderState,
    player: &PlayerState,
    boxes: &[DynamicBox],
    output_path: &Path,
) {
    // Update camera position
    let (camera_pos, camera_target) = calculate_camera_from_player(player);
    render_state.camera.position = camera_pos;
    render_state.camera.target = camera_target;

    // Render with players and boxes
    render_state.render_with_players_and_boxes(
        player.position,
        player.rotation_yaw,
        &[],
        boxes,
    );

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
fn test_box_interaction_captures_screenshots() {
    // Initialize headless render state
    let render_state = pollster::block_on(HeadlessRenderState::new(800, 600));

    let mut render_state = match render_state {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Skipping test_box_interaction_captures_screenshots: {}", e);
            return;
        }
    };

    // Create initial player state at origin
    let mut player = PlayerState::new(1);
    player.position = Vec3::new(0.0, 0.0, 0.0);
    player.rotation_yaw = 0.0;

    // Create dynamic boxes near the player
    let mut boxes = vec![
        // Orange box in front of player
        DynamicBox::new(Vec3::new(0.0, 0.5, -2.0), (1.0, 1.0, 1.0), BOX_ORANGE),
        // Purple box to the right of player
        DynamicBox::new(Vec3::new(2.0, 0.5, 0.0), (1.0, 1.0, 1.0), BOX_PURPLE),
        // Cyan box behind and to the left
        DynamicBox::new(Vec3::new(-1.5, 0.5, 2.0), (1.0, 1.0, 1.0), BOX_CYAN),
    ];

    // Screenshot output paths
    let screenshots_dir = Path::new("tests/screenshots");
    let initial_path = screenshots_dir.join("box_initial.png");
    let push_1_path = screenshots_dir.join("box_push_1.png");
    let push_2_path = screenshots_dir.join("box_push_2.png");

    // 1. Capture initial positions
    render_and_capture_with_boxes(&mut render_state, &player, &boxes, &initial_path);
    println!("Initial box positions:");
    for (i, b) in boxes.iter().enumerate() {
        println!("  Box {}: {:?}", i, b.position);
    }

    // 2. Move player forward to push the orange box
    move_player(&mut player, "forward", 1.5);
    push_box(&player, &mut boxes[0]); // Push the orange box
    render_and_capture_with_boxes(&mut render_state, &player, &boxes, &push_1_path);
    println!("After push 1 - Player at {:?}", player.position);
    println!("  Orange box moved to: {:?}", boxes[0].position);

    // 3. Move player right to push the purple box
    move_player(&mut player, "right", 1.5);
    push_box(&player, &mut boxes[1]); // Push the purple box
    render_and_capture_with_boxes(&mut render_state, &player, &boxes, &push_2_path);
    println!("After push 2 - Player at {:?}", player.position);
    println!("  Purple box moved to: {:?}", boxes[1].position);

    // Verify all screenshot files exist
    assert!(initial_path.exists(), "box_initial.png was not created");
    assert!(push_1_path.exists(), "box_push_1.png was not created");
    assert!(push_2_path.exists(), "box_push_2.png was not created");

    println!("All box interaction screenshots captured successfully!");
    println!("  - {:?}", initial_path);
    println!("  - {:?}", push_1_path);
    println!("  - {:?}", push_2_path);
}
