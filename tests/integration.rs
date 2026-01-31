//! Unified integration test runner for all visual integration tests.
//!
//! Run all visual integration tests:
//! ```bash
//! cargo test --test integration --features test-helpers
//! ```
//!
//! This test module includes all visual integration test modules:
//! - movement: Tests player movement (WASD controls)
//! - jump: Tests player jumping physics
//! - box_interaction: Tests player interaction with dynamic boxes
//!
//! Individual test files are included as submodules via `#[path]` attributes.

// Include individual integration test modules using #[path] attribute
// This allows running all visual tests with a single `cargo test --test integration` command

#[path = "movement_integration.rs"]
mod movement;

#[path = "jump_integration.rs"]
mod jump;

#[path = "box_interaction_integration.rs"]
mod box_interaction;

#[path = "movement_video_integration.rs"]
mod movement_video;

/// Placeholder test to verify the integration test runner compiles and runs.
/// The actual tests are in the included modules above.
#[test]
fn integration_test_runner_works() {
    println!("Integration test runner initialized.");
    println!("All visual integration test modules are available:");
    println!("  - movement (WASD controls)");
    println!("  - jump (jump physics)");
    println!("  - box_interaction (dynamic boxes)");
    println!("  - movement_video (WASD movement with video recording)");
}
