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

    /// Vertex with position and color for the cube mesh
    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Vertex {
        pub position: [f32; 3],
        pub color: [f32; 3],
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
                ],
            }
        }
    }

    /// Vertex with position, UV coordinates, and color for textured rendering.
    /// Each face of a cube needs unique vertices for proper UV mapping (24 vertices per cube).
    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct TexturedVertex {
        pub position: [f32; 3],
        pub uv: [f32; 2],
        pub color: [f32; 3],
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
                ],
            }
        }
    }

    /// Cube vertices with positions and colors (8 vertices, each corner a different color)
    pub const CUBE_VERTICES: &[Vertex] = &[
        // Front face (z = 0.5)
        Vertex { position: [-0.5, -0.5,  0.5], color: [1.0, 0.0, 0.0] }, // 0: red
        Vertex { position: [ 0.5, -0.5,  0.5], color: [0.0, 1.0, 0.0] }, // 1: green
        Vertex { position: [ 0.5,  0.5,  0.5], color: [0.0, 0.0, 1.0] }, // 2: blue
        Vertex { position: [-0.5,  0.5,  0.5], color: [1.0, 1.0, 0.0] }, // 3: yellow
        // Back face (z = -0.5)
        Vertex { position: [-0.5, -0.5, -0.5], color: [1.0, 0.0, 1.0] }, // 4: magenta
        Vertex { position: [ 0.5, -0.5, -0.5], color: [0.0, 1.0, 1.0] }, // 5: cyan
        Vertex { position: [ 0.5,  0.5, -0.5], color: [1.0, 1.0, 1.0] }, // 6: white
        Vertex { position: [-0.5,  0.5, -0.5], color: [0.5, 0.5, 0.5] }, // 7: gray
    ];

    /// Cube indices for indexed drawing (12 triangles = 36 indices)
    pub const CUBE_INDICES: &[u16] = &[
        // Front face
        0, 1, 2, 2, 3, 0,
        // Right face
        1, 5, 6, 6, 2, 1,
        // Back face
        5, 4, 7, 7, 6, 5,
        // Left face
        4, 0, 3, 3, 7, 4,
        // Top face
        3, 2, 6, 6, 7, 3,
        // Bottom face
        4, 5, 1, 1, 0, 4,
    ];

    /// Dark gray color for the ground plane (#404040 = 0.251, 0.251, 0.251)
    const GROUND_GRAY: [f32; 3] = [0.251, 0.251, 0.251];

    /// Ground plane vertices (50x50 quad at y=0, centered at origin)
    pub const GROUND_VERTICES: &[Vertex] = &[
        Vertex { position: [-25.0, 0.0, -25.0], color: GROUND_GRAY }, // 0: back-left
        Vertex { position: [ 25.0, 0.0, -25.0], color: GROUND_GRAY }, // 1: back-right
        Vertex { position: [ 25.0, 0.0,  25.0], color: GROUND_GRAY }, // 2: front-right
        Vertex { position: [-25.0, 0.0,  25.0], color: GROUND_GRAY }, // 3: front-left
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

    /// Generate cube vertices at a given offset with a given size and color
    /// Size is (width, height, depth) and offset is the center position
    fn create_cube_vertices(
        size: (f32, f32, f32),
        offset: (f32, f32, f32),
        color: [f32; 3],
    ) -> [Vertex; 8] {
        let (w, h, d) = (size.0 / 2.0, size.1 / 2.0, size.2 / 2.0);
        let (ox, oy, oz) = offset;
        [
            // Front face (z = +d)
            Vertex { position: [ox - w, oy - h, oz + d], color },
            Vertex { position: [ox + w, oy - h, oz + d], color },
            Vertex { position: [ox + w, oy + h, oz + d], color },
            Vertex { position: [ox - w, oy + h, oz + d], color },
            // Back face (z = -d)
            Vertex { position: [ox - w, oy - h, oz - d], color },
            Vertex { position: [ox + w, oy - h, oz - d], color },
            Vertex { position: [ox + w, oy + h, oz - d], color },
            Vertex { position: [ox - w, oy + h, oz - d], color },
        ]
    }

    /// Generate indices for a cube part, offset by the starting vertex index
    const fn create_cube_indices(start: u16) -> [u16; 36] {
        [
            // Front face
            start, start + 1, start + 2, start + 2, start + 3, start,
            // Right face
            start + 1, start + 5, start + 6, start + 6, start + 2, start + 1,
            // Back face
            start + 5, start + 4, start + 7, start + 7, start + 6, start + 5,
            // Left face
            start + 4, start, start + 3, start + 3, start + 7, start + 4,
            // Top face
            start + 3, start + 2, start + 6, start + 6, start + 7, start + 3,
            // Bottom face
            start + 4, start + 5, start + 1, start + 1, start, start + 4,
        ]
    }

    /// Create 24 vertices for a textured cube (4 vertices per face, 6 faces).
    /// Unlike shared-corner cubes, textured cubes need unique vertices per face for proper UV mapping.
    /// UV coordinates map the full texture (0,0 to 1,1) to each face.
    pub fn create_textured_cube_vertices(
        size: (f32, f32, f32),
        offset: (f32, f32, f32),
        color: [f32; 3],
    ) -> [TexturedVertex; 24] {
        let (w, h, d) = (size.0 / 2.0, size.1 / 2.0, size.2 / 2.0);
        let (ox, oy, oz) = offset;

        [
            // Front face (+Z) - vertices 0-3
            TexturedVertex { position: [ox - w, oy - h, oz + d], uv: [0.0, 1.0], color },
            TexturedVertex { position: [ox + w, oy - h, oz + d], uv: [1.0, 1.0], color },
            TexturedVertex { position: [ox + w, oy + h, oz + d], uv: [1.0, 0.0], color },
            TexturedVertex { position: [ox - w, oy + h, oz + d], uv: [0.0, 0.0], color },

            // Back face (-Z) - vertices 4-7
            TexturedVertex { position: [ox + w, oy - h, oz - d], uv: [0.0, 1.0], color },
            TexturedVertex { position: [ox - w, oy - h, oz - d], uv: [1.0, 1.0], color },
            TexturedVertex { position: [ox - w, oy + h, oz - d], uv: [1.0, 0.0], color },
            TexturedVertex { position: [ox + w, oy + h, oz - d], uv: [0.0, 0.0], color },

            // Right face (+X) - vertices 8-11
            TexturedVertex { position: [ox + w, oy - h, oz + d], uv: [0.0, 1.0], color },
            TexturedVertex { position: [ox + w, oy - h, oz - d], uv: [1.0, 1.0], color },
            TexturedVertex { position: [ox + w, oy + h, oz - d], uv: [1.0, 0.0], color },
            TexturedVertex { position: [ox + w, oy + h, oz + d], uv: [0.0, 0.0], color },

            // Left face (-X) - vertices 12-15
            TexturedVertex { position: [ox - w, oy - h, oz - d], uv: [0.0, 1.0], color },
            TexturedVertex { position: [ox - w, oy - h, oz + d], uv: [1.0, 1.0], color },
            TexturedVertex { position: [ox - w, oy + h, oz + d], uv: [1.0, 0.0], color },
            TexturedVertex { position: [ox - w, oy + h, oz - d], uv: [0.0, 0.0], color },

            // Top face (+Y) - vertices 16-19
            TexturedVertex { position: [ox - w, oy + h, oz + d], uv: [0.0, 1.0], color },
            TexturedVertex { position: [ox + w, oy + h, oz + d], uv: [1.0, 1.0], color },
            TexturedVertex { position: [ox + w, oy + h, oz - d], uv: [1.0, 0.0], color },
            TexturedVertex { position: [ox - w, oy + h, oz - d], uv: [0.0, 0.0], color },

            // Bottom face (-Y) - vertices 20-23
            TexturedVertex { position: [ox - w, oy - h, oz - d], uv: [0.0, 1.0], color },
            TexturedVertex { position: [ox + w, oy - h, oz - d], uv: [1.0, 1.0], color },
            TexturedVertex { position: [ox + w, oy - h, oz + d], uv: [1.0, 0.0], color },
            TexturedVertex { position: [ox - w, oy - h, oz + d], uv: [0.0, 0.0], color },
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
        let mut vertices = Vec::with_capacity(STATIC_OBJECTS.len() * 8);
        let mut indices = Vec::with_capacity(STATIC_OBJECTS.len() * 36);

        for (i, obj) in STATIC_OBJECTS.iter().enumerate() {
            let cube_verts = create_cube_vertices(obj.size, obj.position, obj.color);
            vertices.extend_from_slice(&cube_verts);

            let cube_indices = create_cube_indices((i * 8) as u16);
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
        let mut vertices = Vec::with_capacity(48); // 6 parts * 8 vertices
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

        // Add indices for each part (6 parts, 8 vertices each)
        for i in 0..6 {
            let part_indices = create_cube_indices(i * 8);
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
        uniform_buffer: Buffer,
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
        static_objects_vertex_buffer: Buffer,
        static_objects_index_buffer: Buffer,
        static_objects_num_indices: u32,
        // Textured rendering fields
        textured_objects_vertex_buffer: Buffer,
        textured_objects_index_buffer: Buffer,
        textured_objects_num_indices: u32,
        textured_bind_group: BindGroup,
        textured_pipeline: RenderPipeline,
        uniform_buffer: Buffer,
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

            // Create uniform buffer for MVP matrix (64 bytes = 4x4 f32 matrix)
            let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("MVP Uniform Buffer"),
                contents: bytemuck::cast_slice(&[Mat4::IDENTITY]),
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

            // Create bind group layout
            let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("MVP Bind Group Layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

            // Create bind group
            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("MVP Bind Group"),
                layout: &bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
            });

            // Create textured bind group layout (MVP uniform + texture + sampler)
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
                static_objects_vertex_buffer,
                static_objects_index_buffer,
                static_objects_num_indices,
                textured_objects_vertex_buffer,
                textured_objects_index_buffer,
                textured_objects_num_indices,
                textured_bind_group,
                textured_pipeline,
                uniform_buffer,
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

            // Create uniform buffer for MVP matrix (64 bytes = 4x4 f32 matrix)
            let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("MVP Uniform Buffer"),
                contents: bytemuck::cast_slice(&[Mat4::IDENTITY]),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });

            // Create bind group layout
            let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("MVP Bind Group Layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

            // Create bind group
            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("MVP Bind Group"),
                layout: &bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
            });

            // Load shader
            let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));

            // Create pipeline layout
            let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
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

            Self {
                surface,
                device,
                queue,
                config,
                size,
                window,
                camera,
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
                uniform_buffer,
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

        // 8 vertices for a cube (one per corner)
        assert_eq!(CUBE_VERTICES.len(), 8);
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

        // Vertex should be 6 floats (3 position + 3 color) = 24 bytes
        assert_eq!(std::mem::size_of::<Vertex>(), 24);
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
        // 6 body parts * 8 vertices each = 48 vertices
        assert_eq!(vertices.len(), 48);
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
        // 8 objects * 8 vertices each = 64 vertices
        assert_eq!(vertices.len(), 64);
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

        // TexturedVertex should be 8 floats (3 position + 2 uv + 3 color) = 32 bytes
        assert_eq!(std::mem::size_of::<TexturedVertex>(), 32);
    }

    #[test]
    fn test_textured_vertex_memory_layout() {
        use crate::render::TexturedVertex;

        // Verify the struct is properly aligned for GPU use
        let vertex = TexturedVertex {
            position: [1.0, 2.0, 3.0],
            uv: [0.5, 0.5],
            color: [1.0, 0.0, 0.0],
        };

        // Verify fields are accessible
        assert_eq!(vertex.position, [1.0, 2.0, 3.0]);
        assert_eq!(vertex.uv, [0.5, 0.5]);
        assert_eq!(vertex.color, [1.0, 0.0, 0.0]);
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
}
