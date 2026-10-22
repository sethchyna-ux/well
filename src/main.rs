// Updated Well terminal main entry point using winit 0.30 ApplicationHandler

use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use pollster::block_on;

struct RenderState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
}

#[derive(Default)]
struct WellApp {
    window: Option<Arc<Window>>,
    state: Option<RenderState>,
}

impl ApplicationHandler for WellApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let win_attrs = Window::default_attributes().with_title("Well Shell");
            let window = match event_loop.create_window(win_attrs) {
                Ok(window) => Arc::new(window),
                Err(error) => {
                    eprintln!("Failed to create Well window: {error}");
                    event_loop.exit();
                    return;
                }
            };
            match create_render_state(window.clone()) {
                Ok(state) => {
                    window.request_redraw();
                    self.window = Some(window);
                    self.state = Some(state);
                }
                Err(error) => {
                    eprintln!("Failed to initialize Well rendering: {error}");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(state) = &mut self.state {
                    state.size = physical_size;
                    state.config.width = physical_size.width.max(1);
                    state.config.height = physical_size.height.max(1);
                    state.surface.configure(&state.device, &state.config);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.state {
                    let output = match state.surface.get_current_texture() {
                        Ok(output) => output,
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            state.surface.configure(&state.device, &state.config);
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                            return;
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            eprintln!("Well rendering ran out of GPU memory");
                            event_loop.exit();
                            return;
                        }
                        Err(wgpu::SurfaceError::Timeout) => return,
                    };
                    let view = output
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder =
                        state
                            .device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("Render Encoder"),
                            });

                    {
                        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Clear Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.05,
                                        g: 0.05,
                                        b: 0.08,
                                        a: 1.0,
                                    }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                    }

                    state.queue.submit(std::iter::once(encoder.finish()));
                    output.present();
                }
            }
            _ => (),
        }
    }
}

fn create_render_state(window: Arc<Window>) -> Result<RenderState, String> {
    let size = window.inner_size();
    let instance = wgpu::Instance::default();
    let surface = instance
        .create_surface(window)
        .map_err(|error| format!("failed to create surface: {error}"))?;

    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .ok_or_else(|| "no compatible graphics adapter found".to_owned())?;

    let (device, queue) = block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("Well Render Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        },
        None,
    ))
    .map_err(|error| format!("failed to create device: {error}"))?;

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|format| format.is_srgb())
        .or_else(|| surface_caps.formats.first().copied())
        .ok_or_else(|| "graphics surface reports no supported formats".to_owned())?;
    let present_mode = surface_caps
        .present_modes
        .iter()
        .copied()
        .find(|mode| *mode == wgpu::PresentMode::Fifo)
        .or_else(|| surface_caps.present_modes.first().copied())
        .ok_or_else(|| "graphics surface reports no present modes".to_owned())?;
    let alpha_mode = surface_caps
        .alpha_modes
        .first()
        .copied()
        .ok_or_else(|| "graphics surface reports no alpha modes".to_owned())?;

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode,
        alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    Ok(RenderState {
        surface,
        device,
        queue,
        config,
        size,
    })
}

fn main() {
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("Failed to create Well event loop: {error}");
            return;
        }
    };
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = WellApp::default();
    if let Err(error) = event_loop.run_app(&mut app) {
        eprintln!("Well event loop failed: {error}");
    }
}
