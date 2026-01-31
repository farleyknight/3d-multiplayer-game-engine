//! Game engine shared library
//! Contains rendering, networking, and shared types for the 3D multiplayer game.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Test helper utilities (only available when running tests or for integration tests)
#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers;

/// Version of the game engine
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Texture loading and management for block textures
pub mod texture {
    use std::collections::HashMap;
    use std::path::Path;
    use wgpu::{Device, Queue, Sampler, Texture, TextureDescriptor, TextureFormat, TextureUsages};

    /// Error type for texture loading operations
    #[derive(Debug)]
    pub enum TextureError {
        /// File not found or cannot be read
        IoError(std::io::Error),
        /// Image decoding failed
        ImageError(image::ImageError),
        /// Texture creation failed
        WgpuError(String),
    }

    impl std::fmt::Display for TextureError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TextureError::IoError(e) => write!(f, "IO error: {}", e),
                TextureError::ImageError(e) => write!(f, "Image error: {}", e),
                TextureError::WgpuError(e) => write!(f, "WGPU error: {}", e),
            }
        }
    }

    impl std::error::Error for TextureError {}

    impl From<std::io::Error> for TextureError {
        fn from(err: std::io::Error) -> Self {
            TextureError::IoError(err)
        }
    }

    impl From<image::ImageError> for TextureError {
        fn from(err: image::ImageError) -> Self {
            TextureError::ImageError(err)
        }
    }

    /// Load a single PNG texture from disk and create a wgpu Texture.
    ///
    /// # Arguments
    /// * `device` - The wgpu device for creating the texture
    /// * `queue` - The wgpu queue for uploading texture data
    /// * `path` - Path to the PNG file
    ///
    /// # Returns
    /// A wgpu Texture ready for use in shaders, or a TextureError if loading fails.
    pub fn load_png(device: &Device, queue: &Queue, path: &Path) -> Result<Texture, TextureError> {
        // Read and decode the image file
        let img = image::open(path)?;

        // Convert to RGBA8 format
        let rgba = img.to_rgba8();
        let dimensions = rgba.dimensions();
        let (width, height) = dimensions;

        // Create the wgpu texture
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(path.to_str().unwrap_or("texture")),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload the pixel data to the texture
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Ok(texture)
    }

    /// Load multiple block textures from a directory.
    ///
    /// Scans the given directory for PNG files and loads each one into a wgpu Texture.
    /// The texture name in the HashMap is the filename without extension (e.g., "grass_side").
    ///
    /// # Arguments
    /// * `device` - The wgpu device for creating textures
    /// * `queue` - The wgpu queue for uploading texture data
    /// * `base_path` - Path to the directory containing PNG files
    ///
    /// # Returns
    /// A HashMap mapping texture names to wgpu Textures.
    pub fn load_block_textures(
        device: &Device,
        queue: &Queue,
        base_path: &Path,
    ) -> Result<HashMap<String, Texture>, TextureError> {
        let mut textures = HashMap::new();

        // Read the directory
        let entries = std::fs::read_dir(base_path)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only process PNG files
            if path.extension().map_or(false, |ext| ext == "png") {
                if let Some(stem) = path.file_stem() {
                    let name = stem.to_string_lossy().to_string();
                    match load_png(device, queue, &path) {
                        Ok(texture) => {
                            textures.insert(name, texture);
                        }
                        Err(e) => {
                            // Log the error but continue loading other textures
                            log::warn!("Failed to load texture {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        Ok(textures)
    }

    /// Create a nearest-neighbor sampler for pixel art textures.
    ///
    /// Uses nearest-neighbor filtering to preserve the blocky appearance of Minecraft-style textures.
    pub fn create_pixel_art_sampler(device: &Device) -> Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Pixel Art Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        })
    }

    /// Load the block texture atlas for voxel rendering.
    ///
    /// Tries to load cobblestone.png from the textures directory, falling back to a
    /// procedural checkerboard pattern if the file is not available.
    pub fn load_block_texture_atlas(device: &Device, queue: &Queue) -> Texture {
        let texture_path = std::path::Path::new("textures/default-textures/textures/blocks/cobblestone.png");
        if texture_path.exists() {
            if let Ok(texture) = load_png(device, queue, texture_path) {
                return texture;
            }
        }

        // Create a simple 16x16 checkerboard fallback texture
        let mut fallback_data = vec![0u8; 16 * 16 * 4];
        for y in 0..16 {
            for x in 0..16 {
                let idx = (y * 16 + x) * 4;
                let is_light = (x / 4 + y / 4) % 2 == 0;
                let color = if is_light { 200u8 } else { 100u8 };
                fallback_data[idx] = color;     // R
                fallback_data[idx + 1] = color; // G
                fallback_data[idx + 2] = color; // B
                fallback_data[idx + 3] = 255;   // A
            }
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Fallback Block Texture"),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &fallback_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(16 * 4),
                rows_per_image: Some(16),
            },
            wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
        );
        texture
    }

    /// Holds loaded textures and a sampler for rendering.
    pub struct TextureLoader {
        /// Loaded block textures, keyed by filename without extension
        pub textures: HashMap<String, Texture>,
        /// Sampler configured for pixel art (nearest-neighbor filtering)
        pub sampler: Sampler,
    }

    impl TextureLoader {
        /// Create a new TextureLoader by loading all PNG textures from the given directory.
        ///
        /// # Arguments
        /// * `device` - The wgpu device for creating textures and sampler
        /// * `queue` - The wgpu queue for uploading texture data
        /// * `textures_path` - Path to the directory containing PNG textures
        pub fn new(
            device: &Device,
            queue: &Queue,
            textures_path: &Path,
        ) -> Result<Self, TextureError> {
            let textures = load_block_textures(device, queue, textures_path)?;
            let sampler = create_pixel_art_sampler(device);
            Ok(Self { textures, sampler })
        }

        /// Get a reference to a texture by name.
        pub fn get(&self, name: &str) -> Option<&Texture> {
            self.textures.get(name)
        }

        /// Get the number of loaded textures.
        pub fn len(&self) -> usize {
            self.textures.len()
        }

        /// Check if no textures are loaded.
        pub fn is_empty(&self) -> bool {
            self.textures.is_empty()
        }

        /// Get an iterator over all texture names.
        pub fn texture_names(&self) -> impl Iterator<Item = &String> {
            self.textures.keys()
        }
    }
}

/// Block type definitions and texture mappings
pub mod blocks {
    /// Types of blocks that can exist in the game world
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum BlockType {
        /// Grass block - green top, dirt bottom, grass sides
        Grass,
        /// Dirt block - same texture on all sides
        Dirt,
        /// Stone block - same texture on all sides
        Stone,
        /// Cobblestone block - same texture on all sides
        Cobblestone,
        /// Wood (oak planks) block - same texture on all sides
        Wood,
    }

    /// Texture names for each face of a block
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BlockFaceTextures {
        /// Texture name for the top face (+Y)
        pub top: &'static str,
        /// Texture name for the bottom face (-Y)
        pub bottom: &'static str,
        /// Texture name for the north face (-Z)
        pub north: &'static str,
        /// Texture name for the south face (+Z)
        pub south: &'static str,
        /// Texture name for the east face (+X)
        pub east: &'static str,
        /// Texture name for the west face (-X)
        pub west: &'static str,
    }

    impl BlockFaceTextures {
        /// Create a BlockFaceTextures where all faces use the same texture
        pub const fn uniform(texture: &'static str) -> Self {
            Self {
                top: texture,
                bottom: texture,
                north: texture,
                south: texture,
                east: texture,
                west: texture,
            }
        }

        /// Create a BlockFaceTextures with different top, bottom, and side textures
        pub const fn top_bottom_sides(
            top: &'static str,
            bottom: &'static str,
            sides: &'static str,
        ) -> Self {
            Self {
                top,
                bottom,
                north: sides,
                south: sides,
                east: sides,
                west: sides,
            }
        }
    }

    impl BlockType {
        /// Get the texture names for each face of this block type
        pub fn textures(&self) -> BlockFaceTextures {
            match self {
                BlockType::Grass => BlockFaceTextures::top_bottom_sides(
                    "grass_top",
                    "dirt",
                    "grass_side_carried",
                ),
                BlockType::Dirt => BlockFaceTextures::uniform("dirt"),
                BlockType::Stone => BlockFaceTextures::uniform("stone"),
                BlockType::Cobblestone => BlockFaceTextures::uniform("cobblestone"),
                BlockType::Wood => BlockFaceTextures::uniform("planks_oak"),
            }
        }

        /// Get all available block types
        pub fn all() -> &'static [BlockType] {
            &[
                BlockType::Grass,
                BlockType::Dirt,
                BlockType::Stone,
                BlockType::Cobblestone,
                BlockType::Wood,
            ]
        }
    }
}

/// Voxel world generation and management
pub mod voxel {
    use crate::blocks::BlockType;
    use crate::render::TexturedVertex;
    use std::collections::HashMap;

    /// World size constants
    pub const WORLD_SIZE_X: i32 = 16;
    pub const WORLD_SIZE_Z: i32 = 16;
    pub const WORLD_MIN_Y: i32 = -4;
    pub const WORLD_MAX_Y: i32 = 0;

    /// A voxel world containing block data
    #[derive(Debug, Clone)]
    pub struct VoxelWorld {
        /// Block data stored by position (x, y, z)
        pub blocks: HashMap<(i32, i32, i32), BlockType>,
    }

    impl VoxelWorld {
        /// Create an empty voxel world
        pub fn new() -> Self {
            Self {
                blocks: HashMap::new(),
            }
        }

        /// Generate a flat world with layered terrain:
        /// - y=0: Grass blocks
        /// - y=-1, y=-2: Dirt blocks
        /// - y=-3, y=-4: Stone blocks
        ///
        /// World spans x: -8 to 7, z: -8 to 7 (16x16 blocks centered at origin)
        pub fn generate_flat_world() -> Self {
            let mut world = Self::new();

            let half_x = WORLD_SIZE_X / 2;
            let half_z = WORLD_SIZE_Z / 2;

            for x in -half_x..half_x {
                for z in -half_z..half_z {
                    for y in WORLD_MIN_Y..=WORLD_MAX_Y {
                        let block_type = match y {
                            0 => BlockType::Grass,
                            -1 | -2 => BlockType::Dirt,
                            _ => BlockType::Stone, // y <= -3
                        };
                        world.blocks.insert((x, y, z), block_type);
                    }
                }
            }

            world
        }

        /// Get the block at the given position, if any
        pub fn get_block(&self, x: i32, y: i32, z: i32) -> Option<&BlockType> {
            self.blocks.get(&(x, y, z))
        }

        /// Set a block at the given position
        pub fn set_block(&mut self, x: i32, y: i32, z: i32, block_type: BlockType) {
            self.blocks.insert((x, y, z), block_type);
        }

        /// Remove a block at the given position, returning the removed block type if any
        pub fn remove_block(&mut self, x: i32, y: i32, z: i32) -> Option<BlockType> {
            self.blocks.remove(&(x, y, z))
        }

        /// Get the number of blocks in the world
        pub fn block_count(&self) -> usize {
            self.blocks.len()
        }

        /// Build a mesh of all blocks in the world with face culling optimization.
        /// Only renders faces that are not adjacent to another solid block.
        /// Returns (vertices, indices) for rendering with the textured pipeline.
        pub fn build_mesh(&self) -> (Vec<TexturedVertex>, Vec<u16>) {
            use crate::render::{BlockFace, create_textured_face_vertices, create_face_indices};

            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            let mut vertex_count = 0u16;

            for ((x, y, z), _block_type) in &self.blocks {
                // Convert block coordinates to world position (each block is 1x1x1)
                let offset = (*x as f32, *y as f32, *z as f32);

                // Check each face - only render if no adjacent block exists
                for face in BlockFace::all() {
                    let (dx, dy, dz) = face.neighbor_offset();
                    let neighbor_pos = (*x + dx, *y + dy, *z + dz);

                    // Only render this face if there's no solid neighbor
                    if self.blocks.get(&neighbor_pos).is_none() {
                        // Add 4 vertices for this face
                        let face_verts = create_textured_face_vertices(face, offset, [1.0, 1.0, 1.0]);
                        vertices.extend_from_slice(&face_verts);

                        // Add 6 indices for this face
                        let face_indices = create_face_indices(vertex_count);
                        indices.extend_from_slice(&face_indices);

                        vertex_count += 4;
                    }
                }
            }

            (vertices, indices)
        }
    }

    impl Default for VoxelWorld {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// Raycasting module for block interaction (placement and destruction)
pub mod raycast {
    use super::voxel::VoxelWorld;
    use glam::Vec3;

    /// Result of a successful raycast hit
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct RaycastHit {
        /// The block position that was hit
        pub block_pos: (i32, i32, i32),
        /// The face normal of the hit (used for block placement)
        pub face_normal: (i32, i32, i32),
    }

    /// Cast a ray through the voxel world using DDA algorithm.
    /// Returns the first solid block hit within max_distance, along with the face normal.
    ///
    /// # Arguments
    /// * `world` - The voxel world to raycast against
    /// * `origin` - Ray starting position (usually camera/eye position)
    /// * `direction` - Ray direction (normalized look direction)
    /// * `max_distance` - Maximum distance to check (in blocks)
    pub fn raycast_voxel(
        world: &VoxelWorld,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
    ) -> Option<RaycastHit> {
        // Normalize direction to ensure consistent stepping
        let dir = direction.normalize();

        // Handle zero direction components to avoid division by zero
        let dir_x = if dir.x.abs() < 1e-10 { 1e-10 } else { dir.x };
        let dir_y = if dir.y.abs() < 1e-10 { 1e-10 } else { dir.y };
        let dir_z = if dir.z.abs() < 1e-10 { 1e-10 } else { dir.z };

        // Current voxel position
        let mut voxel_x = origin.x.floor() as i32;
        let mut voxel_y = origin.y.floor() as i32;
        let mut voxel_z = origin.z.floor() as i32;

        // Step direction for each axis (-1 or 1)
        let step_x = if dir_x > 0.0 { 1 } else { -1 };
        let step_y = if dir_y > 0.0 { 1 } else { -1 };
        let step_z = if dir_z > 0.0 { 1 } else { -1 };

        // Distance along ray to next voxel boundary for each axis
        let t_delta_x = (1.0 / dir_x).abs();
        let t_delta_y = (1.0 / dir_y).abs();
        let t_delta_z = (1.0 / dir_z).abs();

        // Initial t values to first voxel boundary
        let mut t_max_x = if dir_x > 0.0 {
            ((voxel_x + 1) as f32 - origin.x) / dir_x
        } else {
            (voxel_x as f32 - origin.x) / dir_x
        };
        let mut t_max_y = if dir_y > 0.0 {
            ((voxel_y + 1) as f32 - origin.y) / dir_y
        } else {
            (voxel_y as f32 - origin.y) / dir_y
        };
        let mut t_max_z = if dir_z > 0.0 {
            ((voxel_z + 1) as f32 - origin.z) / dir_z
        } else {
            (voxel_z as f32 - origin.z) / dir_z
        };

        // Track which face we entered from
        let mut face_normal = (0, 0, 0);
        let mut distance_traveled = 0.0;

        // DDA loop
        while distance_traveled < max_distance {
            // Check if current voxel contains a block
            if world.get_block(voxel_x, voxel_y, voxel_z).is_some() {
                return Some(RaycastHit {
                    block_pos: (voxel_x, voxel_y, voxel_z),
                    face_normal,
                });
            }

            // Step to next voxel boundary along the closest axis
            if t_max_x < t_max_y && t_max_x < t_max_z {
                distance_traveled = t_max_x;
                t_max_x += t_delta_x;
                voxel_x += step_x;
                face_normal = (-step_x, 0, 0);
            } else if t_max_y < t_max_z {
                distance_traveled = t_max_y;
                t_max_y += t_delta_y;
                voxel_y += step_y;
                face_normal = (0, -step_y, 0);
            } else {
                distance_traveled = t_max_z;
                t_max_z += t_delta_z;
                voxel_z += step_z;
                face_normal = (0, 0, -step_z);
            }
        }

        None
    }
}

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

/// Rendering module with wgpu initialization and render loop
pub mod render {
    use glam::{Mat4, Vec3};
    use std::sync::Arc;
    use wgpu::util::DeviceExt;
    use wgpu::{
        include_wgsl, BindGroup, BindGroupDescriptor, BindGroupEntry,
        BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType,
        BufferUsages, Color, CommandEncoderDescriptor, Device, DeviceDescriptor, Features,
        FragmentState, FrontFace, IndexFormat, Instance, InstanceDescriptor, Limits, LoadOp,
        MultisampleState, Operations, PipelineLayoutDescriptor, PolygonMode, PowerPreference,
        PrimitiveState, PrimitiveTopology, Queue, RenderPassColorAttachment, RenderPassDescriptor,
        RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, ShaderStages, StoreOp,
        Surface, SurfaceConfiguration, TextureUsages, TextureViewDescriptor, VertexAttribute,
        VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
    };
    use winit::dpi::PhysicalSize;
    use winit::window::Window;

    /// Window title for the game client
    pub const WINDOW_TITLE: &str = "3D Multiplayer Game";

    /// Sky blue clear color (#87CEEB = RGB 135, 206, 235)
    pub const SKY_BLUE: Color = Color {
        r: 135.0 / 255.0,
        g: 206.0 / 255.0,
        b: 235.0 / 255.0,
        a: 1.0,
    };

    /// Default camera field of view in radians (60 degrees)
    pub const DEFAULT_FOV: f32 = std::f32::consts::PI / 3.0;

    /// Default near clipping plane distance
    pub const DEFAULT_NEAR: f32 = 0.1;

    /// Default far clipping plane distance
    pub const DEFAULT_FAR: f32 = 100.0;

    /// Perspective camera with view and projection matrices
    #[derive(Debug, Clone, Copy)]
    pub struct Camera {
        /// Camera position in world space
        pub position: Vec3,
        /// Point the camera is looking at
        pub target: Vec3,
        /// Up vector (typically Y-up)
        pub up: Vec3,
        /// Field of view in radians
        pub fov: f32,
        /// Aspect ratio (width / height)
        pub aspect_ratio: f32,
        /// Near clipping plane
        pub near: f32,
        /// Far clipping plane
        pub far: f32,
    }

    /// Calculate model matrix for a player at given position and rotation
    /// Rotation is around Y axis (yaw) in radians
    pub fn player_model_matrix(position: Vec3, rotation_yaw: f32) -> Mat4 {
        let translation = Mat4::from_translation(position);
        let rotation = Mat4::from_rotation_y(rotation_yaw);
        translation * rotation
    }

    impl Camera {
        /// Create a new camera with default parameters
        pub fn new(position: Vec3, target: Vec3, aspect_ratio: f32) -> Self {
            Self {
                position,
                target,
                up: Vec3::Y,
                fov: DEFAULT_FOV,
                aspect_ratio,
                near: DEFAULT_NEAR,
                far: DEFAULT_FAR,
            }
        }

        /// Calculate the view matrix (world space to camera space)
        pub fn view_matrix(&self) -> Mat4 {
            Mat4::look_at_rh(self.position, self.target, self.up)
        }

        /// Calculate the projection matrix (camera space to clip space)
        pub fn projection_matrix(&self) -> Mat4 {
            Mat4::perspective_rh(self.fov, self.aspect_ratio, self.near, self.far)
        }

        /// Calculate the combined view-projection matrix
        pub fn view_projection_matrix(&self) -> Mat4 {
            self.projection_matrix() * self.view_matrix()
        }
    }

    /// Light uniforms for directional (sun) lighting
    /// Layout matches WGSL struct with proper alignment (vec3 followed by padding)
    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct LightUniforms {
        /// Direction the sun is pointing (towards the scene)
        pub sun_direction: [f32; 3],
        /// Padding to align sun_color to 16 bytes
        pub _padding1: f32,
        /// Color/intensity of the sun light
        pub sun_color: [f32; 3],
        /// Ambient light strength (0.0 - 1.0)
        pub ambient_strength: f32,
    }

    impl Default for LightUniforms {
        fn default() -> Self {
            // Sun direction pointing down and slightly to the side (normalized)
            let dir = Vec3::new(-0.5, -1.0, -0.3).normalize();
            Self {
                sun_direction: [dir.x, dir.y, dir.z],
                _padding1: 0.0,
                sun_color: [1.0, 1.0, 0.95], // Slightly warm white
                ambient_strength: 0.3, // 30% ambient light
            }
        }
    }

    /// Vertex with position, color, and normal for the cube mesh
    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Vertex {
        pub position: [f32; 3],
        pub color: [f32; 3],
        pub normal: [f32; 3],
    }

    impl Vertex {
        /// Vertex buffer layout descriptor
        pub fn desc() -> VertexBufferLayout<'static> {
            VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: VertexStepMode::Vertex,
                attributes: &[
                    VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: VertexFormat::Float32x3,
                    },
                    VertexAttribute {
                        offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                        shader_location: 1,
                        format: VertexFormat::Float32x3,
                    },
                    VertexAttribute {
                        offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                        shader_location: 2,
                        format: VertexFormat::Float32x3,
                    },
                ],
            }
        }
    }

    /// Vertex with position, UV coordinates, color, and normal for textured rendering.
    /// Each face of a cube needs unique vertices for proper UV mapping (24 vertices per cube).
    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct TexturedVertex {
        pub position: [f32; 3],
        pub uv: [f32; 2],
        pub color: [f32; 3],
        pub normal: [f32; 3],
    }

    impl TexturedVertex {
        /// Vertex buffer layout descriptor for textured vertices
        pub fn desc() -> VertexBufferLayout<'static> {
            VertexBufferLayout {
                array_stride: std::mem::size_of::<TexturedVertex>() as wgpu::BufferAddress,
                step_mode: VertexStepMode::Vertex,
                attributes: &[
                    // position: shader_location 0
                    VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: VertexFormat::Float32x3,
                    },
                    // uv: shader_location 1
                    VertexAttribute {
                        offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                        shader_location: 1,
                        format: VertexFormat::Float32x2,
                    },
                    // color: shader_location 2
                    VertexAttribute {
                        offset: (std::mem::size_of::<[f32; 3]>() + std::mem::size_of::<[f32; 2]>()) as wgpu::BufferAddress,
                        shader_location: 2,
                        format: VertexFormat::Float32x3,
                    },
                    // normal: shader_location 3
                    VertexAttribute {
                        offset: (std::mem::size_of::<[f32; 3]>() + std::mem::size_of::<[f32; 2]>() + std::mem::size_of::<[f32; 3]>()) as wgpu::BufferAddress,
                        shader_location: 3,
                        format: VertexFormat::Float32x3,
                    },
                ],
            }
        }
    }

    /// Face direction for block face culling
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BlockFace {
        /// +X direction (right)
        PosX,
        /// -X direction (left)
        NegX,
        /// +Y direction (top)
        PosY,
        /// -Y direction (bottom)
        NegY,
        /// +Z direction (front)
        PosZ,
        /// -Z direction (back)
        NegZ,
    }

    impl BlockFace {
        /// Returns the neighbor offset for this face direction
        pub fn neighbor_offset(&self) -> (i32, i32, i32) {
            match self {
                BlockFace::PosX => (1, 0, 0),
                BlockFace::NegX => (-1, 0, 0),
                BlockFace::PosY => (0, 1, 0),
                BlockFace::NegY => (0, -1, 0),
                BlockFace::PosZ => (0, 0, 1),
                BlockFace::NegZ => (0, 0, -1),
            }
        }

        /// Returns all 6 face directions
        pub fn all() -> [BlockFace; 6] {
            [
                BlockFace::PosX,
                BlockFace::NegX,
                BlockFace::PosY,
                BlockFace::NegY,
                BlockFace::PosZ,
                BlockFace::NegZ,
            ]
        }
    }

    /// Create 4 vertices for a single face of a textured cube.
    /// Used for face culling optimization - only creates vertices for visible faces.
    pub fn create_textured_face_vertices(
        face: BlockFace,
        offset: (f32, f32, f32),
        color: [f32; 3],
    ) -> [TexturedVertex; 4] {
        let (ox, oy, oz) = offset;
        // Block size is 1.0, half-size is 0.5
        let w = 0.5;
        let h = 0.5;
        let d = 0.5;

        match face {
            BlockFace::PosZ => [
                // Front face (+Z)
                TexturedVertex { position: [ox - w, oy - h, oz + d], uv: [0.0, 1.0], color, normal: [0.0, 0.0, 1.0] },
                TexturedVertex { position: [ox + w, oy - h, oz + d], uv: [1.0, 1.0], color, normal: [0.0, 0.0, 1.0] },
                TexturedVertex { position: [ox + w, oy + h, oz + d], uv: [1.0, 0.0], color, normal: [0.0, 0.0, 1.0] },
                TexturedVertex { position: [ox - w, oy + h, oz + d], uv: [0.0, 0.0], color, normal: [0.0, 0.0, 1.0] },
            ],
            BlockFace::NegZ => [
                // Back face (-Z)
                TexturedVertex { position: [ox + w, oy - h, oz - d], uv: [0.0, 1.0], color, normal: [0.0, 0.0, -1.0] },
                TexturedVertex { position: [ox - w, oy - h, oz - d], uv: [1.0, 1.0], color, normal: [0.0, 0.0, -1.0] },
                TexturedVertex { position: [ox - w, oy + h, oz - d], uv: [1.0, 0.0], color, normal: [0.0, 0.0, -1.0] },
                TexturedVertex { position: [ox + w, oy + h, oz - d], uv: [0.0, 0.0], color, normal: [0.0, 0.0, -1.0] },
            ],
            BlockFace::PosX => [
                // Right face (+X)
                TexturedVertex { position: [ox + w, oy - h, oz + d], uv: [0.0, 1.0], color, normal: [1.0, 0.0, 0.0] },
                TexturedVertex { position: [ox + w, oy - h, oz - d], uv: [1.0, 1.0], color, normal: [1.0, 0.0, 0.0] },
                TexturedVertex { position: [ox + w, oy + h, oz - d], uv: [1.0, 0.0], color, normal: [1.0, 0.0, 0.0] },
                TexturedVertex { position: [ox + w, oy + h, oz + d], uv: [0.0, 0.0], color, normal: [1.0, 0.0, 0.0] },
            ],
            BlockFace::NegX => [
                // Left face (-X)
                TexturedVertex { position: [ox - w, oy - h, oz - d], uv: [0.0, 1.0], color, normal: [-1.0, 0.0, 0.0] },
                TexturedVertex { position: [ox - w, oy - h, oz + d], uv: [1.0, 1.0], color, normal: [-1.0, 0.0, 0.0] },
                TexturedVertex { position: [ox - w, oy + h, oz + d], uv: [1.0, 0.0], color, normal: [-1.0, 0.0, 0.0] },
                TexturedVertex { position: [ox - w, oy + h, oz - d], uv: [0.0, 0.0], color, normal: [-1.0, 0.0, 0.0] },
            ],
            BlockFace::PosY => [
                // Top face (+Y)
                TexturedVertex { position: [ox - w, oy + h, oz + d], uv: [0.0, 1.0], color, normal: [0.0, 1.0, 0.0] },
                TexturedVertex { position: [ox + w, oy + h, oz + d], uv: [1.0, 1.0], color, normal: [0.0, 1.0, 0.0] },
                TexturedVertex { position: [ox + w, oy + h, oz - d], uv: [1.0, 0.0], color, normal: [0.0, 1.0, 0.0] },
                TexturedVertex { position: [ox - w, oy + h, oz - d], uv: [0.0, 0.0], color, normal: [0.0, 1.0, 0.0] },
            ],
            BlockFace::NegY => [
                // Bottom face (-Y)
                TexturedVertex { position: [ox - w, oy - h, oz - d], uv: [0.0, 1.0], color, normal: [0.0, -1.0, 0.0] },
                TexturedVertex { position: [ox + w, oy - h, oz - d], uv: [1.0, 1.0], color, normal: [0.0, -1.0, 0.0] },
                TexturedVertex { position: [ox + w, oy - h, oz + d], uv: [1.0, 0.0], color, normal: [0.0, -1.0, 0.0] },
                TexturedVertex { position: [ox - w, oy - h, oz + d], uv: [0.0, 0.0], color, normal: [0.0, -1.0, 0.0] },
            ],
        }
    }

    /// Generate indices for a single face (4-vertex layout).
    /// Pattern: v0, v1, v2, v2, v3, v0 for each quad face.
    /// Returns 6 indices (2 triangles × 3 vertices).
    pub const fn create_face_indices(start: u16) -> [u16; 6] {
        [start, start + 1, start + 2, start + 2, start + 3, start]
    }

    /// Cube vertices with positions, colors, and normals (24 vertices, 4 per face)
    pub const CUBE_VERTICES: &[Vertex] = &[
        // Front face (+Z)
        Vertex { position: [-0.5, -0.5,  0.5], color: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 1.0] },
        Vertex { position: [ 0.5, -0.5,  0.5], color: [0.0, 1.0, 0.0], normal: [0.0, 0.0, 1.0] },
        Vertex { position: [ 0.5,  0.5,  0.5], color: [0.0, 0.0, 1.0], normal: [0.0, 0.0, 1.0] },
        Vertex { position: [-0.5,  0.5,  0.5], color: [1.0, 1.0, 0.0], normal: [0.0, 0.0, 1.0] },
        // Back face (-Z)
        Vertex { position: [ 0.5, -0.5, -0.5], color: [0.0, 1.0, 1.0], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [-0.5, -0.5, -0.5], color: [1.0, 0.0, 1.0], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [-0.5,  0.5, -0.5], color: [0.5, 0.5, 0.5], normal: [0.0, 0.0, -1.0] },
        Vertex { position: [ 0.5,  0.5, -0.5], color: [1.0, 1.0, 1.0], normal: [0.0, 0.0, -1.0] },
        // Right face (+X)
        Vertex { position: [ 0.5, -0.5,  0.5], color: [0.0, 1.0, 0.0], normal: [1.0, 0.0, 0.0] },
        Vertex { position: [ 0.5, -0.5, -0.5], color: [0.0, 1.0, 1.0], normal: [1.0, 0.0, 0.0] },
        Vertex { position: [ 0.5,  0.5, -0.5], color: [1.0, 1.0, 1.0], normal: [1.0, 0.0, 0.0] },
        Vertex { position: [ 0.5,  0.5,  0.5], color: [0.0, 0.0, 1.0], normal: [1.0, 0.0, 0.0] },
        // Left face (-X)
        Vertex { position: [-0.5, -0.5, -0.5], color: [1.0, 0.0, 1.0], normal: [-1.0, 0.0, 0.0] },
        Vertex { position: [-0.5, -0.5,  0.5], color: [1.0, 0.0, 0.0], normal: [-1.0, 0.0, 0.0] },
        Vertex { position: [-0.5,  0.5,  0.5], color: [1.0, 1.0, 0.0], normal: [-1.0, 0.0, 0.0] },
        Vertex { position: [-0.5,  0.5, -0.5], color: [0.5, 0.5, 0.5], normal: [-1.0, 0.0, 0.0] },
        // Top face (+Y)
        Vertex { position: [-0.5,  0.5,  0.5], color: [1.0, 1.0, 0.0], normal: [0.0, 1.0, 0.0] },
        Vertex { position: [ 0.5,  0.5,  0.5], color: [0.0, 0.0, 1.0], normal: [0.0, 1.0, 0.0] },
        Vertex { position: [ 0.5,  0.5, -0.5], color: [1.0, 1.0, 1.0], normal: [0.0, 1.0, 0.0] },
        Vertex { position: [-0.5,  0.5, -0.5], color: [0.5, 0.5, 0.5], normal: [0.0, 1.0, 0.0] },
        // Bottom face (-Y)
        Vertex { position: [-0.5, -0.5, -0.5], color: [1.0, 0.0, 1.0], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [ 0.5, -0.5, -0.5], color: [0.0, 1.0, 1.0], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [ 0.5, -0.5,  0.5], color: [0.0, 1.0, 0.0], normal: [0.0, -1.0, 0.0] },
        Vertex { position: [-0.5, -0.5,  0.5], color: [1.0, 0.0, 0.0], normal: [0.0, -1.0, 0.0] },
    ];

    /// Cube indices for indexed drawing (12 triangles = 36 indices)
    /// Using 24-vertex layout with 4 vertices per face
    pub const CUBE_INDICES: &[u16] = &[
        // Front face (vertices 0-3)
        0, 1, 2, 2, 3, 0,
        // Back face (vertices 4-7)
        4, 5, 6, 6, 7, 4,
        // Right face (vertices 8-11)
        8, 9, 10, 10, 11, 8,
        // Left face (vertices 12-15)
        12, 13, 14, 14, 15, 12,
        // Top face (vertices 16-19)
        16, 17, 18, 18, 19, 16,
        // Bottom face (vertices 20-23)
        20, 21, 22, 22, 23, 20,
    ];

    /// Dark gray color for the ground plane (#404040 = 0.251, 0.251, 0.251)
    const GROUND_GRAY: [f32; 3] = [0.251, 0.251, 0.251];

    /// Upward normal for ground plane
    const UP_NORMAL: [f32; 3] = [0.0, 1.0, 0.0];

    /// Ground plane vertices (50x50 quad at y=0, centered at origin)
    pub const GROUND_VERTICES: &[Vertex] = &[
        Vertex { position: [-25.0, 0.0, -25.0], color: GROUND_GRAY, normal: UP_NORMAL }, // 0: back-left
        Vertex { position: [ 25.0, 0.0, -25.0], color: GROUND_GRAY, normal: UP_NORMAL }, // 1: back-right
        Vertex { position: [ 25.0, 0.0,  25.0], color: GROUND_GRAY, normal: UP_NORMAL }, // 2: front-right
        Vertex { position: [-25.0, 0.0,  25.0], color: GROUND_GRAY, normal: UP_NORMAL }, // 3: front-left
    ];

    /// Ground plane indices (2 triangles = 6 indices)
    pub const GROUND_INDICES: &[u16] = &[
        0, 1, 2, // first triangle
        2, 3, 0, // second triangle
    ];

    // Humanoid character body part dimensions (per RENDERING.md spec)
    // Head: 0.4×0.4×0.4 at y=1.6
    // Torso: 0.6×0.8×0.3 at y=1.0
    // Arms: 0.2×0.6×0.2 at ±0.4 from center, y=1.0
    // Legs: 0.2×0.8×0.2 at ±0.15 from center, y=0.4

    /// Local player color: Blue (#4444FF)
    pub const PLAYER_BLUE: [f32; 3] = [0.267, 0.267, 1.0];

    /// Other player color: Red (#FF4444)
    pub const OTHER_PLAYER_RED: [f32; 3] = [1.0, 0.267, 0.267];

    /// Static environment object definition
    pub struct StaticObject {
        pub position: (f32, f32, f32),
        pub size: (f32, f32, f32),
        pub color: [f32; 3],
    }

    /// Dynamic box that can be moved at runtime (e.g., pushable boxes).
    /// Unlike StaticObject, DynamicBox positions can change during gameplay.
    #[derive(Debug, Clone, Copy)]
    pub struct DynamicBox {
        /// Center position of the box in world space
        pub position: Vec3,
        /// Size of the box (width, height, depth)
        pub size: (f32, f32, f32),
        /// RGB color of the box
        pub color: [f32; 3],
    }

    impl DynamicBox {
        /// Create a new dynamic box with the given parameters.
        pub fn new(position: Vec3, size: (f32, f32, f32), color: [f32; 3]) -> Self {
            Self { position, size, color }
        }
    }

    /// Build mesh vertices and indices for a single dynamic box.
    /// Returns (vertices, indices) that can be used to create GPU buffers.
    pub fn build_dynamic_box_mesh(dynamic_box: &DynamicBox) -> (Vec<Vertex>, Vec<u16>) {
        let vertices = create_cube_vertices(
            dynamic_box.size,
            (dynamic_box.position.x, dynamic_box.position.y, dynamic_box.position.z),
            dynamic_box.color,
        );
        let indices = create_cube_indices(0);
        (vertices.to_vec(), indices.to_vec())
    }

    /// Build mesh vertices and indices for multiple dynamic boxes.
    /// Returns combined (vertices, indices) for all boxes.
    pub fn build_dynamic_boxes_mesh(boxes: &[DynamicBox]) -> (Vec<Vertex>, Vec<u16>) {
        let mut vertices = Vec::with_capacity(boxes.len() * 8);
        let mut indices = Vec::with_capacity(boxes.len() * 36);

        for (i, dynamic_box) in boxes.iter().enumerate() {
            let cube_verts = create_cube_vertices(
                dynamic_box.size,
                (dynamic_box.position.x, dynamic_box.position.y, dynamic_box.position.z),
                dynamic_box.color,
            );
            vertices.extend_from_slice(&cube_verts);

            let cube_indices = create_cube_indices((i * 8) as u16);
            indices.extend_from_slice(&cube_indices);
        }

        (vertices, indices)
    }

    // Colors for static objects
    const GRAY: [f32; 3] = [0.5, 0.5, 0.5];
    const DARK_GRAY: [f32; 3] = [0.3, 0.3, 0.3];
    const BROWN: [f32; 3] = [0.545, 0.271, 0.075];
    const GREEN: [f32; 3] = [0.2, 0.6, 0.2];

    /// Static environment objects scattered around the scene
    pub const STATIC_OBJECTS: &[StaticObject] = &[
        // Cubes
        StaticObject { position: (10.0, 1.0, 10.0), size: (2.0, 2.0, 2.0), color: GRAY },
        StaticObject { position: (-15.0, 0.5, 5.0), size: (1.0, 1.0, 1.0), color: BROWN },
        StaticObject { position: (8.0, 1.5, -12.0), size: (3.0, 3.0, 3.0), color: DARK_GRAY },
        StaticObject { position: (-5.0, 0.75, -18.0), size: (1.5, 1.5, 1.5), color: GREEN },
        StaticObject { position: (18.0, 1.0, -5.0), size: (2.0, 2.0, 2.0), color: BROWN },
        // Walls (tall thin boxes)
        StaticObject { position: (-20.0, 2.0, 0.0), size: (1.0, 4.0, 8.0), color: GRAY },
        StaticObject { position: (0.0, 2.0, -20.0), size: (8.0, 4.0, 1.0), color: DARK_GRAY },
        StaticObject { position: (15.0, 1.5, 15.0), size: (0.5, 3.0, 6.0), color: BROWN },
    ];

    /// Generate cube vertices at a given offset with a given size and color.
    /// Returns 24 vertices (4 per face) with proper normals for lighting.
    /// Size is (width, height, depth) and offset is the center position.
    fn create_cube_vertices(
        size: (f32, f32, f32),
        offset: (f32, f32, f32),
        color: [f32; 3],
    ) -> [Vertex; 24] {
        let (w, h, d) = (size.0 / 2.0, size.1 / 2.0, size.2 / 2.0);
        let (ox, oy, oz) = offset;
        [
            // Front face (+Z) - vertices 0-3
            Vertex { position: [ox - w, oy - h, oz + d], color, normal: [0.0, 0.0, 1.0] },
            Vertex { position: [ox + w, oy - h, oz + d], color, normal: [0.0, 0.0, 1.0] },
            Vertex { position: [ox + w, oy + h, oz + d], color, normal: [0.0, 0.0, 1.0] },
            Vertex { position: [ox - w, oy + h, oz + d], color, normal: [0.0, 0.0, 1.0] },
            // Back face (-Z) - vertices 4-7
            Vertex { position: [ox + w, oy - h, oz - d], color, normal: [0.0, 0.0, -1.0] },
            Vertex { position: [ox - w, oy - h, oz - d], color, normal: [0.0, 0.0, -1.0] },
            Vertex { position: [ox - w, oy + h, oz - d], color, normal: [0.0, 0.0, -1.0] },
            Vertex { position: [ox + w, oy + h, oz - d], color, normal: [0.0, 0.0, -1.0] },
            // Right face (+X) - vertices 8-11
            Vertex { position: [ox + w, oy - h, oz + d], color, normal: [1.0, 0.0, 0.0] },
            Vertex { position: [ox + w, oy - h, oz - d], color, normal: [1.0, 0.0, 0.0] },
            Vertex { position: [ox + w, oy + h, oz - d], color, normal: [1.0, 0.0, 0.0] },
            Vertex { position: [ox + w, oy + h, oz + d], color, normal: [1.0, 0.0, 0.0] },
            // Left face (-X) - vertices 12-15
            Vertex { position: [ox - w, oy - h, oz - d], color, normal: [-1.0, 0.0, 0.0] },
            Vertex { position: [ox - w, oy - h, oz + d], color, normal: [-1.0, 0.0, 0.0] },
            Vertex { position: [ox - w, oy + h, oz + d], color, normal: [-1.0, 0.0, 0.0] },
            Vertex { position: [ox - w, oy + h, oz - d], color, normal: [-1.0, 0.0, 0.0] },
            // Top face (+Y) - vertices 16-19
            Vertex { position: [ox - w, oy + h, oz + d], color, normal: [0.0, 1.0, 0.0] },
            Vertex { position: [ox + w, oy + h, oz + d], color, normal: [0.0, 1.0, 0.0] },
            Vertex { position: [ox + w, oy + h, oz - d], color, normal: [0.0, 1.0, 0.0] },
            Vertex { position: [ox - w, oy + h, oz - d], color, normal: [0.0, 1.0, 0.0] },
            // Bottom face (-Y) - vertices 20-23
            Vertex { position: [ox - w, oy - h, oz - d], color, normal: [0.0, -1.0, 0.0] },
            Vertex { position: [ox + w, oy - h, oz - d], color, normal: [0.0, -1.0, 0.0] },
            Vertex { position: [ox + w, oy - h, oz + d], color, normal: [0.0, -1.0, 0.0] },
            Vertex { position: [ox - w, oy - h, oz + d], color, normal: [0.0, -1.0, 0.0] },
        ]
    }

    /// Generate indices for a cube part, offset by the starting vertex index.
    /// Uses 24-vertex layout with 4 vertices per face.
    const fn create_cube_indices(start: u16) -> [u16; 36] {
        [
            // Front face (vertices 0-3)
            start + 0, start + 1, start + 2, start + 2, start + 3, start + 0,
            // Back face (vertices 4-7)
            start + 4, start + 5, start + 6, start + 6, start + 7, start + 4,
            // Right face (vertices 8-11)
            start + 8, start + 9, start + 10, start + 10, start + 11, start + 8,
            // Left face (vertices 12-15)
            start + 12, start + 13, start + 14, start + 14, start + 15, start + 12,
            // Top face (vertices 16-19)
            start + 16, start + 17, start + 18, start + 18, start + 19, start + 16,
            // Bottom face (vertices 20-23)
            start + 20, start + 21, start + 22, start + 22, start + 23, start + 20,
        ]
    }

    /// Create 24 vertices for a textured cube (4 vertices per face, 6 faces).
    /// Unlike shared-corner cubes, textured cubes need unique vertices per face for proper UV mapping.
    /// UV coordinates map the full texture (0,0 to 1,1) to each face.
    /// Includes normals for directional lighting.
    pub fn create_textured_cube_vertices(
        size: (f32, f32, f32),
        offset: (f32, f32, f32),
        color: [f32; 3],
    ) -> [TexturedVertex; 24] {
        let (w, h, d) = (size.0 / 2.0, size.1 / 2.0, size.2 / 2.0);
        let (ox, oy, oz) = offset;

        [
            // Front face (+Z) - vertices 0-3
            TexturedVertex { position: [ox - w, oy - h, oz + d], uv: [0.0, 1.0], color, normal: [0.0, 0.0, 1.0] },
            TexturedVertex { position: [ox + w, oy - h, oz + d], uv: [1.0, 1.0], color, normal: [0.0, 0.0, 1.0] },
            TexturedVertex { position: [ox + w, oy + h, oz + d], uv: [1.0, 0.0], color, normal: [0.0, 0.0, 1.0] },
            TexturedVertex { position: [ox - w, oy + h, oz + d], uv: [0.0, 0.0], color, normal: [0.0, 0.0, 1.0] },

            // Back face (-Z) - vertices 4-7
            TexturedVertex { position: [ox + w, oy - h, oz - d], uv: [0.0, 1.0], color, normal: [0.0, 0.0, -1.0] },
            TexturedVertex { position: [ox - w, oy - h, oz - d], uv: [1.0, 1.0], color, normal: [0.0, 0.0, -1.0] },
            TexturedVertex { position: [ox - w, oy + h, oz - d], uv: [1.0, 0.0], color, normal: [0.0, 0.0, -1.0] },
            TexturedVertex { position: [ox + w, oy + h, oz - d], uv: [0.0, 0.0], color, normal: [0.0, 0.0, -1.0] },

            // Right face (+X) - vertices 8-11
            TexturedVertex { position: [ox + w, oy - h, oz + d], uv: [0.0, 1.0], color, normal: [1.0, 0.0, 0.0] },
            TexturedVertex { position: [ox + w, oy - h, oz - d], uv: [1.0, 1.0], color, normal: [1.0, 0.0, 0.0] },
            TexturedVertex { position: [ox + w, oy + h, oz - d], uv: [1.0, 0.0], color, normal: [1.0, 0.0, 0.0] },
            TexturedVertex { position: [ox + w, oy + h, oz + d], uv: [0.0, 0.0], color, normal: [1.0, 0.0, 0.0] },

            // Left face (-X) - vertices 12-15
            TexturedVertex { position: [ox - w, oy - h, oz - d], uv: [0.0, 1.0], color, normal: [-1.0, 0.0, 0.0] },
            TexturedVertex { position: [ox - w, oy - h, oz + d], uv: [1.0, 1.0], color, normal: [-1.0, 0.0, 0.0] },
            TexturedVertex { position: [ox - w, oy + h, oz + d], uv: [1.0, 0.0], color, normal: [-1.0, 0.0, 0.0] },
            TexturedVertex { position: [ox - w, oy + h, oz - d], uv: [0.0, 0.0], color, normal: [-1.0, 0.0, 0.0] },

            // Top face (+Y) - vertices 16-19
            TexturedVertex { position: [ox - w, oy + h, oz + d], uv: [0.0, 1.0], color, normal: [0.0, 1.0, 0.0] },
            TexturedVertex { position: [ox + w, oy + h, oz + d], uv: [1.0, 1.0], color, normal: [0.0, 1.0, 0.0] },
            TexturedVertex { position: [ox + w, oy + h, oz - d], uv: [1.0, 0.0], color, normal: [0.0, 1.0, 0.0] },
            TexturedVertex { position: [ox - w, oy + h, oz - d], uv: [0.0, 0.0], color, normal: [0.0, 1.0, 0.0] },

            // Bottom face (-Y) - vertices 20-23
            TexturedVertex { position: [ox - w, oy - h, oz - d], uv: [0.0, 1.0], color, normal: [0.0, -1.0, 0.0] },
            TexturedVertex { position: [ox + w, oy - h, oz - d], uv: [1.0, 1.0], color, normal: [0.0, -1.0, 0.0] },
            TexturedVertex { position: [ox + w, oy - h, oz + d], uv: [1.0, 0.0], color, normal: [0.0, -1.0, 0.0] },
            TexturedVertex { position: [ox - w, oy - h, oz + d], uv: [0.0, 0.0], color, normal: [0.0, -1.0, 0.0] },
        ]
    }

    /// Generate indices for a textured cube (24-vertex layout).
    /// Each face uses 4 vertices with indices: 0,1,2, 2,3,0 pattern.
    /// Returns 36 indices (6 faces × 2 triangles × 3 vertices).
    pub const fn create_textured_cube_indices(start: u16) -> [u16; 36] {
        [
            // Front face (vertices 0-3)
            start + 0, start + 1, start + 2, start + 2, start + 3, start + 0,
            // Back face (vertices 4-7)
            start + 4, start + 5, start + 6, start + 6, start + 7, start + 4,
            // Right face (vertices 8-11)
            start + 8, start + 9, start + 10, start + 10, start + 11, start + 8,
            // Left face (vertices 12-15)
            start + 12, start + 13, start + 14, start + 14, start + 15, start + 12,
            // Top face (vertices 16-19)
            start + 16, start + 17, start + 18, start + 18, start + 19, start + 16,
            // Bottom face (vertices 20-23)
            start + 20, start + 21, start + 22, start + 22, start + 23, start + 20,
        ]
    }

    /// Build mesh for all static environment objects
    /// Returns (vertices, indices) for all static objects combined
    pub fn build_static_objects_mesh() -> (Vec<Vertex>, Vec<u16>) {
        let mut vertices = Vec::with_capacity(STATIC_OBJECTS.len() * 24);
        let mut indices = Vec::with_capacity(STATIC_OBJECTS.len() * 36);

        for (i, obj) in STATIC_OBJECTS.iter().enumerate() {
            let cube_verts = create_cube_vertices(obj.size, obj.position, obj.color);
            vertices.extend_from_slice(&cube_verts);

            let cube_indices = create_cube_indices((i * 24) as u16);
            indices.extend_from_slice(&cube_indices);
        }

        (vertices, indices)
    }

    /// Build mesh for all static environment objects using textured vertices.
    /// Returns (vertices, indices) for all static objects combined with UV mapping.
    pub fn build_textured_static_objects_mesh() -> (Vec<TexturedVertex>, Vec<u16>) {
        let mut vertices = Vec::with_capacity(STATIC_OBJECTS.len() * 24);
        let mut indices = Vec::with_capacity(STATIC_OBJECTS.len() * 36);

        for (i, obj) in STATIC_OBJECTS.iter().enumerate() {
            // Use white color (1.0, 1.0, 1.0) so texture colors show through without tinting
            let cube_verts = create_textured_cube_vertices(obj.size, obj.position, [1.0, 1.0, 1.0]);
            vertices.extend_from_slice(&cube_verts);

            let cube_indices = create_textured_cube_indices((i * 24) as u16);
            indices.extend_from_slice(&cube_indices);
        }

        (vertices, indices)
    }

    /// Build humanoid vertices from body parts
    /// Returns (vertices, indices) for the complete humanoid mesh
    pub fn build_humanoid_mesh(color: [f32; 3]) -> (Vec<Vertex>, Vec<u16>) {
        let mut vertices = Vec::with_capacity(144); // 6 parts * 24 vertices
        let mut indices = Vec::with_capacity(216); // 6 parts * 36 indices

        // Head: 0.4×0.4×0.4 at y=1.6 (center at 1.6, so bottom at 1.4, top at 1.8)
        let head = create_cube_vertices((0.4, 0.4, 0.4), (0.0, 1.6, 0.0), color);
        vertices.extend_from_slice(&head);

        // Torso: 0.6×0.8×0.3 at y=1.0 (center at 1.0, so bottom at 0.6, top at 1.4)
        let torso = create_cube_vertices((0.6, 0.8, 0.3), (0.0, 1.0, 0.0), color);
        vertices.extend_from_slice(&torso);

        // Left Arm: 0.2×0.6×0.2 at x=-0.4, y=1.0
        let left_arm = create_cube_vertices((0.2, 0.6, 0.2), (-0.4, 1.0, 0.0), color);
        vertices.extend_from_slice(&left_arm);

        // Right Arm: 0.2×0.6×0.2 at x=+0.4, y=1.0
        let right_arm = create_cube_vertices((0.2, 0.6, 0.2), (0.4, 1.0, 0.0), color);
        vertices.extend_from_slice(&right_arm);

        // Left Leg: 0.2×0.8×0.2 at x=-0.15, y=0.4 (center at 0.4, so bottom at 0.0, top at 0.8)
        let left_leg = create_cube_vertices((0.2, 0.8, 0.2), (-0.15, 0.4, 0.0), color);
        vertices.extend_from_slice(&left_leg);

        // Right Leg: 0.2×0.8×0.2 at x=+0.15, y=0.4
        let right_leg = create_cube_vertices((0.2, 0.8, 0.2), (0.15, 0.4, 0.0), color);
        vertices.extend_from_slice(&right_leg);

        // Add indices for each part (6 parts, 24 vertices each)
        for i in 0..6 {
            let part_indices = create_cube_indices(i * 24);
            indices.extend_from_slice(&part_indices);
        }

        (vertices, indices)
    }

    /// Holds all wgpu state needed for rendering
    pub struct RenderState<'window> {
        pub surface: Surface<'window>,
        pub device: Device,
        pub queue: Queue,
        pub config: SurfaceConfiguration,
        pub size: PhysicalSize<u32>,
        pub window: Arc<Window>,
        pub camera: Camera,
        /// The voxel world containing all blocks
        pub voxel_world: crate::voxel::VoxelWorld,
        vertex_buffer: Buffer,
        index_buffer: Buffer,
        num_indices: u32,
        ground_vertex_buffer: Buffer,
        ground_index_buffer: Buffer,
        ground_num_indices: u32,
        humanoid_vertex_buffer: Buffer,
        humanoid_index_buffer: Buffer,
        humanoid_num_indices: u32,
        other_player_vertex_buffer: Buffer,
        other_player_index_buffer: Buffer,
        other_player_num_indices: u32,
        static_objects_vertex_buffer: Buffer,
        static_objects_index_buffer: Buffer,
        static_objects_num_indices: u32,
        // Voxel world buffers
        voxel_vertex_buffer: Buffer,
        voxel_index_buffer: Buffer,
        voxel_num_indices: u32,
        // Textured rendering pipeline for voxels
        textured_bind_group: BindGroup,
        textured_pipeline: RenderPipeline,
        uniform_buffer: Buffer,
        #[allow(dead_code)] // Scaffolding for future lighting features
        light_uniform_buffer: Buffer,
        bind_group: BindGroup,
        pipeline: RenderPipeline,
    }

    /// Default headless render dimensions
    pub const HEADLESS_WIDTH: u32 = 800;
    pub const HEADLESS_HEIGHT: u32 = 600;

    /// Holds wgpu state for headless (offscreen) rendering without a window.
    /// Useful for testing, CI/CD pipelines, and server-side rendering.
    pub struct HeadlessRenderState {
        pub device: Device,
        pub queue: Queue,
        pub camera: Camera,
        pub width: u32,
        pub height: u32,
        /// The voxel world containing all blocks
        pub voxel_world: crate::voxel::VoxelWorld,
        render_texture: wgpu::Texture,
        vertex_buffer: Buffer,
        index_buffer: Buffer,
        num_indices: u32,
        ground_vertex_buffer: Buffer,
        ground_index_buffer: Buffer,
        ground_num_indices: u32,
        humanoid_vertex_buffer: Buffer,
        humanoid_index_buffer: Buffer,
        humanoid_num_indices: u32,
        other_player_vertex_buffer: Buffer,
        other_player_index_buffer: Buffer,
        other_player_num_indices: u32,
        // Textured rendering fields
        textured_objects_vertex_buffer: Buffer,
        textured_objects_index_buffer: Buffer,
        textured_objects_num_indices: u32,
        // Voxel world buffers
        voxel_vertex_buffer: Buffer,
        voxel_index_buffer: Buffer,
        voxel_num_indices: u32,
        textured_bind_group: BindGroup,
        textured_pipeline: RenderPipeline,
        uniform_buffer: Buffer,
        #[allow(dead_code)] // Scaffolding for future lighting features
        light_uniform_buffer: Buffer,
        bind_group: BindGroup,
        pipeline: RenderPipeline,
    }

    impl HeadlessRenderState {
        /// Initialize headless wgpu rendering with the given dimensions.
        pub async fn new(width: u32, height: u32) -> Result<Self, String> {
            let instance = Instance::new(InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let adapter = instance
                .request_adapter(&RequestAdapterOptions {
                    power_preference: PowerPreference::default(),
                    compatible_surface: None, // No surface for headless
                    force_fallback_adapter: false,
                })
                .await
                .ok_or_else(|| "Failed to find a suitable GPU adapter for headless rendering".to_string())?;

            let (device, queue) = adapter
                .request_device(
                    &DeviceDescriptor {
                        required_features: Features::empty(),
                        required_limits: Limits::default(),
                        label: Some("Headless Device"),
                    },
                    None,
                )
                .await
                .map_err(|e| format!("Failed to create device: {:?}", e))?;

            // Create render texture for offscreen rendering
            let render_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Headless Render Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
                view_formats: &[],
            });

            // Create camera with aspect ratio for the render dimensions
            let aspect_ratio = width as f32 / height as f32;
            let camera = Camera::new(Vec3::new(0.0, 2.0, 5.0), Vec3::new(0.0, 1.0, 0.0), aspect_ratio);

            // Create vertex buffer
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cube Vertex Buffer"),
                contents: bytemuck::cast_slice(CUBE_VERTICES),
                usage: BufferUsages::VERTEX,
            });

            // Create index buffer
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cube Index Buffer"),
                contents: bytemuck::cast_slice(CUBE_INDICES),
                usage: BufferUsages::INDEX,
            });
            let num_indices = CUBE_INDICES.len() as u32;

            // Create ground vertex buffer
            let ground_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Ground Vertex Buffer"),
                contents: bytemuck::cast_slice(GROUND_VERTICES),
                usage: BufferUsages::VERTEX,
            });

            // Create ground index buffer
            let ground_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Ground Index Buffer"),
                contents: bytemuck::cast_slice(GROUND_INDICES),
                usage: BufferUsages::INDEX,
            });
            let ground_num_indices = GROUND_INDICES.len() as u32;

            // Create humanoid mesh buffers for local player (blue)
            let (humanoid_vertices, humanoid_indices) = build_humanoid_mesh(PLAYER_BLUE);
            let humanoid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Humanoid Vertex Buffer"),
                contents: bytemuck::cast_slice(&humanoid_vertices),
                usage: BufferUsages::VERTEX,
            });
            let humanoid_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Humanoid Index Buffer"),
                contents: bytemuck::cast_slice(&humanoid_indices),
                usage: BufferUsages::INDEX,
            });
            let humanoid_num_indices = humanoid_indices.len() as u32;

            // Create humanoid mesh buffers for other players (red)
            let (other_player_vertices, other_player_indices) = build_humanoid_mesh(OTHER_PLAYER_RED);
            let other_player_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Other Player Vertex Buffer"),
                contents: bytemuck::cast_slice(&other_player_vertices),
                usage: BufferUsages::VERTEX,
            });
            let other_player_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Other Player Index Buffer"),
                contents: bytemuck::cast_slice(&other_player_indices),
                usage: BufferUsages::INDEX,
            });
            let other_player_num_indices = other_player_indices.len() as u32;

            // Create textured static objects mesh buffers
            let (textured_objects_vertices, textured_objects_indices) = build_textured_static_objects_mesh();
            let textured_objects_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Textured Objects Vertex Buffer"),
                contents: bytemuck::cast_slice(&textured_objects_vertices),
                usage: BufferUsages::VERTEX,
            });
            let textured_objects_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Textured Objects Index Buffer"),
                contents: bytemuck::cast_slice(&textured_objects_indices),
                usage: BufferUsages::INDEX,
            });
            let textured_objects_num_indices = textured_objects_indices.len() as u32;

            // Create voxel world mesh buffers (with COPY_DST for dynamic updates)
            let voxel_world = crate::voxel::VoxelWorld::generate_flat_world();
            let (voxel_vertices, voxel_indices) = voxel_world.build_mesh();
            let voxel_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Voxel World Vertex Buffer"),
                contents: bytemuck::cast_slice(&voxel_vertices),
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            });
            let voxel_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Voxel World Index Buffer"),
                contents: bytemuck::cast_slice(&voxel_indices),
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            });
            let voxel_num_indices = voxel_indices.len() as u32;

            // Create uniform buffer for MVP matrix (64 bytes = 4x4 f32 matrix)
            let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("MVP Uniform Buffer"),
                contents: bytemuck::cast_slice(&[Mat4::IDENTITY]),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

            // Create light uniform buffer with default directional light
            let light_uniforms = LightUniforms::default();
            let light_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Light Uniform Buffer"),
                contents: bytemuck::cast_slice(&[light_uniforms]),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

            // Load block texture for textured rendering
            // Try to load cobblestone texture, fall back to creating a default texture if not available
            let texture_path = std::path::Path::new("textures/default-textures/textures/blocks/cobblestone.png");
            let block_texture = if texture_path.exists() {
                crate::texture::load_png(&device, &queue, texture_path)
                    .map_err(|e| format!("Failed to load texture: {}", e))?
            } else {
                // Create a simple 16x16 checkerboard fallback texture
                let mut fallback_data = vec![0u8; 16 * 16 * 4];
                for y in 0..16 {
                    for x in 0..16 {
                        let idx = (y * 16 + x) * 4;
                        let is_light = (x / 4 + y / 4) % 2 == 0;
                        let color = if is_light { 200u8 } else { 100u8 };
                        fallback_data[idx] = color;     // R
                        fallback_data[idx + 1] = color; // G
                        fallback_data[idx + 2] = color; // B
                        fallback_data[idx + 3] = 255;   // A
                    }
                }
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Fallback Block Texture"),
                    size: wgpu::Extent3d {
                        width: 16,
                        height: 16,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &fallback_data,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(16 * 4),
                        rows_per_image: Some(16),
                    },
                    wgpu::Extent3d {
                        width: 16,
                        height: 16,
                        depth_or_array_layers: 1,
                    },
                );
                texture
            };

            let block_texture_view = block_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let block_sampler = crate::texture::create_pixel_art_sampler(&device);

            // Create bind group layout (MVP uniform + light uniform)
            let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("MVP Bind Group Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

            // Create bind group
            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("MVP Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: light_uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            // Create textured bind group layout (MVP uniform + texture + sampler + light uniform)
            let textured_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Textured Bind Group Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

            // Create textured bind group
            let textured_bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Textured Bind Group"),
                layout: &textured_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&block_texture_view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&block_sampler),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: light_uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            // Load shader
            let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));

            // Load textured shader
            let textured_shader = device.create_shader_module(include_wgsl!("textured_shader.wgsl"));

            // Create pipeline layout
            let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            // Create textured pipeline layout
            let textured_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Textured Render Pipeline Layout"),
                bind_group_layouts: &[&textured_bind_group_layout],
                push_constant_ranges: &[],
            });

            // Create render pipeline with texture format (not surface format)
            let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Headless Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::TextureFormat::Rgba8UnormSrgb.into())],
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            });

            // Create textured render pipeline
            let textured_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Headless Textured Render Pipeline"),
                layout: Some(&textured_pipeline_layout),
                vertex: VertexState {
                    module: &textured_shader,
                    entry_point: "vs_main",
                    buffers: &[TexturedVertex::desc()],
                },
                fragment: Some(FragmentState {
                    module: &textured_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::TextureFormat::Rgba8UnormSrgb.into())],
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            });

            Ok(Self {
                device,
                queue,
                camera,
                width,
                height,
                voxel_world,
                render_texture,
                vertex_buffer,
                index_buffer,
                num_indices,
                ground_vertex_buffer,
                ground_index_buffer,
                ground_num_indices,
                humanoid_vertex_buffer,
                humanoid_index_buffer,
                humanoid_num_indices,
                other_player_vertex_buffer,
                other_player_index_buffer,
                other_player_num_indices,
                textured_objects_vertex_buffer,
                textured_objects_index_buffer,
                textured_objects_num_indices,
                voxel_vertex_buffer,
                voxel_index_buffer,
                voxel_num_indices,
                textured_bind_group,
                textured_pipeline,
                uniform_buffer,
                light_uniform_buffer,
                bind_group,
                pipeline,
            })
        }

        /// Render a frame with the local player and other players to the offscreen texture.
        /// Returns a reference to the render texture for screenshot capture.
        pub fn render_with_players(
            &mut self,
            local_position: Vec3,
            local_rotation_yaw: f32,
            other_players: &[(Vec3, f32)],
        ) -> &wgpu::Texture {
            let view = self.render_texture.create_view(&TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Headless Render Encoder"),
                });

            // Draw ground and reference cube with solid-color pipeline
            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Headless Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(SKY_BLUE),
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.pipeline);

                // Draw ground plane first (identity model matrix)
                let ground_mvp = self.camera.view_projection_matrix();
                self.queue
                    .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[ground_mvp]));
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.ground_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.ground_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.ground_num_indices, 0, 0..1);

                // Draw cube (for reference, identity model matrix)
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
            }

            // Draw static environment objects with textured pipeline
            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Headless Textured Objects Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.textured_pipeline);
                render_pass.set_bind_group(0, &self.textured_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.textured_objects_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.textured_objects_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.textured_objects_num_indices, 0, 0..1);
            }

            // Draw voxel world terrain with textured pipeline
            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Headless Voxel World Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.textured_pipeline);
                render_pass.set_bind_group(0, &self.textured_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.voxel_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.voxel_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.voxel_num_indices, 0, 0..1);
            }

            // Draw local player (blue humanoid) at their position
            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Headless Local Player Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                let model = player_model_matrix(local_position, local_rotation_yaw);
                let mvp = self.camera.view_projection_matrix() * model;
                self.queue
                    .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[mvp]));

                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.humanoid_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.humanoid_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.humanoid_num_indices, 0, 0..1);
            }

            // Draw each other player (red humanoid) at their position
            for (position, rotation_yaw) in other_players {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Headless Other Player Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                let model = player_model_matrix(*position, *rotation_yaw);
                let mvp = self.camera.view_projection_matrix() * model;
                self.queue
                    .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[mvp]));

                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.other_player_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.other_player_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.other_player_num_indices, 0, 0..1);
            }

            self.queue.submit(std::iter::once(encoder.finish()));

            &self.render_texture
        }

        /// Get a reference to the render texture for screenshot capture
        pub fn render_texture(&self) -> &wgpu::Texture {
            &self.render_texture
        }

        /// Render a frame with the local player, other players, and dynamic boxes.
        /// Returns a reference to the render texture for screenshot capture.
        pub fn render_with_players_and_boxes(
            &mut self,
            local_position: Vec3,
            local_rotation_yaw: f32,
            other_players: &[(Vec3, f32)],
            dynamic_boxes: &[DynamicBox],
        ) -> &wgpu::Texture {
            let view = self.render_texture.create_view(&TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Headless Render Encoder"),
                });

            // Draw ground and reference cube with solid-color pipeline
            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Headless Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(SKY_BLUE),
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.pipeline);

                // Draw ground plane first (identity model matrix)
                let ground_mvp = self.camera.view_projection_matrix();
                self.queue
                    .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[ground_mvp]));
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.ground_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.ground_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.ground_num_indices, 0, 0..1);

                // Draw cube (for reference, identity model matrix)
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
            }

            // Draw static environment objects with textured pipeline
            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Headless Textured Objects Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.textured_pipeline);
                render_pass.set_bind_group(0, &self.textured_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.textured_objects_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.textured_objects_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.textured_objects_num_indices, 0, 0..1);
            }

            // Draw voxel world terrain with textured pipeline
            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Headless Voxel World Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.textured_pipeline);
                render_pass.set_bind_group(0, &self.textured_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.voxel_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.voxel_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.voxel_num_indices, 0, 0..1);
            }

            // Draw dynamic boxes
            if !dynamic_boxes.is_empty() {
                let (box_vertices, box_indices) = build_dynamic_boxes_mesh(dynamic_boxes);
                let box_vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Dynamic Box Vertex Buffer"),
                    contents: bytemuck::cast_slice(&box_vertices),
                    usage: BufferUsages::VERTEX,
                });
                let box_index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Dynamic Box Index Buffer"),
                    contents: bytemuck::cast_slice(&box_indices),
                    usage: BufferUsages::INDEX,
                });
                let box_num_indices = box_indices.len() as u32;

                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Dynamic Boxes Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                // Dynamic boxes use identity model matrix (positions baked into vertices)
                let mvp = self.camera.view_projection_matrix();
                self.queue
                    .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[mvp]));

                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, box_vertex_buffer.slice(..));
                render_pass.set_index_buffer(box_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..box_num_indices, 0, 0..1);
            }

            // Draw local player (blue humanoid) at their position
            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Headless Local Player Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                let model = player_model_matrix(local_position, local_rotation_yaw);
                let mvp = self.camera.view_projection_matrix() * model;
                self.queue
                    .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[mvp]));

                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.humanoid_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.humanoid_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.humanoid_num_indices, 0, 0..1);
            }

            // Draw each other player (red humanoid) at their position
            for (position, rotation_yaw) in other_players {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Headless Other Player Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                let model = player_model_matrix(*position, *rotation_yaw);
                let mvp = self.camera.view_projection_matrix() * model;
                self.queue
                    .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[mvp]));

                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.other_player_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.other_player_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.other_player_num_indices, 0, 0..1);
            }

            self.queue.submit(std::iter::once(encoder.finish()));

            &self.render_texture
        }

        /// Rebuild voxel mesh buffers after world changes (block placement/destruction).
        /// This recreates the vertex and index buffers from the current voxel_world state.
        pub fn rebuild_voxel_mesh(&mut self) {
            let (voxel_vertices, voxel_indices) = self.voxel_world.build_mesh();

            // Recreate buffers with new data
            self.voxel_vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Voxel World Vertex Buffer"),
                contents: bytemuck::cast_slice(&voxel_vertices),
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            });
            self.voxel_index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Voxel World Index Buffer"),
                contents: bytemuck::cast_slice(&voxel_indices),
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            });
            self.voxel_num_indices = voxel_indices.len() as u32;
        }
    }

    impl<'window> RenderState<'window> {
        /// Initialize wgpu with the given window
        pub async fn new(window: Arc<Window>) -> Self {
            let size = window.inner_size();

            let instance = Instance::new(InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance.create_surface(window.clone()).unwrap();

            let adapter = instance
                .request_adapter(&RequestAdapterOptions {
                    power_preference: PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find a suitable GPU adapter");

            let (device, queue) = adapter
                .request_device(
                    &DeviceDescriptor {
                        required_features: Features::empty(),
                        required_limits: Limits::default(),
                        label: None,
                    },
                    None,
                )
                .await
                .expect("Failed to create device");

            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .copied()
                .find(|f| f.is_srgb())
                .unwrap_or(surface_caps.formats[0]);

            let config = SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width,
                height: size.height,
                present_mode: surface_caps.present_modes[0],
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            // Create camera positioned at (0, 2, 5) looking at player head height
            let aspect_ratio = size.width as f32 / size.height as f32;
            let camera = Camera::new(Vec3::new(0.0, 2.0, 5.0), Vec3::new(0.0, 1.0, 0.0), aspect_ratio);

            // Create vertex buffer
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cube Vertex Buffer"),
                contents: bytemuck::cast_slice(CUBE_VERTICES),
                usage: BufferUsages::VERTEX,
            });

            // Create index buffer
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Cube Index Buffer"),
                contents: bytemuck::cast_slice(CUBE_INDICES),
                usage: BufferUsages::INDEX,
            });
            let num_indices = CUBE_INDICES.len() as u32;

            // Create ground vertex buffer
            let ground_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Ground Vertex Buffer"),
                contents: bytemuck::cast_slice(GROUND_VERTICES),
                usage: BufferUsages::VERTEX,
            });

            // Create ground index buffer
            let ground_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Ground Index Buffer"),
                contents: bytemuck::cast_slice(GROUND_INDICES),
                usage: BufferUsages::INDEX,
            });
            let ground_num_indices = GROUND_INDICES.len() as u32;

            // Create humanoid mesh buffers for local player (blue)
            let (humanoid_vertices, humanoid_indices) = build_humanoid_mesh(PLAYER_BLUE);
            let humanoid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Humanoid Vertex Buffer"),
                contents: bytemuck::cast_slice(&humanoid_vertices),
                usage: BufferUsages::VERTEX,
            });
            let humanoid_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Humanoid Index Buffer"),
                contents: bytemuck::cast_slice(&humanoid_indices),
                usage: BufferUsages::INDEX,
            });
            let humanoid_num_indices = humanoid_indices.len() as u32;

            // Create humanoid mesh buffers for other players (red)
            let (other_player_vertices, other_player_indices) = build_humanoid_mesh(OTHER_PLAYER_RED);
            let other_player_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Other Player Vertex Buffer"),
                contents: bytemuck::cast_slice(&other_player_vertices),
                usage: BufferUsages::VERTEX,
            });
            let other_player_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Other Player Index Buffer"),
                contents: bytemuck::cast_slice(&other_player_indices),
                usage: BufferUsages::INDEX,
            });
            let other_player_num_indices = other_player_indices.len() as u32;

            // Create static objects mesh buffers
            let (static_objects_vertices, static_objects_indices) = build_static_objects_mesh();
            let static_objects_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Static Objects Vertex Buffer"),
                contents: bytemuck::cast_slice(&static_objects_vertices),
                usage: BufferUsages::VERTEX,
            });
            let static_objects_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Static Objects Index Buffer"),
                contents: bytemuck::cast_slice(&static_objects_indices),
                usage: BufferUsages::INDEX,
            });
            let static_objects_num_indices = static_objects_indices.len() as u32;

            // Create voxel world mesh buffers (with COPY_DST for dynamic updates)
            let voxel_world = crate::voxel::VoxelWorld::generate_flat_world();
            let (voxel_vertices, voxel_indices) = voxel_world.build_mesh();
            let voxel_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Voxel World Vertex Buffer"),
                contents: bytemuck::cast_slice(&voxel_vertices),
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            });
            let voxel_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Voxel World Index Buffer"),
                contents: bytemuck::cast_slice(&voxel_indices),
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            });
            let voxel_num_indices = voxel_indices.len() as u32;

            // Load block texture atlas for voxel rendering
            let block_texture = crate::texture::load_block_texture_atlas(&device, &queue);
            let block_texture_view = block_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let block_sampler = crate::texture::create_pixel_art_sampler(&device);

            // Create uniform buffer for MVP matrix (64 bytes = 4x4 f32 matrix)
            let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("MVP Uniform Buffer"),
                contents: bytemuck::cast_slice(&[Mat4::IDENTITY]),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

            // Create light uniform buffer with default directional light
            let light_uniforms = LightUniforms::default();
            let light_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Light Uniform Buffer"),
                contents: bytemuck::cast_slice(&[light_uniforms]),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

            // Create bind group layout (MVP uniform + light uniform)
            let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("MVP Bind Group Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

            // Create bind group
            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("MVP Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: light_uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            // Create textured bind group layout (MVP uniform + texture + sampler + light uniform)
            let textured_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Textured Bind Group Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

            // Create textured bind group
            let textured_bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Textured Bind Group"),
                layout: &textured_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&block_texture_view),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&block_sampler),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: light_uniform_buffer.as_entire_binding(),
                    },
                ],
            });

            // Load shader
            let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));

            // Load textured shader
            let textured_shader = device.create_shader_module(include_wgsl!("textured_shader.wgsl"));

            // Create pipeline layout
            let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            // Create textured pipeline layout
            let textured_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Textured Render Pipeline Layout"),
                bind_group_layouts: &[&textured_bind_group_layout],
                push_constant_ranges: &[],
            });

            // Create render pipeline
            let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(surface_format.into())],
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            });

            // Create textured render pipeline
            let textured_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Textured Render Pipeline"),
                layout: Some(&textured_pipeline_layout),
                vertex: VertexState {
                    module: &textured_shader,
                    entry_point: "vs_main",
                    buffers: &[TexturedVertex::desc()],
                },
                fragment: Some(FragmentState {
                    module: &textured_shader,
                    entry_point: "fs_main",
                    targets: &[Some(surface_format.into())],
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            });

            Self {
                surface,
                device,
                queue,
                config,
                size,
                window,
                camera,
                voxel_world,
                vertex_buffer,
                index_buffer,
                num_indices,
                ground_vertex_buffer,
                ground_index_buffer,
                ground_num_indices,
                humanoid_vertex_buffer,
                humanoid_index_buffer,
                humanoid_num_indices,
                other_player_vertex_buffer,
                other_player_index_buffer,
                other_player_num_indices,
                static_objects_vertex_buffer,
                static_objects_index_buffer,
                static_objects_num_indices,
                voxel_vertex_buffer,
                voxel_index_buffer,
                voxel_num_indices,
                textured_bind_group,
                textured_pipeline,
                uniform_buffer,
                light_uniform_buffer,
                bind_group,
                pipeline,
            }
        }

        /// Handle window resize
        pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
            if new_size.width > 0 && new_size.height > 0 {
                self.size = new_size;
                self.config.width = new_size.width;
                self.config.height = new_size.height;
                self.surface.configure(&self.device, &self.config);
                self.camera.aspect_ratio = new_size.width as f32 / new_size.height as f32;
            }
        }

        /// Render a frame with the cube (legacy, renders humanoid at origin)
        pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
            self.render_with_players(Vec3::ZERO, 0.0, &[])
        }

        /// Render a frame with the local player and other players
        /// - local_position: position of the local player's humanoid
        /// - local_rotation_yaw: rotation of the local player in radians
        /// - other_players: slice of (position, rotation_yaw) for other players to render
        pub fn render_with_players(
            &mut self,
            local_position: Vec3,
            local_rotation_yaw: f32,
            other_players: &[(Vec3, f32)],
        ) -> Result<(), wgpu::SurfaceError> {
            let output = self.surface.get_current_texture()?;
            let view = output
                .texture
                .create_view(&TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(SKY_BLUE),
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.pipeline);

                // Draw ground plane first (identity model matrix)
                let ground_mvp = self.camera.view_projection_matrix();
                self.queue
                    .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[ground_mvp]));
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.ground_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.ground_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.ground_num_indices, 0, 0..1);

                // Draw static environment objects (identity model matrix, same MVP)
                render_pass.set_vertex_buffer(0, self.static_objects_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.static_objects_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.static_objects_num_indices, 0, 0..1);

                // Draw cube (for reference, identity model matrix)
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
            }

            // Draw voxel world terrain with textured pipeline
            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Voxel World Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.textured_pipeline);
                render_pass.set_bind_group(0, &self.textured_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.voxel_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.voxel_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.voxel_num_indices, 0, 0..1);
            }

            // We need to end the render pass to update the uniform buffer for each player
            // Then create a new render pass for each player with their model matrix
            // This is inefficient but works without changing the shader

            // Draw local player (blue humanoid) at their position
            {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Local Player Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load, // Don't clear, load existing content
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                let model = player_model_matrix(local_position, local_rotation_yaw);
                let mvp = self.camera.view_projection_matrix() * model;
                self.queue
                    .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[mvp]));

                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.humanoid_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.humanoid_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.humanoid_num_indices, 0, 0..1);
            }

            // Draw each other player (red humanoid) at their position
            for (position, rotation_yaw) in other_players {
                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("Other Player Render Pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                let model = player_model_matrix(*position, *rotation_yaw);
                let mvp = self.camera.view_projection_matrix() * model;
                self.queue
                    .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[mvp]));

                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.other_player_vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.other_player_index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.other_player_num_indices, 0, 0..1);
            }

            self.queue.submit(std::iter::once(encoder.finish()));
            output.present();

            Ok(())
        }

        /// Rebuild voxel mesh buffers after world changes (block placement/destruction).
        /// This recreates the vertex and index buffers from the current voxel_world state.
        pub fn rebuild_voxel_mesh(&mut self) {
            let (voxel_vertices, voxel_indices) = self.voxel_world.build_mesh();

            // Recreate buffers with new data
            self.voxel_vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Voxel World Vertex Buffer"),
                contents: bytemuck::cast_slice(&voxel_vertices),
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            });
            self.voxel_index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Voxel World Index Buffer"),
                contents: bytemuck::cast_slice(&voxel_indices),
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            });
            self.voxel_num_indices = voxel_indices.len() as u32;
        }
    }
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
            rotation_yaw: std::f32::consts::PI,
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
                rotation_yaw: std::f32::consts::PI,
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

    #[test]
    fn test_render_window_title() {
        use crate::render::WINDOW_TITLE;

        assert_eq!(WINDOW_TITLE, "3D Multiplayer Game");
    }

    #[test]
    fn test_render_sky_blue_color() {
        use crate::render::SKY_BLUE;

        // Sky blue is #87CEEB = RGB(135, 206, 235)
        const TOLERANCE: f64 = 0.001;
        assert!((SKY_BLUE.r - 135.0 / 255.0).abs() < TOLERANCE);
        assert!((SKY_BLUE.g - 206.0 / 255.0).abs() < TOLERANCE);
        assert!((SKY_BLUE.b - 235.0 / 255.0).abs() < TOLERANCE);
        assert!((SKY_BLUE.a - 1.0).abs() < TOLERANCE);
    }

    #[test]
    fn test_camera_new_default_values() {
        use crate::render::{Camera, DEFAULT_FAR, DEFAULT_FOV, DEFAULT_NEAR};

        let camera = Camera::new(Vec3::new(0.0, 2.0, 5.0), Vec3::ZERO, 16.0 / 9.0);

        assert_eq!(camera.position, Vec3::new(0.0, 2.0, 5.0));
        assert_eq!(camera.target, Vec3::ZERO);
        assert_eq!(camera.up, Vec3::Y);
        assert_eq!(camera.fov, DEFAULT_FOV);
        assert!((camera.aspect_ratio - 16.0 / 9.0).abs() < 0.001);
        assert_eq!(camera.near, DEFAULT_NEAR);
        assert_eq!(camera.far, DEFAULT_FAR);
    }

    #[test]
    fn test_camera_view_matrix_is_invertible() {
        use crate::render::Camera;

        let camera = Camera::new(Vec3::new(0.0, 2.0, 5.0), Vec3::ZERO, 1.0);
        let view = camera.view_matrix();

        // A valid view matrix should be invertible
        let det = view.determinant();
        assert!(det.abs() > 0.0001, "View matrix should be invertible");
    }

    #[test]
    fn test_camera_projection_matrix_is_valid() {
        use crate::render::Camera;

        let camera = Camera::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, 1.0);
        let proj = camera.projection_matrix();

        // A valid perspective projection matrix should be invertible
        let det = proj.determinant();
        assert!(det.abs() > 0.0001, "Projection matrix should be invertible");
    }

    #[test]
    fn test_camera_view_projection_combines_correctly() {
        use crate::render::Camera;
        use glam::Vec4;

        let camera = Camera::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, 1.0);
        let vp = camera.view_projection_matrix();
        let view = camera.view_matrix();
        let proj = camera.projection_matrix();

        // Test that view_projection_matrix = projection * view
        let expected = proj * view;
        let test_point = Vec4::new(1.0, 2.0, 3.0, 1.0);

        let result_vp = vp * test_point;
        let result_expected = expected * test_point;

        const TOLERANCE: f32 = 0.0001;
        assert!((result_vp.x - result_expected.x).abs() < TOLERANCE);
        assert!((result_vp.y - result_expected.y).abs() < TOLERANCE);
        assert!((result_vp.z - result_expected.z).abs() < TOLERANCE);
        assert!((result_vp.w - result_expected.w).abs() < TOLERANCE);
    }

    #[test]
    fn test_camera_transforms_origin_to_center() {
        use crate::render::Camera;
        use glam::Vec4;

        // Camera looking at origin from Z axis
        let camera = Camera::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, 1.0);
        let vp = camera.view_projection_matrix();

        // Origin should transform near center of screen (x=0, y=0 in NDC after perspective divide)
        let origin = Vec4::new(0.0, 0.0, 0.0, 1.0);
        let transformed = vp * origin;

        // After perspective divide, x and y should be near 0
        let ndc_x = transformed.x / transformed.w;
        let ndc_y = transformed.y / transformed.w;

        const TOLERANCE: f32 = 0.0001;
        assert!(ndc_x.abs() < TOLERANCE, "Origin x should be at center: {}", ndc_x);
        assert!(ndc_y.abs() < TOLERANCE, "Origin y should be at center: {}", ndc_y);
    }

    #[test]
    fn test_camera_constants() {
        use crate::render::{DEFAULT_FAR, DEFAULT_FOV, DEFAULT_NEAR};

        // FOV should be 60 degrees (PI/3 radians)
        const TOLERANCE: f32 = 0.0001;
        assert!((DEFAULT_FOV - std::f32::consts::PI / 3.0).abs() < TOLERANCE);
        assert_eq!(DEFAULT_NEAR, 0.1);
        assert_eq!(DEFAULT_FAR, 100.0);
    }

    #[test]
    fn test_cube_vertex_count() {
        use crate::render::CUBE_VERTICES;

        // 24 vertices for a cube (4 per face for proper normals)
        assert_eq!(CUBE_VERTICES.len(), 24);
    }

    #[test]
    fn test_cube_index_count() {
        use crate::render::CUBE_INDICES;

        // 36 indices for 12 triangles (6 faces * 2 triangles * 3 vertices)
        assert_eq!(CUBE_INDICES.len(), 36);
    }

    #[test]
    fn test_cube_indices_valid() {
        use crate::render::{CUBE_INDICES, CUBE_VERTICES};

        // All indices should be within vertex bounds
        for &index in CUBE_INDICES {
            assert!(
                (index as usize) < CUBE_VERTICES.len(),
                "Index {} out of bounds for {} vertices",
                index,
                CUBE_VERTICES.len()
            );
        }
    }

    #[test]
    fn test_vertex_struct_size() {
        use crate::render::Vertex;

        // Vertex should be 9 floats (3 position + 3 color + 3 normal) = 36 bytes
        assert_eq!(std::mem::size_of::<Vertex>(), 36);
    }

    #[test]
    fn test_ground_vertex_count() {
        use crate::render::GROUND_VERTICES;

        // 4 vertices for a quad
        assert_eq!(GROUND_VERTICES.len(), 4);
    }

    #[test]
    fn test_ground_index_count() {
        use crate::render::GROUND_INDICES;

        // 6 indices for 2 triangles
        assert_eq!(GROUND_INDICES.len(), 6);
    }

    #[test]
    fn test_ground_indices_valid() {
        use crate::render::{GROUND_INDICES, GROUND_VERTICES};

        // All indices should be within vertex bounds
        for &index in GROUND_INDICES {
            assert!(
                (index as usize) < GROUND_VERTICES.len(),
                "Index {} out of bounds for {} vertices",
                index,
                GROUND_VERTICES.len()
            );
        }
    }

    #[test]
    fn test_ground_vertices_at_y_zero() {
        use crate::render::GROUND_VERTICES;

        // All ground vertices should be at y=0
        for vertex in GROUND_VERTICES {
            assert_eq!(vertex.position[1], 0.0, "Ground vertex should be at y=0");
        }
    }

    #[test]
    fn test_ground_plane_size() {
        use crate::render::GROUND_VERTICES;

        // Ground plane should be 50x50, centered at origin (-25 to 25)
        let min_x = GROUND_VERTICES.iter().map(|v| v.position[0]).fold(f32::INFINITY, f32::min);
        let max_x = GROUND_VERTICES.iter().map(|v| v.position[0]).fold(f32::NEG_INFINITY, f32::max);
        let min_z = GROUND_VERTICES.iter().map(|v| v.position[2]).fold(f32::INFINITY, f32::min);
        let max_z = GROUND_VERTICES.iter().map(|v| v.position[2]).fold(f32::NEG_INFINITY, f32::max);

        assert_eq!(min_x, -25.0);
        assert_eq!(max_x, 25.0);
        assert_eq!(min_z, -25.0);
        assert_eq!(max_z, 25.0);
    }

    #[test]
    fn test_humanoid_vertex_count() {
        use crate::render::build_humanoid_mesh;

        let (vertices, _) = build_humanoid_mesh([0.0, 0.0, 1.0]);
        // 6 body parts * 24 vertices each (4 per face for proper normals) = 144 vertices
        assert_eq!(vertices.len(), 144);
    }

    #[test]
    fn test_humanoid_index_count() {
        use crate::render::build_humanoid_mesh;

        let (_, indices) = build_humanoid_mesh([0.0, 0.0, 1.0]);
        // 6 body parts * 36 indices each = 216 indices
        assert_eq!(indices.len(), 216);
    }

    #[test]
    fn test_humanoid_indices_valid() {
        use crate::render::build_humanoid_mesh;

        let (vertices, indices) = build_humanoid_mesh([0.0, 0.0, 1.0]);
        // All indices should be within vertex bounds
        for &index in &indices {
            assert!(
                (index as usize) < vertices.len(),
                "Index {} out of bounds for {} vertices",
                index,
                vertices.len()
            );
        }
    }

    #[test]
    fn test_humanoid_has_correct_color() {
        use crate::render::build_humanoid_mesh;

        let color = [0.267, 0.267, 1.0];
        let (vertices, _) = build_humanoid_mesh(color);

        // All vertices should have the specified color
        for vertex in &vertices {
            assert_eq!(vertex.color, color);
        }
    }

    #[test]
    fn test_humanoid_height() {
        use crate::render::build_humanoid_mesh;

        let (vertices, _) = build_humanoid_mesh([0.0, 0.0, 1.0]);

        // Find min and max Y coordinates
        let min_y = vertices.iter().map(|v| v.position[1]).fold(f32::INFINITY, f32::min);
        let max_y = vertices.iter().map(|v| v.position[1]).fold(f32::NEG_INFINITY, f32::max);

        // Humanoid should be on the ground (min_y = 0) and about 1.8 units tall (head top at 1.8)
        assert!(min_y.abs() < 0.001, "Humanoid bottom should be at y=0, got {}", min_y);
        assert!((max_y - 1.8).abs() < 0.001, "Humanoid top should be at y=1.8, got {}", max_y);
    }

    #[test]
    fn test_humanoid_centered_horizontally() {
        use crate::render::build_humanoid_mesh;

        let (vertices, _) = build_humanoid_mesh([0.0, 0.0, 1.0]);

        // Find min and max X coordinates
        let min_x = vertices.iter().map(|v| v.position[0]).fold(f32::INFINITY, f32::min);
        let max_x = vertices.iter().map(|v| v.position[0]).fold(f32::NEG_INFINITY, f32::max);

        // Humanoid should be roughly symmetric around x=0
        // With arms at ±0.4 and arm width 0.2, extent is about ±0.5
        assert!((min_x + max_x).abs() < 0.001, "Humanoid should be centered on X axis");
    }

    #[test]
    fn test_static_objects_count() {
        use crate::render::STATIC_OBJECTS;

        // Should have 8 static objects (5 cubes + 3 walls)
        assert_eq!(STATIC_OBJECTS.len(), 8);
    }

    #[test]
    fn test_static_objects_vertex_count() {
        use crate::render::build_static_objects_mesh;

        let (vertices, _) = build_static_objects_mesh();
        // 8 objects * 24 vertices each (4 per face for proper normals) = 192 vertices
        assert_eq!(vertices.len(), 192);
    }

    #[test]
    fn test_static_objects_index_count() {
        use crate::render::build_static_objects_mesh;

        let (_, indices) = build_static_objects_mesh();
        // 8 objects * 36 indices each = 288 indices
        assert_eq!(indices.len(), 288);
    }

    #[test]
    fn test_static_objects_indices_valid() {
        use crate::render::build_static_objects_mesh;

        let (vertices, indices) = build_static_objects_mesh();
        // All indices should be within vertex bounds
        for &index in &indices {
            assert!(
                (index as usize) < vertices.len(),
                "Index {} out of bounds for {} vertices",
                index,
                vertices.len()
            );
        }
    }

    #[test]
    fn test_static_objects_not_at_origin() {
        use crate::render::STATIC_OBJECTS;

        // No object should be at or very near the origin (where players spawn)
        for obj in STATIC_OBJECTS {
            let dist_from_origin = (obj.position.0.powi(2) + obj.position.2.powi(2)).sqrt();
            assert!(
                dist_from_origin > 3.0,
                "Static object at ({}, {}, {}) is too close to origin spawn point",
                obj.position.0, obj.position.1, obj.position.2
            );
        }
    }

    #[test]
    fn test_other_player_red_color() {
        use crate::render::OTHER_PLAYER_RED;

        // Other player color is #FF4444 = RGB(255, 68, 68) = (1.0, 0.267, 0.267)
        const TOLERANCE: f32 = 0.01;
        assert!((OTHER_PLAYER_RED[0] - 1.0).abs() < TOLERANCE);
        assert!((OTHER_PLAYER_RED[1] - 0.267).abs() < TOLERANCE);
        assert!((OTHER_PLAYER_RED[2] - 0.267).abs() < TOLERANCE);
    }

    #[test]
    fn test_player_model_matrix_identity_at_origin() {
        use crate::render::player_model_matrix;
        use glam::Mat4;

        // At origin with no rotation, model matrix should be identity
        let model = player_model_matrix(Vec3::ZERO, 0.0);
        let identity = Mat4::IDENTITY;

        for i in 0..16 {
            let row = i / 4;
            let col = i % 4;
            assert!(
                (model.col(col)[row] - identity.col(col)[row]).abs() < 0.001,
                "Model matrix should be identity at origin with yaw=0"
            );
        }
    }

    #[test]
    fn test_player_model_matrix_translation() {
        use crate::render::player_model_matrix;
        use glam::Vec4;

        // With translation but no rotation
        let position = Vec3::new(5.0, 0.0, -3.0);
        let model = player_model_matrix(position, 0.0);

        // Transform a point at origin - should move to position
        let origin = Vec4::new(0.0, 0.0, 0.0, 1.0);
        let transformed = model * origin;

        const TOLERANCE: f32 = 0.001;
        assert!((transformed.x - 5.0).abs() < TOLERANCE);
        assert!((transformed.y - 0.0).abs() < TOLERANCE);
        assert!((transformed.z - (-3.0)).abs() < TOLERANCE);
    }

    #[test]
    fn test_player_model_matrix_rotation() {
        use crate::render::player_model_matrix;
        use glam::Vec4;
        use std::f32::consts::PI;

        // With 90 degree rotation (positive yaw = clockwise when viewed from above)
        // point at (0, 0, 1) should move to (1, 0, 0)
        let model = player_model_matrix(Vec3::ZERO, PI / 2.0);
        let point = Vec4::new(0.0, 0.0, 1.0, 1.0);
        let transformed = model * point;

        const TOLERANCE: f32 = 0.001;
        assert!((transformed.x - 1.0).abs() < TOLERANCE, "X should be 1, got {}", transformed.x);
        assert!(transformed.y.abs() < TOLERANCE, "Y should be 0, got {}", transformed.y);
        assert!(transformed.z.abs() < TOLERANCE, "Z should be 0, got {}", transformed.z);
    }

    #[test]
    fn test_player_model_matrix_translation_and_rotation() {
        use crate::render::player_model_matrix;
        use glam::Vec4;
        use std::f32::consts::PI;

        // Translation (10, 0, 5) and 180 degree rotation
        // Point at origin relative to player should end up at the translation position
        let model = player_model_matrix(Vec3::new(10.0, 0.0, 5.0), PI);
        let origin = Vec4::new(0.0, 0.0, 0.0, 1.0);
        let transformed = model * origin;

        const TOLERANCE: f32 = 0.001;
        assert!((transformed.x - 10.0).abs() < TOLERANCE, "X should be 10, got {}", transformed.x);
        assert!(transformed.y.abs() < TOLERANCE, "Y should be 0, got {}", transformed.y);
        assert!((transformed.z - 5.0).abs() < TOLERANCE, "Z should be 5, got {}", transformed.z);
    }

    #[test]
    fn test_other_player_humanoid_has_red_color() {
        use crate::render::{build_humanoid_mesh, OTHER_PLAYER_RED};

        let (vertices, _) = build_humanoid_mesh(OTHER_PLAYER_RED);

        // All vertices should have the red color
        for vertex in &vertices {
            assert_eq!(vertex.color, OTHER_PLAYER_RED);
        }
    }

    #[test]
    fn test_headless_render_constants() {
        use crate::render::{HEADLESS_HEIGHT, HEADLESS_WIDTH};

        assert_eq!(HEADLESS_WIDTH, 800);
        assert_eq!(HEADLESS_HEIGHT, 600);
    }

    #[test]
    fn test_headless_render_state_initialization() {
        use crate::render::HeadlessRenderState;

        // Try to create a headless render state
        let result = pollster::block_on(HeadlessRenderState::new(800, 600));

        match result {
            Ok(state) => {
                // Verify dimensions
                assert_eq!(state.width, 800);
                assert_eq!(state.height, 600);
                // Verify camera aspect ratio
                let expected_aspect = 800.0 / 600.0;
                assert!((state.camera.aspect_ratio - expected_aspect).abs() < 0.001);
            }
            Err(e) => {
                // If no GPU is available, the test is inconclusive but should not fail
                eprintln!("Skipping test_headless_render_state_initialization: {}", e);
            }
        }
    }

    #[test]
    fn test_headless_render_state_render_returns_texture() {
        use crate::render::HeadlessRenderState;

        let result = pollster::block_on(HeadlessRenderState::new(100, 100));

        match result {
            Ok(mut state) => {
                // Render a frame and check we get a texture reference back
                let texture = state.render_with_players(Vec3::ZERO, 0.0, &[]);

                // Verify the texture has the expected dimensions
                let size = texture.size();
                assert_eq!(size.width, 100);
                assert_eq!(size.height, 100);
            }
            Err(e) => {
                eprintln!("Skipping test_headless_render_state_render_returns_texture: {}", e);
            }
        }
    }

    #[test]
    fn test_headless_render_state_with_other_players() {
        use crate::render::HeadlessRenderState;

        let result = pollster::block_on(HeadlessRenderState::new(100, 100));

        match result {
            Ok(mut state) => {
                // Render with some other players
                let other_players = vec![
                    (Vec3::new(5.0, 0.0, 0.0), 0.0),
                    (Vec3::new(-5.0, 0.0, 0.0), std::f32::consts::PI),
                ];
                let texture = state.render_with_players(Vec3::ZERO, 0.0, &other_players);

                // Should return a valid texture
                let size = texture.size();
                assert_eq!(size.width, 100);
                assert_eq!(size.height, 100);
            }
            Err(e) => {
                eprintln!("Skipping test_headless_render_state_with_other_players: {}", e);
            }
        }
    }

    // Texture loading tests
    #[test]
    fn test_load_single_png_texture() {
        use crate::texture::load_png;
        use std::path::Path;

        // Create wgpu device and queue for testing
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        let Some(adapter) = adapter else {
            eprintln!("Skipping test_load_single_png_texture: no GPU adapter available");
            return;
        };

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ))
        .expect("Failed to create device");

        // Load a real texture from the textures directory
        let texture_path = Path::new("textures/default-textures/textures/blocks/dirt.png");
        if texture_path.exists() {
            let result = load_png(&device, &queue, texture_path);
            assert!(result.is_ok(), "Failed to load texture: {:?}", result.err());

            let texture = result.unwrap();
            let size = texture.size();
            // Minecraft textures are typically 16x16
            assert!(size.width > 0, "Texture width should be positive");
            assert!(size.height > 0, "Texture height should be positive");
        } else {
            eprintln!("Skipping test_load_single_png_texture: texture file not found at {:?}", texture_path);
        }
    }

    #[test]
    fn test_load_png_missing_file() {
        use crate::texture::load_png;
        use std::path::Path;

        // Create wgpu device and queue for testing
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        let Some(adapter) = adapter else {
            eprintln!("Skipping test_load_png_missing_file: no GPU adapter available");
            return;
        };

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ))
        .expect("Failed to create device");

        // Try to load a non-existent file
        let result = load_png(&device, &queue, Path::new("nonexistent/texture.png"));
        assert!(result.is_err(), "Should fail for missing file");
    }

    #[test]
    fn test_load_block_textures_from_directory() {
        use crate::texture::load_block_textures;
        use std::path::Path;

        // Create wgpu device and queue for testing
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        let Some(adapter) = adapter else {
            eprintln!("Skipping test_load_block_textures_from_directory: no GPU adapter available");
            return;
        };

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ))
        .expect("Failed to create device");

        let textures_path = Path::new("textures/default-textures/textures/blocks");
        if textures_path.exists() {
            let result = load_block_textures(&device, &queue, textures_path);
            assert!(result.is_ok(), "Failed to load block textures: {:?}", result.err());

            let textures = result.unwrap();
            // Should have loaded multiple textures
            assert!(!textures.is_empty(), "Should have loaded at least one texture");

            // Check that some common textures are loaded
            if textures.contains_key("dirt") {
                let dirt_texture = textures.get("dirt").unwrap();
                let size = dirt_texture.size();
                assert!(size.width > 0 && size.height > 0);
            }
        } else {
            eprintln!("Skipping test_load_block_textures_from_directory: textures directory not found");
        }
    }

    #[test]
    fn test_texture_loader_creation() {
        use crate::texture::TextureLoader;
        use std::path::Path;

        // Create wgpu device and queue for testing
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        let Some(adapter) = adapter else {
            eprintln!("Skipping test_texture_loader_creation: no GPU adapter available");
            return;
        };

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ))
        .expect("Failed to create device");

        let textures_path = Path::new("textures/default-textures/textures/blocks");
        if textures_path.exists() {
            let result = TextureLoader::new(&device, &queue, textures_path);
            assert!(result.is_ok(), "Failed to create TextureLoader: {:?}", result.err());

            let loader = result.unwrap();
            // Should have loaded textures
            assert!(!loader.is_empty(), "TextureLoader should have loaded textures");
            assert!(loader.len() > 0, "TextureLoader should have at least one texture");

            // Check texture access
            for name in loader.texture_names() {
                assert!(loader.get(name).is_some(), "Should be able to get texture by name");
            }
        } else {
            eprintln!("Skipping test_texture_loader_creation: textures directory not found");
        }
    }

    #[test]
    fn test_pixel_art_sampler_creation() {
        use crate::texture::create_pixel_art_sampler;

        // Create wgpu device for testing
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        let Some(adapter) = adapter else {
            eprintln!("Skipping test_pixel_art_sampler_creation: no GPU adapter available");
            return;
        };

        let (device, _queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ))
        .expect("Failed to create device");

        // Creating the sampler should not panic
        let _sampler = create_pixel_art_sampler(&device);
    }

    // TexturedVertex and UV mapping tests
    #[test]
    fn test_textured_vertex_struct_size() {
        use crate::render::TexturedVertex;

        // TexturedVertex should be 11 floats (3 position + 2 uv + 3 color + 3 normal) = 44 bytes
        assert_eq!(std::mem::size_of::<TexturedVertex>(), 44);
    }

    #[test]
    fn test_textured_vertex_memory_layout() {
        use crate::render::TexturedVertex;

        // Verify the struct is properly aligned for GPU use
        let vertex = TexturedVertex {
            position: [1.0, 2.0, 3.0],
            uv: [0.5, 0.5],
            color: [1.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
        };

        // Verify fields are accessible
        assert_eq!(vertex.position, [1.0, 2.0, 3.0]);
        assert_eq!(vertex.uv, [0.5, 0.5]);
        assert_eq!(vertex.color, [1.0, 0.0, 0.0]);
        assert_eq!(vertex.normal, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn test_create_textured_cube_vertices_count() {
        use crate::render::create_textured_cube_vertices;

        let vertices = create_textured_cube_vertices(
            (1.0, 1.0, 1.0),
            (0.0, 0.0, 0.0),
            [1.0, 1.0, 1.0],
        );

        // 24 vertices: 4 vertices per face × 6 faces
        assert_eq!(vertices.len(), 24);
    }

    #[test]
    fn test_create_textured_cube_vertices_uv_range() {
        use crate::render::create_textured_cube_vertices;

        let vertices = create_textured_cube_vertices(
            (1.0, 1.0, 1.0),
            (0.0, 0.0, 0.0),
            [1.0, 1.0, 1.0],
        );

        // All UV coordinates should be in [0, 1] range
        for vertex in &vertices {
            assert!(
                vertex.uv[0] >= 0.0 && vertex.uv[0] <= 1.0,
                "UV u coordinate out of range: {}",
                vertex.uv[0]
            );
            assert!(
                vertex.uv[1] >= 0.0 && vertex.uv[1] <= 1.0,
                "UV v coordinate out of range: {}",
                vertex.uv[1]
            );
        }
    }

    #[test]
    fn test_create_textured_cube_vertices_has_all_uv_corners() {
        use crate::render::create_textured_cube_vertices;

        let vertices = create_textured_cube_vertices(
            (1.0, 1.0, 1.0),
            (0.0, 0.0, 0.0),
            [1.0, 1.0, 1.0],
        );

        // Each face (4 vertices) should have UV corners: (0,0), (1,0), (1,1), (0,1)
        // Check that all 6 faces have proper UV mapping
        for face_idx in 0..6 {
            let face_start = face_idx * 4;
            let face_uvs: Vec<[f32; 2]> = (0..4)
                .map(|i| vertices[face_start + i].uv)
                .collect();

            // Each face should contain all 4 UV corners
            let has_00 = face_uvs.iter().any(|uv| uv[0] == 0.0 && uv[1] == 0.0);
            let has_10 = face_uvs.iter().any(|uv| uv[0] == 1.0 && uv[1] == 0.0);
            let has_11 = face_uvs.iter().any(|uv| uv[0] == 1.0 && uv[1] == 1.0);
            let has_01 = face_uvs.iter().any(|uv| uv[0] == 0.0 && uv[1] == 1.0);

            assert!(
                has_00 && has_10 && has_11 && has_01,
                "Face {} does not have all UV corners: {:?}",
                face_idx,
                face_uvs
            );
        }
    }

    #[test]
    fn test_create_textured_cube_vertices_color() {
        use crate::render::create_textured_cube_vertices;

        let color = [0.5, 0.3, 0.1];
        let vertices = create_textured_cube_vertices(
            (1.0, 1.0, 1.0),
            (0.0, 0.0, 0.0),
            color,
        );

        // All vertices should have the specified color
        for vertex in &vertices {
            assert_eq!(vertex.color, color);
        }
    }

    #[test]
    fn test_create_textured_cube_indices_count() {
        use crate::render::create_textured_cube_indices;

        let indices = create_textured_cube_indices(0);

        // 36 indices: 6 faces × 2 triangles × 3 vertices
        assert_eq!(indices.len(), 36);
    }

    #[test]
    fn test_create_textured_cube_indices_valid() {
        use crate::render::{create_textured_cube_indices, create_textured_cube_vertices};

        let vertices = create_textured_cube_vertices(
            (1.0, 1.0, 1.0),
            (0.0, 0.0, 0.0),
            [1.0, 1.0, 1.0],
        );
        let indices = create_textured_cube_indices(0);

        // All indices should be within vertex bounds
        for &index in &indices {
            assert!(
                (index as usize) < vertices.len(),
                "Index {} out of bounds for {} vertices",
                index,
                vertices.len()
            );
        }
    }

    #[test]
    fn test_create_textured_cube_indices_with_offset() {
        use crate::render::create_textured_cube_indices;

        let start_offset = 24u16;
        let indices = create_textured_cube_indices(start_offset);

        // All indices should be offset by the start value
        for &index in &indices {
            assert!(
                index >= start_offset && index < start_offset + 24,
                "Index {} should be in range [{}, {})",
                index,
                start_offset,
                start_offset + 24
            );
        }
    }

    // Block type tests
    #[test]
    fn test_block_type_all_variants() {
        use crate::blocks::BlockType;

        let all = BlockType::all();
        assert_eq!(all.len(), 5);
        assert!(all.contains(&BlockType::Grass));
        assert!(all.contains(&BlockType::Dirt));
        assert!(all.contains(&BlockType::Stone));
        assert!(all.contains(&BlockType::Cobblestone));
        assert!(all.contains(&BlockType::Wood));
    }

    #[test]
    fn test_block_type_grass_textures() {
        use crate::blocks::BlockType;

        let textures = BlockType::Grass.textures();
        assert_eq!(textures.top, "grass_top");
        assert_eq!(textures.bottom, "dirt");
        assert_eq!(textures.north, "grass_side_carried");
        assert_eq!(textures.south, "grass_side_carried");
        assert_eq!(textures.east, "grass_side_carried");
        assert_eq!(textures.west, "grass_side_carried");
    }

    #[test]
    fn test_block_type_dirt_textures() {
        use crate::blocks::BlockType;

        let textures = BlockType::Dirt.textures();
        // Dirt should have uniform texture on all faces
        assert_eq!(textures.top, "dirt");
        assert_eq!(textures.bottom, "dirt");
        assert_eq!(textures.north, "dirt");
        assert_eq!(textures.south, "dirt");
        assert_eq!(textures.east, "dirt");
        assert_eq!(textures.west, "dirt");
    }

    #[test]
    fn test_block_type_stone_textures() {
        use crate::blocks::BlockType;

        let textures = BlockType::Stone.textures();
        // Stone should have uniform texture on all faces
        assert_eq!(textures.top, "stone");
        assert_eq!(textures.bottom, "stone");
        assert_eq!(textures.north, "stone");
        assert_eq!(textures.south, "stone");
        assert_eq!(textures.east, "stone");
        assert_eq!(textures.west, "stone");
    }

    #[test]
    fn test_block_type_cobblestone_textures() {
        use crate::blocks::BlockType;

        let textures = BlockType::Cobblestone.textures();
        // Cobblestone should have uniform texture on all faces
        assert_eq!(textures.top, "cobblestone");
        assert_eq!(textures.bottom, "cobblestone");
        assert_eq!(textures.north, "cobblestone");
        assert_eq!(textures.south, "cobblestone");
        assert_eq!(textures.east, "cobblestone");
        assert_eq!(textures.west, "cobblestone");
    }

    #[test]
    fn test_block_type_wood_textures() {
        use crate::blocks::BlockType;

        let textures = BlockType::Wood.textures();
        // Wood (oak planks) should have uniform texture on all faces
        assert_eq!(textures.top, "planks_oak");
        assert_eq!(textures.bottom, "planks_oak");
        assert_eq!(textures.north, "planks_oak");
        assert_eq!(textures.south, "planks_oak");
        assert_eq!(textures.east, "planks_oak");
        assert_eq!(textures.west, "planks_oak");
    }

    #[test]
    fn test_block_face_textures_uniform() {
        use crate::blocks::BlockFaceTextures;

        let textures = BlockFaceTextures::uniform("test_texture");
        assert_eq!(textures.top, "test_texture");
        assert_eq!(textures.bottom, "test_texture");
        assert_eq!(textures.north, "test_texture");
        assert_eq!(textures.south, "test_texture");
        assert_eq!(textures.east, "test_texture");
        assert_eq!(textures.west, "test_texture");
    }

    #[test]
    fn test_block_face_textures_top_bottom_sides() {
        use crate::blocks::BlockFaceTextures;

        let textures = BlockFaceTextures::top_bottom_sides("top_tex", "bottom_tex", "side_tex");
        assert_eq!(textures.top, "top_tex");
        assert_eq!(textures.bottom, "bottom_tex");
        assert_eq!(textures.north, "side_tex");
        assert_eq!(textures.south, "side_tex");
        assert_eq!(textures.east, "side_tex");
        assert_eq!(textures.west, "side_tex");
    }

    #[test]
    fn test_block_type_textures_match_existing_files() {
        use crate::blocks::BlockType;
        use std::path::Path;

        let textures_dir = Path::new("textures/default-textures/textures/blocks");
        if !textures_dir.exists() {
            eprintln!("Skipping test_block_type_textures_match_existing_files: textures directory not found");
            return;
        }

        // Collect all texture names from all block types
        let mut texture_names = std::collections::HashSet::new();
        for block_type in BlockType::all() {
            let textures = block_type.textures();
            texture_names.insert(textures.top);
            texture_names.insert(textures.bottom);
            texture_names.insert(textures.north);
            texture_names.insert(textures.south);
            texture_names.insert(textures.east);
            texture_names.insert(textures.west);
        }

        // Verify each texture file exists
        for name in texture_names {
            let texture_path = textures_dir.join(format!("{}.png", name));
            assert!(
                texture_path.exists(),
                "Texture file not found: {:?}",
                texture_path
            );
        }
    }

    #[test]
    fn test_block_type_equality() {
        use crate::blocks::BlockType;

        assert_eq!(BlockType::Grass, BlockType::Grass);
        assert_ne!(BlockType::Grass, BlockType::Dirt);
        assert_ne!(BlockType::Stone, BlockType::Cobblestone);
    }

    #[test]
    fn test_block_type_clone() {
        use crate::blocks::BlockType;

        let original = BlockType::Wood;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_block_type_copy() {
        use crate::blocks::BlockType;

        let original = BlockType::Stone;
        let copied: BlockType = original; // Copy, not move
        assert_eq!(original, copied); // original still accessible
    }

    #[test]
    fn test_block_type_debug() {
        use crate::blocks::BlockType;

        let debug_str = format!("{:?}", BlockType::Grass);
        assert_eq!(debug_str, "Grass");
    }

    #[test]
    fn test_block_type_hash() {
        use crate::blocks::BlockType;
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert(BlockType::Grass, "grass_block");
        map.insert(BlockType::Dirt, "dirt_block");

        assert_eq!(map.get(&BlockType::Grass), Some(&"grass_block"));
        assert_eq!(map.get(&BlockType::Dirt), Some(&"dirt_block"));
        assert_eq!(map.get(&BlockType::Stone), None);
    }

    // Voxel world tests
    #[test]
    fn test_voxel_world_new_empty() {
        use crate::voxel::VoxelWorld;

        let world = VoxelWorld::new();
        assert_eq!(world.block_count(), 0);
    }

    #[test]
    fn test_voxel_world_set_and_get_block() {
        use crate::blocks::BlockType;
        use crate::voxel::VoxelWorld;

        let mut world = VoxelWorld::new();
        world.set_block(0, 0, 0, BlockType::Grass);

        assert_eq!(world.get_block(0, 0, 0), Some(&BlockType::Grass));
        assert_eq!(world.get_block(1, 0, 0), None);
    }

    #[test]
    fn test_voxel_generate_flat_world_block_count() {
        use crate::voxel::{VoxelWorld, WORLD_SIZE_X, WORLD_SIZE_Z, WORLD_MIN_Y, WORLD_MAX_Y};

        let world = VoxelWorld::generate_flat_world();

        // World is 16x16 blocks (x: -8 to 7, z: -8 to 7)
        // Depth is 5 layers (y: -4 to 0)
        let expected_blocks = (WORLD_SIZE_X * WORLD_SIZE_Z * (WORLD_MAX_Y - WORLD_MIN_Y + 1)) as usize;
        assert_eq!(world.block_count(), expected_blocks);
    }

    #[test]
    fn test_voxel_generate_flat_world_grass_on_top() {
        use crate::blocks::BlockType;
        use crate::voxel::VoxelWorld;

        let world = VoxelWorld::generate_flat_world();

        // All blocks at y=0 should be grass
        for x in -8..8 {
            for z in -8..8 {
                assert_eq!(
                    world.get_block(x, 0, z),
                    Some(&BlockType::Grass),
                    "Block at ({}, 0, {}) should be Grass",
                    x, z
                );
            }
        }
    }

    #[test]
    fn test_voxel_generate_flat_world_dirt_below_grass() {
        use crate::blocks::BlockType;
        use crate::voxel::VoxelWorld;

        let world = VoxelWorld::generate_flat_world();

        // All blocks at y=-1 and y=-2 should be dirt
        for x in -8..8 {
            for z in -8..8 {
                assert_eq!(
                    world.get_block(x, -1, z),
                    Some(&BlockType::Dirt),
                    "Block at ({}, -1, {}) should be Dirt",
                    x, z
                );
                assert_eq!(
                    world.get_block(x, -2, z),
                    Some(&BlockType::Dirt),
                    "Block at ({}, -2, {}) should be Dirt",
                    x, z
                );
            }
        }
    }

    #[test]
    fn test_voxel_generate_flat_world_stone_at_bottom() {
        use crate::blocks::BlockType;
        use crate::voxel::VoxelWorld;

        let world = VoxelWorld::generate_flat_world();

        // All blocks at y=-3 and y=-4 should be stone
        for x in -8..8 {
            for z in -8..8 {
                assert_eq!(
                    world.get_block(x, -3, z),
                    Some(&BlockType::Stone),
                    "Block at ({}, -3, {}) should be Stone",
                    x, z
                );
                assert_eq!(
                    world.get_block(x, -4, z),
                    Some(&BlockType::Stone),
                    "Block at ({}, -4, {}) should be Stone",
                    x, z
                );
            }
        }
    }

    #[test]
    fn test_voxel_build_mesh_with_face_culling() {
        use crate::voxel::VoxelWorld;

        let world = VoxelWorld::generate_flat_world();
        let (vertices, indices) = world.build_mesh();

        // With face culling, we should have fewer faces than 6 per block
        // because interior faces are culled.
        // Each visible face has 4 vertices and 6 indices
        let max_vertices = world.block_count() * 24; // Without culling
        let max_indices = world.block_count() * 36;

        // Face culling should reduce vertex/index count significantly
        assert!(
            vertices.len() < max_vertices,
            "Face culling should reduce vertex count: {} >= {} (no reduction)",
            vertices.len(),
            max_vertices
        );
        assert!(
            indices.len() < max_indices,
            "Face culling should reduce index count: {} >= {} (no reduction)",
            indices.len(),
            max_indices
        );

        // Verify consistent vertex/index ratio: 4 vertices per 6 indices (i.e., 2:3)
        // Each face has 4 vertices and 6 indices
        assert_eq!(
            vertices.len() * 6,
            indices.len() * 4,
            "Vertex to index ratio should be 4:6 (2:3)"
        );
    }

    #[test]
    fn test_voxel_build_mesh_single_block_all_faces() {
        use crate::blocks::BlockType;
        use crate::voxel::VoxelWorld;

        // A single isolated block should have all 6 faces rendered
        let mut world = VoxelWorld::new();
        world.set_block(0, 0, 0, BlockType::Grass);

        let (vertices, indices) = world.build_mesh();

        // 6 faces * 4 vertices = 24 vertices
        assert_eq!(vertices.len(), 24, "Single block should have 24 vertices");
        // 6 faces * 6 indices = 36 indices
        assert_eq!(indices.len(), 36, "Single block should have 36 indices");
    }

    #[test]
    fn test_voxel_build_mesh_interior_block_no_faces() {
        use crate::blocks::BlockType;
        use crate::voxel::VoxelWorld;

        // Create a 3x3x3 cube - the center block should have no visible faces
        let mut world = VoxelWorld::new();
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    world.set_block(x, y, z, BlockType::Stone);
                }
            }
        }

        let (vertices, _) = world.build_mesh();

        // 27 blocks total, but the center block (0,0,0) has all neighbors
        // So it contributes 0 faces.
        // The 26 outer blocks each have some exposed faces.
        // Total faces for a 3x3x3 solid cube = 6 faces * 9 blocks per face = 54 faces
        // 54 faces * 4 vertices = 216 vertices
        assert_eq!(
            vertices.len(),
            216,
            "3x3x3 cube should have 216 vertices (54 faces * 4)"
        );
    }

    #[test]
    fn test_voxel_build_mesh_two_adjacent_blocks() {
        use crate::blocks::BlockType;
        use crate::voxel::VoxelWorld;

        // Two blocks adjacent along X axis
        let mut world = VoxelWorld::new();
        world.set_block(0, 0, 0, BlockType::Grass);
        world.set_block(1, 0, 0, BlockType::Grass);

        let (vertices, indices) = world.build_mesh();

        // Each block has 6 faces, but they share one face (culled on both sides)
        // So total visible faces = 6 + 6 - 2 = 10 faces
        // 10 faces * 4 vertices = 40 vertices
        assert_eq!(vertices.len(), 40, "Two adjacent blocks should have 40 vertices");
        // 10 faces * 6 indices = 60 indices
        assert_eq!(indices.len(), 60, "Two adjacent blocks should have 60 indices");
    }

    #[test]
    fn test_voxel_build_mesh_indices_valid() {
        use crate::voxel::VoxelWorld;

        let world = VoxelWorld::generate_flat_world();
        let (vertices, indices) = world.build_mesh();

        // All indices should be within vertex bounds
        for &index in &indices {
            assert!(
                (index as usize) < vertices.len(),
                "Index {} out of bounds for {} vertices",
                index,
                vertices.len()
            );
        }
    }

    #[test]
    fn test_voxel_world_default() {
        use crate::voxel::VoxelWorld;

        let world = VoxelWorld::default();
        assert_eq!(world.block_count(), 0);
    }

    #[test]
    fn test_voxel_world_constants() {
        use crate::voxel::{WORLD_SIZE_X, WORLD_SIZE_Z, WORLD_MIN_Y, WORLD_MAX_Y};

        assert_eq!(WORLD_SIZE_X, 16);
        assert_eq!(WORLD_SIZE_Z, 16);
        assert_eq!(WORLD_MIN_Y, -4);
        assert_eq!(WORLD_MAX_Y, 0);
    }
}
