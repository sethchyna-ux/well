// Updated Well terminal main entry point using winit 0.30 ApplicationHandler

use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use wgpu::{Backends, InstanceDescriptor, RequestAdapterOptions, DeviceDescriptor, TextureUsages, SurfaceConfiguration, PresentMode, CompositeAlphaMode, Instance, Surface};
use pollster::block_on;

#[allow(dead_code)]
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
            let window = Arc::new(event_loop.create_window(win_attrs).unwrap());
            self.window = Some(window.clone());

            // --- Async WGPU Initialization ---
            let size = window.inner_size();
            let instance = wgpu::Instance::default();

            // Create surface using the Arc<Window> directly
            let surface = instance.create_surface(window.clone()).unwrap();

            let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }))
            .expect("Failed to find an appropriate adapter");

            let (device, queue) = block_on(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Well Render Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            ))
            .expect("Failed to create device");

            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .copied()
                .find(|f| f.is_srgb())
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: surface_caps.present_modes[0],
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };

            surface.configure(&device, &config);

            self.state = Some(RenderState {
                surface,
                device,
                queue,
                config,
                size,
            });
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
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
            }
            WindowEvent::RedrawRequested => {
                if let Some(state) = &self.state {
                    if let Ok(output) = state.surface.get_current_texture() {
                        let view = output
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder = state.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("Render Encoder"),
                            },
                        );

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
            }
            _ => (),
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = WellApp::default();
    event_loop.run_app(&mut app).unwrap();
}
