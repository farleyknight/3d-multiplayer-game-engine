//! Integration test for InputSimulator proving WASD input simulation works.
//! Tests that pressing W key moves the player forward (negative Z direction).
//!
//! Run with: `cargo test --test input_simulation_integration --features test-helpers`

#[cfg(feature = "test-helpers")]
use game_engine::test_helpers::{InputSimulator, KeyCode};
use glam::Vec3;

/// Tests that pressing the W key causes the player to move forward.
///
/// At yaw=0, forward direction is -Z. After pressing W and updating movement,
/// the player's Z position should decrease (move in -Z direction).
#[test]
#[cfg(feature = "test-helpers")]
fn test_wasd_input_simulation_w_key_moves_forward() {
    // Create a fresh InputSimulator
    let mut input = InputSimulator::new();

    // Create player position at origin
    let mut position = Vec3::ZERO;
    let yaw = 0.0; // Facing -Z direction (forward at yaw=0)
    let delta_time = 1.0; // 1 second for clear movement

    // Store initial Z position
    let initial_z = position.z;

    // Simulate pressing the W key
    input.press_key(KeyCode::W);

    // Update player movement based on input state
    input.update_player_movement(&mut position, yaw, delta_time);

    // Assert player moved forward (negative Z direction at yaw=0)
    assert!(
        position.z < initial_z,
        "Expected player to move forward (negative Z). Initial Z: {}, Current Z: {}",
        initial_z,
        position.z
    );

    // Print position change for verification
    println!(
        "W key movement test passed: Z changed from {} to {} (delta: {})",
        initial_z,
        position.z,
        position.z - initial_z
    );

    // Also verify no unexpected X/Y movement
    assert!(
        position.x.abs() < 0.001,
        "Expected no X movement, got: {}",
        position.x
    );
    assert!(
        position.y.abs() < 0.001,
        "Expected no Y movement, got: {}",
        position.y
    );
}

/// Tests that pressing the SpaceBar causes the player to jump (Y position increases).
///
/// When on the ground and SpaceBar is pressed, the player should receive an upward velocity
/// and their Y position should increase above GROUND_Y.
#[test]
#[cfg(feature = "test-helpers")]
fn test_spacebar_input_simulation_jump() {
    use game_engine::physics::{PhysicsState, GROUND_Y};

    // Create a fresh InputSimulator
    let mut input = InputSimulator::new();

    // Create player position at ground level
    let mut position = Vec3::new(0.0, GROUND_Y, 0.0);

    // Create physics state (starts on ground)
    let mut physics = PhysicsState::new();

    // Store initial Y position
    let initial_y = position.y;

    // Simulate pressing the Space key
    input.press_key(KeyCode::Space);

    // Update player physics to process the jump
    let delta_time = 1.0 / 60.0; // Standard 60 FPS frame
    input.update_player_physics(&mut position, &mut physics, delta_time);

    // Assert player jumped (Y position increased)
    assert!(
        position.y > initial_y,
        "Expected player to jump (Y increased). Initial Y: {}, Current Y: {}",
        initial_y,
        position.y
    );

    // Assert physics velocity is positive (upward)
    assert!(
        physics.velocity_y > 0.0,
        "Expected positive velocity after jump, got: {}",
        physics.velocity_y
    );

    // Assert player is no longer on ground
    assert!(
        !physics.on_ground,
        "Expected player to be off ground after jump"
    );

    // Print verification message
    println!(
        "SpaceBar jump test passed: Y changed from {} to {} (delta: {}), velocity_y: {}",
        initial_y,
        position.y,
        position.y - initial_y,
        physics.velocity_y
    );
}

/// Tests that all WASD keys produce movement in the expected directions.
#[test]
#[cfg(feature = "test-helpers")]
fn test_wasd_all_directions() {
    let mut input = InputSimulator::new();
    let yaw = 0.0;
    let delta_time = 1.0;

    // Test W (forward = -Z)
    let mut position = Vec3::ZERO;
    input.press_key(KeyCode::W);
    input.update_player_movement(&mut position, yaw, delta_time);
    assert!(position.z < 0.0, "W should move -Z, got z={}", position.z);
    input.clear();

    // Test S (backward = +Z)
    let mut position = Vec3::ZERO;
    input.press_key(KeyCode::S);
    input.update_player_movement(&mut position, yaw, delta_time);
    assert!(position.z > 0.0, "S should move +Z, got z={}", position.z);
    input.clear();

    // Test A (left = +X at yaw=0 based on right being -X)
    let mut position = Vec3::ZERO;
    input.press_key(KeyCode::A);
    input.update_player_movement(&mut position, yaw, delta_time);
    assert!(position.x > 0.0, "A should move +X at yaw=0, got x={}", position.x);
    input.clear();

    // Test D (right = -X at yaw=0)
    let mut position = Vec3::ZERO;
    input.press_key(KeyCode::D);
    input.update_player_movement(&mut position, yaw, delta_time);
    assert!(position.x < 0.0, "D should move -X at yaw=0, got x={}", position.x);

    println!("All WASD direction tests passed!");
}
