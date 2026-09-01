//! Well Terminal (Phrear) - Unified Workspace Host
//!
//! Subsystems:
//! - ATLAS: Host Windowing & Event Loop (winit)
//! - ORPHEUS: GPU Text Shaping & Texture Array Renderer (wgpu)
//! - METIS: Shell Logic Core, PTY Session & Suggestions (well-shell)
//! - MNEME: Multi-line Rope Buffer & Tree-sitter AST Validator (well-editor)
//! - ASTRAEA: Sub-100µs Memory-Mapped State Vector & Prompt Compiler (well-prompt)
//! - THEIA: Immediate-Mode Visual Config Tab (well-config)
//! - HERMES: Lock-Free Seqlock IPC & JSON-RPC Agent Dispatch (well-ipc)

use std::sync::Arc;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent, Modifiers, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    keyboard::{Key, NamedKey},
    window::WindowBuilder,
};
use well_config::TheiasPrismPanel;
use well_editor::MnemeEditor;
use well_ipc::{HermesChannel, TheiaConfigPayload};
use well_prompt::{AstraeaStateVector, PromptCompiler};
use well_render::{CellInstance, OrpheusRenderer};
use well_shell::{MetisExecutor, PtySession};

#[derive(Debug, Clone, Copy)]
pub enum PtyEvent {
    NewOutput,
}

pub struct WellTerminalState {
    pub channel: Arc<HermesChannel<TheiaConfigPayload>>,
    pub shell: MetisExecutor,
    pub editor: MnemeEditor,
    pub prompt_compiler: PromptCompiler,
    pub config_panel: TheiasPrismPanel,
    pub pty_session: Arc<PtySession>,
}

impl WellTerminalState {
    pub fn new(pty_session: Arc<PtySession>) -> Self {
        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let prompt_state = Arc::new(AstraeaStateVector::new());
        let prompt_compiler = PromptCompiler::new(prompt_state);
        let config_panel = TheiasPrismPanel::new(Arc::clone(&channel));

        Self {
            channel,
            shell: MetisExecutor::new(),
            editor: MnemeEditor::default(),
            prompt_compiler,
            config_panel,
            pty_session,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoopBuilder::<PtyEvent>::with_user_event().build()?;

    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Well Terminal (Phrear) — Atlas Metal Host")
            .with_inner_size(LogicalSize::new(1280.0, 720.0))
            .with_min_inner_size(LogicalSize::new(640.0, 360.0))
            .build(&event_loop)?,
    );

    // 1. Initialize WebGPU / Metal Instance
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });

    // 2. Create OS Surface attached to window
    let surface = instance.create_surface(Arc::clone(&window))?;

    // 3. Request GPU adapter compatible with surface
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .ok_or("Failed to locate compatible WebGPU adapter")?;

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(surface_caps.formats[0]);

    let size = window.inner_size();
    let scale_factor = window.scale_factor() as f32;
    let mut cell_width = (10.0 * scale_factor).max(10.0);
    let mut cell_height = (20.0 * scale_factor).max(20.0);
    let mut grid_cols = (size.width as f32 / cell_width).floor() as u32;
    let mut grid_rows = (size.height as f32 / cell_height).floor() as u32;

    // 4. Initialize Orpheus Renderer
    let mut renderer = pollster::block_on(OrpheusRenderer::new(
        &instance,
        &surface,
        surface_format,
        grid_rows.max(1),
        grid_cols.max(1),
        cell_width,
        cell_height,
    ))?;

    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&renderer.device, &surface_config);
    renderer.update_projection_matrix(size.width as f32, size.height as f32);

    println!("Atlas Window created: {}x{} (scale factor: {})", size.width, size.height, scale_factor);
    println!("Surface format: {:?}", surface_format);
    println!("Grid matrix: {} cols x {} rows (cell: {}x{})", grid_cols, grid_rows, cell_width, cell_height);

    // 5. Initialize interactive PTY session
    let proxy = event_loop.create_proxy();
    let pty_session = Arc::new(PtySession::spawn(
        grid_rows.max(1) as u16,
        grid_cols.max(1) as u16,
        move || {
            let _ = proxy.send_event(PtyEvent::NewOutput);
        },
    )?);

    let _state = WellTerminalState::new(Arc::clone(&pty_session));
    let mut modifiers_state = Modifiers::default();

    window.request_redraw();

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        match event {
            Event::UserEvent(PtyEvent::NewOutput) => {
                window.request_redraw();
            }
            Event::WindowEvent { window_id, event: window_event } if window_id == window.id() => {
                match window_event {
                    WindowEvent::CloseRequested => {
                        println!("Window close requested, exiting cleanly.");
                        elwt.exit();
                    }
                    WindowEvent::Resized(new_size) => {
                        surface_config.width = new_size.width.max(1);
                        surface_config.height = new_size.height.max(1);
                        surface.configure(&renderer.device, &surface_config);
                        grid_cols = (new_size.width as f32 / cell_width).floor() as u32;
                        grid_rows = (new_size.height as f32 / cell_height).floor() as u32;
                        let _ = pty_session.resize(grid_rows.max(1) as u16, grid_cols.max(1) as u16);
                        renderer.update_projection_matrix(new_size.width as f32, new_size.height as f32);
                        window.request_redraw();
                    }
                    WindowEvent::ScaleFactorChanged { .. } => {
                        let new_size = window.inner_size();
                        surface_config.width = new_size.width.max(1);
                        surface_config.height = new_size.height.max(1);
                        surface.configure(&renderer.device, &surface_config);
                        let sf = window.scale_factor() as f32;
                        cell_width = (10.0 * sf).max(10.0);
                        cell_height = (20.0 * sf).max(20.0);
                        grid_cols = (new_size.width as f32 / cell_width).floor() as u32;
                        grid_rows = (new_size.height as f32 / cell_height).floor() as u32;
                        let _ = pty_session.resize(grid_rows.max(1) as u16, grid_cols.max(1) as u16);
                        renderer.update_projection_matrix(new_size.width as f32, new_size.height as f32);
                        window.request_redraw();
                    }
                    WindowEvent::ModifiersChanged(new_mods) => {
                        modifiers_state = new_mods;
                    }
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                state: ElementState::Pressed,
                                logical_key,
                                text,
                                ..
                            },
                        ..
                    } => {
                        let is_ctrl = modifiers_state.state().control_key();

                        let input_bytes: Option<Vec<u8>> = match logical_key {
                            Key::Named(NamedKey::Enter) => Some(vec![b'\r']),
                            Key::Named(NamedKey::Backspace) => Some(vec![0x7f]),
                            Key::Named(NamedKey::Tab) => Some(vec![b'\t']),
                            Key::Named(NamedKey::Escape) => Some(vec![0x1b]),
                            Key::Named(NamedKey::ArrowUp) => Some(b"\x1b[A".to_vec()),
                            Key::Named(NamedKey::ArrowDown) => Some(b"\x1b[B".to_vec()),
                            Key::Named(NamedKey::ArrowRight) => Some(b"\x1b[C".to_vec()),
                            Key::Named(NamedKey::ArrowLeft) => Some(b"\x1b[D".to_vec()),
                            Key::Named(NamedKey::Home) => Some(b"\x1b[H".to_vec()),
                            Key::Named(NamedKey::End) => Some(b"\x1b[F".to_vec()),
                            _ => {
                                if let Some(txt) = text {
                                    if is_ctrl {
                                        if let Some(ch) = txt.chars().next() {
                                            if ch.is_ascii_alphabetic() {
                                                let ctrl_code = (ch.to_ascii_lowercase() as u8) - b'a' + 1;
                                                Some(vec![ctrl_code])
                                            } else {
                                                Some(txt.as_bytes().to_vec())
                                            }
                                        } else {
                                            None
                                        }
                                    } else {
                                        Some(txt.as_bytes().to_vec())
                                    }
                                } else {
                                    None
                                }
                            }
                        };

                        if let Some(bytes) = input_bytes {
                            let _ = pty_session.write_all(&bytes);
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        let frame = match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(wgpu::SurfaceError::Lost) => {
                                surface.configure(&renderer.device, &surface_config);
                                return;
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                elwt.exit();
                                return;
                            }
                            Err(e) => {
                                eprintln!("Surface error: {:?}", e);
                                return;
                            }
                        };

                        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder = renderer.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("AtlasRenderEncoder"),
                            },
                        );

                        // Assemble text cells from live vt100 virtual screen
                        let mut cells: Vec<CellInstance> = Vec::new();
                        if let Ok(parser) = pty_session.parser.lock() {
                            renderer.build_cells_from_vt100(
                                parser.screen(),
                                &mut cells,
                                grid_rows,
                                grid_cols,
                            );
                        }

                        // Draw all cells in a single instanced call
                        renderer.draw_frame(&view, &mut encoder, &cells);

                        renderer.queue.submit(std::iter::once(encoder.finish()));
                        frame.present();
                    }
                    _ => ()
                }
            }
            _ => ()
        }
    })?;

    Ok(())
}
