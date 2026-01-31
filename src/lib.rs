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
        uniform_buffer: Buffer,
        bind_group: BindGroup,
        pipeline: RenderPipeline,
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

            // Create camera positioned at (0, 2, 5) looking at origin
            let aspect_ratio = size.width as f32 / size.height as f32;
            let camera = Camera::new(Vec3::new(0.0, 2.0, 5.0), Vec3::ZERO, aspect_ratio);

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

        /// Render a frame with the cube
        pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
            // Calculate MVP matrix (model is identity since cube is at origin)
            let model = Mat4::IDENTITY;
            let mvp = self.camera.view_projection_matrix() * model;

            // Write MVP matrix to uniform buffer
            self.queue
                .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[mvp]));

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
                render_pass.set_bind_group(0, &self.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
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
}
