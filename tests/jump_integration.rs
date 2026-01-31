//! Integration test for player jumping.
//! Tests jump physics (vertical velocity + gravity) and captures screenshots at different phases.

#[cfg(feature = "test-helpers")]
use game_engine::render::HeadlessRenderState;
use game_engine::types::PlayerState;
use glam::Vec3;
#[cfg(feature = "test-helpers")]
use std::path::Path;

#[cfg(feature = "test-helpers")]
use game_engine::test_helpers::capture_screenshot;

/// Camera distance behind player (matches client.rs)
#[cfg(feature = "test-helpers")]
const CAMERA_DISTANCE: f32 = 5.0;

/// Camera height above player (matches client.rs)
#[cfg(feature = "test-helpers")]
const CAMERA_HEIGHT: f32 = 2.0;

/// Jump physics constants
#[cfg(feature = "test-helpers")]
const JUMP_VELOCITY: f32 = 8.0; // Initial upward velocity (units/sec)
const GRAVITY: f32 = -20.0; // Downward acceleration (units/sec²)
const GROUND_Y: f32 = 0.0; // Ground level

/// Time step for physics simulation (seconds)
const PHYSICS_DT: f32 = 1.0 / 60.0;

/// Calculates camera position based on player state for third-person view.
/// Camera is positioned behind and above the player, following the player's yaw.
#[cfg(feature = "test-helpers")]
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

/// Simulates jump physics until the specified condition is met.
/// Returns the number of steps taken.
fn simulate_until<F>(player: &mut PlayerState, velocity_y: &mut f32, condition: F) -> u32
where
    F: Fn(&PlayerState, f32) -> bool,
{
    let mut steps = 0;
    let max_steps = 1000; // Safety limit

    while !condition(player, *velocity_y) && steps < max_steps {
        step_jump_physics(player, velocity_y, PHYSICS_DT);
        steps += 1;
    }

    steps
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
fn test_player_jump_captures_screenshots() {
    // Initialize headless render state
    let render_state = pollster::block_on(HeadlessRenderState::new(800, 600));

    let mut render_state = match render_state {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Skipping test_player_jump_captures_screenshots: {}", e);
            return;
        }
    };

    // Create initial player state at origin, on the ground
    let mut player = PlayerState::new(1);
    player.position = Vec3::new(0.0, GROUND_Y, 0.0);
    player.rotation_yaw = 0.0;

    // Track vertical velocity (not part of PlayerState, tracked locally)
    let mut velocity_y: f32 = 0.0;

    // Screenshot output paths
    let screenshots_dir = Path::new("tests/screenshots");
    let initial_path = screenshots_dir.join("jump_initial.png");
    let ascending_path = screenshots_dir.join("jump_ascending.png");
    let peak_path = screenshots_dir.join("jump_peak.png");
    let descending_path = screenshots_dir.join("jump_descending.png");
    let landed_path = screenshots_dir.join("jump_landed.png");

    // 1. Capture initial position on ground
    render_and_capture(&mut render_state, &player, &initial_path);
    println!(
        "Initial: position.y = {}, velocity_y = {}",
        player.position.y, velocity_y
    );

    // 2. Initiate jump
    velocity_y = JUMP_VELOCITY;

    // 3. Simulate until player is ascending (above ground with positive velocity)
    simulate_until(&mut player, &mut velocity_y, |p, v| {
        p.position.y > 0.5 && v > 0.0
    });
    render_and_capture(&mut render_state, &player, &ascending_path);
    println!(
        "Ascending: position.y = {}, velocity_y = {}",
        player.position.y, velocity_y
    );

    // 4. Simulate until peak (velocity near zero or negative)
    simulate_until(&mut player, &mut velocity_y, |_p, v| v <= 0.0);
    render_and_capture(&mut render_state, &player, &peak_path);
    println!(
        "Peak: position.y = {}, velocity_y = {}",
        player.position.y, velocity_y
    );

    // 5. Simulate until descending (significant downward velocity)
    simulate_until(&mut player, &mut velocity_y, |p, v| {
        v < -2.0 && p.position.y > 0.5
    });
    render_and_capture(&mut render_state, &player, &descending_path);
    println!(
        "Descending: position.y = {}, velocity_y = {}",
        player.position.y, velocity_y
    );

    // 6. Simulate until landed (back on ground)
    simulate_until(&mut player, &mut velocity_y, |p, _v| p.position.y <= GROUND_Y);
    render_and_capture(&mut render_state, &player, &landed_path);
    println!(
        "Landed: position.y = {}, velocity_y = {}",
        player.position.y, velocity_y
    );

    // Verify all screenshot files exist
    assert!(initial_path.exists(), "jump_initial.png was not created");
    assert!(ascending_path.exists(), "jump_ascending.png was not created");
    assert!(peak_path.exists(), "jump_peak.png was not created");
    assert!(
        descending_path.exists(),
        "jump_descending.png was not created"
    );
    assert!(landed_path.exists(), "jump_landed.png was not created");

    println!("All jump screenshots captured successfully!");
    println!("  - {:?}", initial_path);
    println!("  - {:?}", ascending_path);
    println!("  - {:?}", peak_path);
    println!("  - {:?}", descending_path);
    println!("  - {:?}", landed_path);
}
