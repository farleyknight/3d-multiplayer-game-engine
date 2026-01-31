//! Game engine shared library
//! Contains rendering, networking, and shared types for the 3D multiplayer game.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Version of the game engine
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Shared types for player state and game data
pub mod types {
    use super::*;

    /// Represents a player's current state in the game world
    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub struct PlayerState {
        /// Unique identifier assigned by the server
        pub player_id: u32,
        /// 3D position in world space
        pub position: Vec3,
        /// Rotation around the Y axis in radians
        pub rotation_yaw: f32,
    }

    impl PlayerState {
        /// Create a new player state with default position at origin
        pub fn new(player_id: u32) -> Self {
            Self {
                player_id,
                position: Vec3::ZERO,
                rotation_yaw: 0.0,
            }
        }

        /// Create a player state with specified position and rotation
        pub fn with_transform(player_id: u32, position: Vec3, rotation_yaw: f32) -> Self {
            Self {
                player_id,
                position,
                rotation_yaw,
            }
        }
    }

    /// Data for a single player in WorldState broadcasts
    #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
    pub struct PlayerData {
        pub player_id: u32,
        pub position: Vec3,
        pub rotation_yaw: f32,
    }

    impl From<PlayerState> for PlayerData {
        fn from(state: PlayerState) -> Self {
            Self {
                player_id: state.player_id,
                position: state.position,
                rotation_yaw: state.rotation_yaw,
            }
        }
    }
}

/// Network protocol definitions
pub mod network {
    use super::*;
    use crate::types::{PlayerData, PlayerState};
    use std::time::Instant;

    /// Default server port for LAN multiplayer (spec: 7878)
    pub const DEFAULT_PORT: u16 = 7878;

    /// Tracks a connected client on the server
    #[derive(Debug, Clone)]
    pub struct ConnectedClient {
        /// The player's ID assigned by the server
        pub player_id: u32,
        /// When we last received a packet from this client
        pub last_seen: Instant,
        /// The player's current state (position, rotation)
        pub state: PlayerState,
    }

    impl ConnectedClient {
        /// Create a new connected client with the given player ID
        pub fn new(player_id: u32) -> Self {
            Self {
                player_id,
                last_seen: Instant::now(),
                state: PlayerState::new(player_id),
            }
        }

        /// Update the last_seen timestamp to now
        pub fn touch(&mut self) {
            self.last_seen = Instant::now();
        }

        /// Check if this client has timed out based on TIMEOUT_SECONDS
        pub fn is_timed_out(&self) -> bool {
            self.last_seen.elapsed().as_secs() >= TIMEOUT_SECONDS
        }
    }

    /// Timeout in seconds before a client is considered disconnected
    pub const TIMEOUT_SECONDS: u64 = 5;

    /// Interval between WorldState broadcasts in milliseconds
    pub const BROADCAST_INTERVAL_MS: u64 = 50;

    /// Target update rate in Hz
    pub const UPDATE_RATE_HZ: u32 = 20;

    /// Packet type discriminants matching the network protocol spec
    pub mod packet_type {
        pub const PLAYER_UPDATE: u8 = 0x01;
        pub const WELCOME: u8 = 0x10;
        pub const WORLD_STATE: u8 = 0x11;
        pub const PLAYER_LEFT: u8 = 0x12;
    }

    /// Packets sent from client to server
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub enum ClientPacket {
        /// Player position/rotation update (0x01)
        /// player_id is 0 if this is a new client requesting an ID
        PlayerUpdate(PlayerState),
    }

    /// Packets sent from server to client
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub enum ServerPacket {
        /// Welcome message with assigned player ID (0x10)
        Welcome { assigned_player_id: u32 },

        /// World state broadcast containing all player positions (0x11)
        WorldState { players: Vec<PlayerData> },

        /// Notification that a player has disconnected (0x12)
        PlayerLeft { player_id: u32 },
    }

    /// Serialize a client packet to bytes using bincode
    pub fn serialize_client_packet(packet: &ClientPacket) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(packet)
    }

    /// Deserialize a client packet from bytes
    pub fn deserialize_client_packet(data: &[u8]) -> Result<ClientPacket, bincode::Error> {
        bincode::deserialize(data)
    }

    /// Serialize a server packet to bytes using bincode
    pub fn serialize_server_packet(packet: &ServerPacket) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(packet)
    }

    /// Deserialize a server packet from bytes
    pub fn deserialize_server_packet(data: &[u8]) -> Result<ServerPacket, bincode::Error> {
        bincode::deserialize(data)
    }
}

/// Placeholder module for future rendering code
pub mod render {
    /// Window title for the game client
    pub const WINDOW_TITLE: &str = "3D Multiplayer Game";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::*;
    use crate::types::*;

    #[test]
    fn test_player_state_serialization_roundtrip() {
        let state = PlayerState::with_transform(42, Vec3::new(1.0, 2.0, 3.0), 1.57);

        let bytes = bincode::serialize(&state).expect("serialize failed");
        let deserialized: PlayerState = bincode::deserialize(&bytes).expect("deserialize failed");

        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_player_state_default_position() {
        let state = PlayerState::new(1);

        assert_eq!(state.player_id, 1);
        assert_eq!(state.position, Vec3::ZERO);
        assert_eq!(state.rotation_yaw, 0.0);
    }

    #[test]
    fn test_player_data_serialization_roundtrip() {
        let data = PlayerData {
            player_id: 99,
            position: Vec3::new(-5.0, 0.0, 10.0),
            rotation_yaw: 3.14,
        };

        let bytes = bincode::serialize(&data).expect("serialize failed");
        let deserialized: PlayerData = bincode::deserialize(&bytes).expect("deserialize failed");

        assert_eq!(data, deserialized);
    }

    #[test]
    fn test_player_data_from_player_state() {
        let state = PlayerState::with_transform(7, Vec3::new(1.0, 2.0, 3.0), 0.5);
        let data: PlayerData = state.into();

        assert_eq!(data.player_id, state.player_id);
        assert_eq!(data.position, state.position);
        assert_eq!(data.rotation_yaw, state.rotation_yaw);
    }

    #[test]
    fn test_client_packet_player_update_roundtrip() {
        let state = PlayerState::with_transform(0, Vec3::new(10.0, 0.0, -5.0), 2.0);
        let packet = ClientPacket::PlayerUpdate(state);

        let bytes = serialize_client_packet(&packet).expect("serialize failed");
        let deserialized = deserialize_client_packet(&bytes).expect("deserialize failed");

        assert_eq!(packet, deserialized);
    }

    #[test]
    fn test_server_packet_welcome_roundtrip() {
        let packet = ServerPacket::Welcome {
            assigned_player_id: 123,
        };

        let bytes = serialize_server_packet(&packet).expect("serialize failed");
        let deserialized = deserialize_server_packet(&bytes).expect("deserialize failed");

        assert_eq!(packet, deserialized);
    }

    #[test]
    fn test_server_packet_world_state_roundtrip() {
        let players = vec![
            PlayerData {
                player_id: 1,
                position: Vec3::new(0.0, 0.0, 0.0),
                rotation_yaw: 0.0,
            },
            PlayerData {
                player_id: 2,
                position: Vec3::new(5.0, 0.0, 5.0),
                rotation_yaw: 1.57,
            },
            PlayerData {
                player_id: 3,
                position: Vec3::new(-10.0, 2.0, 8.0),
                rotation_yaw: 3.14,
            },
        ];
        let packet = ServerPacket::WorldState { players };

        let bytes = serialize_server_packet(&packet).expect("serialize failed");
        let deserialized = deserialize_server_packet(&bytes).expect("deserialize failed");

        assert_eq!(packet, deserialized);
    }

    #[test]
    fn test_server_packet_world_state_empty_roundtrip() {
        let packet = ServerPacket::WorldState { players: vec![] };

        let bytes = serialize_server_packet(&packet).expect("serialize failed");
        let deserialized = deserialize_server_packet(&bytes).expect("deserialize failed");

        assert_eq!(packet, deserialized);
    }

    #[test]
    fn test_server_packet_player_left_roundtrip() {
        let packet = ServerPacket::PlayerLeft { player_id: 456 };

        let bytes = serialize_server_packet(&packet).expect("serialize failed");
        let deserialized = deserialize_server_packet(&bytes).expect("deserialize failed");

        assert_eq!(packet, deserialized);
    }

    #[test]
    fn test_network_constants() {
        assert_eq!(DEFAULT_PORT, 7878);
        assert_eq!(TIMEOUT_SECONDS, 5);
        assert_eq!(BROADCAST_INTERVAL_MS, 50);
        assert_eq!(UPDATE_RATE_HZ, 20);
    }

    #[test]
    fn test_packet_type_discriminants() {
        assert_eq!(packet_type::PLAYER_UPDATE, 0x01);
        assert_eq!(packet_type::WELCOME, 0x10);
        assert_eq!(packet_type::WORLD_STATE, 0x11);
        assert_eq!(packet_type::PLAYER_LEFT, 0x12);
    }

    #[test]
    fn test_connected_client_new() {
        use crate::network::ConnectedClient;

        let client = ConnectedClient::new(42);

        assert_eq!(client.player_id, 42);
        assert_eq!(client.state.player_id, 42);
        assert_eq!(client.state.position, glam::Vec3::ZERO);
        assert_eq!(client.state.rotation_yaw, 0.0);
        assert!(!client.is_timed_out());
    }

    #[test]
    fn test_connected_client_touch() {
        use crate::network::ConnectedClient;

        let mut client = ConnectedClient::new(1);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let before_touch = client.last_seen;
        client.touch();

        assert!(client.last_seen > before_touch);
    }

    #[test]
    fn test_server_socket_binding() {
        use std::net::UdpSocket;

        // Use a random port to avoid conflicts with other tests or running server
        let socket = UdpSocket::bind("127.0.0.1:0").expect("Failed to bind UDP socket");
        let local_addr = socket.local_addr().expect("Failed to get local address");

        assert!(local_addr.port() > 0);
    }
}
