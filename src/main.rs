// Host windowing, event loop, and tab switcher (Atlas)
fn main() {
    println!("Well terminal starting...");

    // Initialize winit
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .build(&event_loop)
        .unwrap();

    // Initialize wgpu
    let instance = Instance::new(wgpu::Backends::all());
    let surface = instance.create_surface(&window);
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        ..Default::default()
    }).expect("Failed to find an appropriate adapter");

    let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
        features: wgpu::Features::empty(),
        limits: wgpu::Limits::default(),
        label: None,
    }, None).expect("Failed to create device");

    // Create a surface configuration
    let surface_format = surface.get_supported_formats(&adapter)[0];
    let surface_configuration = SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: window.inner_size().width,
        height: window.inner_size().height,
        present_mode: wgpu::PresentMode::Fifo,
    };

    surface.configure(&device, &surface_configuration);

    // Main event loop
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            winit::event::Event::WindowEvent { window_id, event } => {
                if window_id == window.id() {
                    match event {
                        winit::event::WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    });
}

// Required imports
use winit::{event::Event, event_loop::{ControlFlow, EventLoop}, window::WindowBuilder};
use wgpu::{Instance, Surface, SurfaceConfiguration};
