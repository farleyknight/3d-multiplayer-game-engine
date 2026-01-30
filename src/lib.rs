//! Game engine shared library
//! Contains rendering, networking, and shared types for the 3D multiplayer game.

/// Version of the game engine
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Placeholder module for future networking code
pub mod network {
    /// Default server port for LAN multiplayer
    pub const DEFAULT_PORT: u16 = 7777;
}

/// Placeholder module for future rendering code
pub mod render {
    /// Window title for the game client
    pub const WINDOW_TITLE: &str = "3D Multiplayer Game";
}
