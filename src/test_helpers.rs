//! Test helper utilities for the game engine.
//!
//! This module provides utilities for testing, including screenshot capture
//! for visual regression testing and video recording for gameplay capture.

use std::path::{Path, PathBuf};
use wgpu::{BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Device, Queue, Texture};

/// Captures a wgpu texture and saves it as a PNG file.
///
/// This function is useful for visual regression testing and debugging rendering issues.
///
/// # Arguments
/// * `device` - The wgpu device used for creating staging buffers
/// * `queue` - The wgpu queue for submitting commands
/// * `texture` - The texture to capture (must have COPY_SRC usage)
/// * `width` - Width of the texture in pixels
/// * `height` - Height of the texture in pixels
/// * `output_path` - Path where the PNG file will be saved
///
/// # Returns
/// * `Ok(())` if the screenshot was saved successfully
/// * `Err(String)` with a description of what went wrong
///
/// # Example
/// ```ignore
/// use game_engine::test_helpers::capture_screenshot;
/// use std::path::Path;
///
/// // After rendering to a texture...
/// capture_screenshot(
///     &device,
///     &queue,
///     &render_texture,
///     800,
///     600,
///     Path::new("tests/screenshots/test_render.png"),
/// ).expect("Failed to capture screenshot");
/// ```
pub fn capture_screenshot(
    device: &Device,
    queue: &Queue,
    texture: &Texture,
    width: u32,
    height: u32,
    output_path: &Path,
) -> Result<(), String> {
    // Calculate buffer size with proper alignment
    // wgpu requires rows to be aligned to 256 bytes (COPY_BYTES_PER_ROW_ALIGNMENT)
    let bytes_per_pixel = 4u32; // RGBA8
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
    let buffer_size = (padded_bytes_per_row * height) as u64;

    // Create staging buffer for reading texture data
    let staging_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Screenshot Staging Buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Create command encoder to copy texture to buffer
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Screenshot Copy Encoder"),
    });

    // Copy texture to buffer
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));

    // Map the buffer and read the data
    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });

    // Wait for the GPU to finish and mapping to complete
    device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .map_err(|e| format!("Failed to receive mapping result: {}", e))?
        .map_err(|e| format!("Failed to map buffer: {:?}", e))?;

    // Read the data
    let data = buffer_slice.get_mapped_range();

    // Remove row padding and convert to image
    let mut image_data = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
    for row in 0..height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + (width * bytes_per_pixel) as usize;
        image_data.extend_from_slice(&data[start..end]);
    }

    // Drop the mapped range before unmapping
    drop(data);
    staging_buffer.unmap();

    // Save as PNG using the image crate
    save_png(output_path, &image_data, width, height)
}

/// Saves raw RGBA pixel data as a PNG file.
fn save_png(path: &Path, data: &[u8], width: u32, height: u32) -> Result<(), String> {
    use image::{ImageBuffer, Rgba};

    let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(width, height, data.to_vec())
        .ok_or_else(|| "Failed to create image buffer from raw data".to_string())?;

    img.save(path)
        .map_err(|e| format!("Failed to save PNG to {:?}: {}", path, e))?;

    Ok(())
}

/// Captures a wgpu texture and returns the raw RGBA pixel data.
///
/// This is the internal function used by both capture_screenshot and VideoRecorder.
#[cfg(feature = "test-helpers")]
fn capture_texture_data(
    device: &Device,
    queue: &Queue,
    texture: &Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let bytes_per_pixel = 4u32; // RGBA8
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
    let buffer_size = (padded_bytes_per_row * height) as u64;

    let staging_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Frame Capture Staging Buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Frame Capture Copy Encoder"),
    });

    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &staging_buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));

    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });

    device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .map_err(|e| format!("Failed to receive mapping result: {}", e))?
        .map_err(|e| format!("Failed to map buffer: {:?}", e))?;

    let data = buffer_slice.get_mapped_range();

    let mut image_data = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
    for row in 0..height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + (width * bytes_per_pixel) as usize;
        image_data.extend_from_slice(&data[start..end]);
    }

    drop(data);
    staging_buffer.unmap();

    Ok(image_data)
}

/// Video recorder for capturing gameplay frames and encoding to MP4.
///
/// This struct accumulates frames during a test and encodes them to a video file
/// when `finish()` is called.
///
/// # Example
/// ```ignore
/// use game_engine::test_helpers::VideoRecorder;
/// use std::path::Path;
///
/// let mut recorder = VideoRecorder::new(800, 600, 30, Path::new("tests/videos/test.mp4"));
///
/// // During gameplay loop...
/// recorder.capture_frame(&device, &queue, &render_texture)?;
///
/// // When done...
/// recorder.finish()?;
/// ```
#[cfg(feature = "test-helpers")]
pub struct VideoRecorder {
    width: u32,
    height: u32,
    framerate: u32,
    frames: Vec<Vec<u8>>,
    output_path: PathBuf,
}

#[cfg(feature = "test-helpers")]
impl VideoRecorder {
    /// Creates a new video recorder.
    ///
    /// # Arguments
    /// * `width` - Width of the video in pixels
    /// * `height` - Height of the video in pixels
    /// * `framerate` - Target framerate (frames per second)
    /// * `output_path` - Path where the MP4 file will be saved
    pub fn new(width: u32, height: u32, framerate: u32, output_path: &Path) -> Self {
        Self {
            width,
            height,
            framerate,
            frames: Vec::new(),
            output_path: output_path.to_path_buf(),
        }
    }

    /// Captures a frame from a wgpu texture and adds it to the internal buffer.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `queue` - The wgpu queue
    /// * `texture` - The texture to capture (must have COPY_SRC usage)
    pub fn capture_frame(
        &mut self,
        device: &Device,
        queue: &Queue,
        texture: &Texture,
    ) -> Result<(), String> {
        let frame_data = capture_texture_data(device, queue, texture, self.width, self.height)?;
        self.frames.push(frame_data);
        Ok(())
    }

    /// Returns the number of captured frames.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Encodes all captured frames to an MP4 file and saves it.
    ///
    /// This method consumes the recorder and writes the video file.
    pub fn finish(self) -> Result<(), String> {
        if self.frames.is_empty() {
            return Err("No frames captured".to_string());
        }

        // Ensure output directory exists
        if let Some(parent) = self.output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
        }

        self.encode_mp4()
    }

    /// Encodes frames to MP4 using openh264 and mp4 crates.
    fn encode_mp4(self) -> Result<(), String> {
        use mp4::{AvcConfig, FourCC, MediaConfig, Mp4Config, Mp4Writer, TrackConfig};
        use openh264::encoder::Encoder;
        use openh264::formats::{RgbaSliceU8, YUVBuffer};

        // Initialize H.264 encoder (uses default settings, dimensions from YUVSource)
        let mut encoder =
            Encoder::new().map_err(|e| format!("Failed to create H.264 encoder: {:?}", e))?;

        // Encode all frames to NAL units
        let mut nal_units: Vec<Vec<u8>> = Vec::new();
        let mut sps: Option<Vec<u8>> = None;
        let mut pps: Option<Vec<u8>> = None;

        for (frame_idx, frame_rgba) in self.frames.iter().enumerate() {
            // Force keyframe for first frame to ensure we get SPS/PPS
            if frame_idx == 0 {
                encoder.force_intra_frame();
            }

            // Convert RGBA to YUV using openh264's built-in conversion
            let rgba_source =
                RgbaSliceU8::new(frame_rgba, (self.width as usize, self.height as usize));
            let yuv = YUVBuffer::from_rgb_source(rgba_source);

            // Encode frame
            let bitstream = encoder
                .encode(&yuv)
                .map_err(|e| format!("Failed to encode frame: {:?}", e))?;

            // Get full bitstream in Annex B format and extract NAL units
            let stream_data = bitstream.to_vec();
            let extracted_nals = extract_nal_units(&stream_data);

            for nal_data in extracted_nals {
                if !nal_data.is_empty() {
                    // Check NAL unit type (first byte)
                    let nal_type = nal_data[0] & 0x1F;
                    match nal_type {
                        7 => sps = Some(nal_data.clone()), // SPS
                        8 => pps = Some(nal_data.clone()), // PPS
                        _ => {}
                    }
                    nal_units.push(nal_data);
                }
            }
        }

        // Get SPS and PPS from encoded data
        let sps_data = sps.ok_or_else(|| "No SPS found in encoded data".to_string())?;
        let pps_data = pps.ok_or_else(|| "No PPS found in encoded data".to_string())?;

        // Write MP4 file
        let file = std::fs::File::create(&self.output_path)
            .map_err(|e| format!("Failed to create output file: {}", e))?;

        let mp4_config = Mp4Config {
            major_brand: FourCC::from(*b"isom"),
            minor_version: 512,
            compatible_brands: vec![
                FourCC::from(*b"isom"),
                FourCC::from(*b"iso2"),
                FourCC::from(*b"avc1"),
                FourCC::from(*b"mp41"),
            ],
            timescale: 1000,
        };

        let mut mp4_writer = Mp4Writer::write_start(file, &mp4_config)
            .map_err(|e| format!("Failed to start MP4 writer: {}", e))?;

        // Configure video track
        let track_config = TrackConfig {
            track_type: mp4::TrackType::Video,
            timescale: self.framerate * 1000,
            language: String::from("und"),
            media_conf: MediaConfig::AvcConfig(AvcConfig {
                width: self.width as u16,
                height: self.height as u16,
                seq_param_set: sps_data,
                pic_param_set: pps_data,
            }),
        };

        mp4_writer
            .add_track(&track_config)
            .map_err(|e| format!("Failed to add track: {}", e))?;

        // Write samples (skip SPS/PPS NAL units, they're in the track config)
        let mut sample_count = 0u64;
        let frame_duration = 1000u32; // Duration per frame in timescale units (1000 = 1 second at framerate * 1000 timescale)

        for nal_data in &nal_units {
            let nal_type = nal_data[0] & 0x1F;
            // Skip SPS (7) and PPS (8), only write VCL NAL units
            if nal_type != 7 && nal_type != 8 {
                // Prepend NAL unit length (4 bytes, big endian) for AVC format
                let mut sample_data = Vec::with_capacity(4 + nal_data.len());
                sample_data.extend_from_slice(&(nal_data.len() as u32).to_be_bytes());
                sample_data.extend_from_slice(nal_data);

                let is_keyframe = nal_type == 5; // IDR frame

                mp4_writer
                    .write_sample(
                        1, // track_id
                        &mp4::Mp4Sample {
                            start_time: sample_count * frame_duration as u64,
                            duration: frame_duration,
                            rendering_offset: 0,
                            is_sync: is_keyframe,
                            bytes: mp4::Bytes::from(sample_data),
                        },
                    )
                    .map_err(|e| format!("Failed to write sample: {}", e))?;

                sample_count += 1u64;
            }
        }

        mp4_writer
            .write_end()
            .map_err(|e| format!("Failed to finalize MP4: {}", e))?;

        Ok(())
    }
}

/// Extracts NAL units from Annex B formatted bitstream.
/// NAL units are separated by start codes (0x00 0x00 0x00 0x01 or 0x00 0x00 0x01).
#[cfg(feature = "test-helpers")]
fn extract_nal_units(data: &[u8]) -> Vec<Vec<u8>> {
    let mut nals = Vec::new();
    let mut i = 0;

    while i < data.len() {
        // Find start code (0x00 0x00 0x01 or 0x00 0x00 0x00 0x01)
        let start = if i + 3 < data.len()
            && data[i] == 0
            && data[i + 1] == 0
            && data[i + 2] == 0
            && data[i + 3] == 1
        {
            i += 4;
            i
        } else if i + 2 < data.len() && data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            i += 3;
            i
        } else {
            i += 1;
            continue;
        };

        // Find next start code or end of data
        let mut end = start;
        while end < data.len() {
            if end + 3 < data.len()
                && data[end] == 0
                && data[end + 1] == 0
                && data[end + 2] == 0
                && data[end + 3] == 1
            {
                break;
            } else if end + 2 < data.len()
                && data[end] == 0
                && data[end + 1] == 0
                && data[end + 2] == 1
            {
                break;
            }
            end += 1;
        }

        if start < end && end <= data.len() {
            nals.push(data[start..end].to_vec());
        }
        i = end;
    }

    nals
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that we can create a simple test texture and capture it.
    /// This test requires a GPU, so it will be skipped in environments without one.
    #[test]
    fn test_capture_screenshot_creates_file() {
        // Create wgpu instance and adapter
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        let adapter = match adapter {
            Some(a) => a,
            None => {
                eprintln!("Skipping test_capture_screenshot_creates_file: no GPU adapter available");
                return;
            }
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

        // Create a small test texture with solid red color
        let width = 64u32;
        let height = 64u32;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Fill texture with red pixels
        let red_data: Vec<u8> = (0..(width * height))
            .flat_map(|_| [255u8, 0, 0, 255]) // RGBA red
            .collect();

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &red_data,
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

        // Create output path in the tests/screenshots directory
        let output_path = Path::new("tests/screenshots/test_red_square.png");

        // Capture screenshot
        let result = capture_screenshot(&device, &queue, &texture, width, height, output_path);
        assert!(result.is_ok(), "capture_screenshot failed: {:?}", result);

        // Verify file exists
        assert!(output_path.exists(), "Screenshot file was not created");

        // Clean up - remove test file
        std::fs::remove_file(output_path).ok();
    }

    /// Test that VideoRecorder can capture frames and create an MP4 file.
    /// This test requires a GPU, so it will be skipped in environments without one.
    #[test]
    #[cfg(feature = "test-helpers")]
    fn test_video_recorder_creates_mp4() {
        // Create wgpu instance and adapter
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        let adapter = match adapter {
            Some(a) => a,
            None => {
                eprintln!("Skipping test_video_recorder_creates_mp4: no GPU adapter available");
                return;
            }
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

        // Create a small test texture (64x64 for faster encoding)
        let width = 64u32;
        let height = 64u32;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Video Test Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let output_path = Path::new("tests/videos/test_recording.mp4");
        let mut recorder = VideoRecorder::new(width, height, 30, output_path);

        // Capture a few frames with different colors to create a simple animation
        let colors: [(u8, u8, u8); 5] = [
            (255, 0, 0),   // Red
            (0, 255, 0),   // Green
            (0, 0, 255),   // Blue
            (255, 255, 0), // Yellow
            (255, 0, 255), // Magenta
        ];

        for (r, g, b) in colors.iter() {
            // Fill texture with color
            let color_data: Vec<u8> = (0..(width * height))
                .flat_map(|_| [*r, *g, *b, 255u8])
                .collect();

            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &color_data,
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

            // Capture frame
            let result = recorder.capture_frame(&device, &queue, &texture);
            assert!(result.is_ok(), "capture_frame failed: {:?}", result);
        }

        // Verify we captured all frames
        assert_eq!(recorder.frame_count(), 5, "Expected 5 frames to be captured");

        // Finish recording and save the video
        let result = recorder.finish();
        assert!(result.is_ok(), "finish() failed: {:?}", result);

        // Verify file exists and has content
        assert!(output_path.exists(), "MP4 file was not created");
        let metadata = std::fs::metadata(output_path).expect("Failed to get file metadata");
        assert!(metadata.len() > 0, "MP4 file is empty");

        // Clean up - remove test file
        std::fs::remove_file(output_path).ok();
    }
}
