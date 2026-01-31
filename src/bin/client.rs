use std::sync::Arc;
use std::time::{Duration, Instant};

use game_engine::{network, render};
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowBuilder;

/// Target frame duration for 60fps
const FRAME_DURATION: Duration = Duration::from_micros(16_667);

fn main() {
    env_logger::init();
    log::info!("Starting client v{}...", game_engine::VERSION);
    log::info!("Window title: {}", render::WINDOW_TITLE);
    log::info!("Connecting to server on port {}", network::DEFAULT_PORT);

    let event_loop = EventLoop::new().expect("Failed to create event loop");

    let window = Arc::new(
        WindowBuilder::new()
            .with_title(render::WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(800, 600))
            .build(&event_loop)
            .expect("Failed to create window"),
    );

    let mut render_state = pollster::block_on(render::RenderState::new(window.clone()));
    let mut last_frame = Instant::now();

    log::info!("Window created and wgpu initialized");

    event_loop
        .run(move |event, elwt| {
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        log::info!("Close requested, exiting");
                        elwt.exit();
                    }
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key: PhysicalKey::Code(KeyCode::Escape),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    } => {
                        log::info!("Escape pressed, exiting");
                        elwt.exit();
                    }
                    WindowEvent::Resized(physical_size) => {
                        log::debug!("Resized to {:?}", physical_size);
                        render_state.resize(physical_size);
                    }
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let elapsed = now - last_frame;

                        if elapsed >= FRAME_DURATION {
                            last_frame = now;

                            match render_state.render() {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost) => {
                                    render_state.resize(render_state.size);
                                }
                                Err(wgpu::SurfaceError::OutOfMemory) => {
                                    log::error!("Out of memory, exiting");
                                    elwt.exit();
                                }
                                Err(e) => {
                                    log::warn!("Render error: {:?}", e);
                                }
                            }
                        }

                        // Schedule next frame
                        let next_frame = last_frame + FRAME_DURATION;
                        elwt.set_control_flow(ControlFlow::WaitUntil(next_frame));
                        window.request_redraw();
                    }
                    _ => {}
                },
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        })
        .expect("Event loop error");
}
