//! Test helper utilities for the game engine.
//!
//! This module provides utilities for testing, including screenshot capture
//! for visual regression testing.

use std::path::Path;
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
    let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
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
}
