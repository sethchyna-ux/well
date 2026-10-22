use std::sync::Arc;
use well_shell::PtySession;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

use pollster::block_on;

struct RenderState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    egui_context: egui::Context,
    egui_renderer: egui_wgpu::Renderer,
}

#[derive(Default)]
struct WellApp {
    window: Option<Arc<Window>>,
    state: Option<RenderState>,
    pty: Option<PtySession>,
    modifiers: ModifiersState,
    ime_preedit_active: bool,
}

impl WellApp {
    fn send_key(&self, event: &winit::event::KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        let Some(pty) = &self.pty else {
            return;
        };

        if let Some(bytes) = encode_function_key(event.physical_key, self.modifiers) {
            if let Err(error) = pty.write_all(&bytes) {
                eprintln!("Failed to write to terminal: {error}");
            }
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }

        let bytes = match event.physical_key {
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => Some(vec![b'\r']),
            PhysicalKey::Code(KeyCode::Tab) if self.modifiers.shift_key() => {
                Some(b"\x1b[Z".to_vec())
            }
            PhysicalKey::Code(KeyCode::Tab) => Some(vec![b'\t']),
            PhysicalKey::Code(KeyCode::Backspace) => Some(vec![0x7f]),
            PhysicalKey::Code(KeyCode::Escape) => Some(vec![0x1b]),
            PhysicalKey::Code(KeyCode::ArrowUp) => Some(encode_csi_key(b'A', self.modifiers)),
            PhysicalKey::Code(KeyCode::ArrowDown) => Some(encode_csi_key(b'B', self.modifiers)),
            PhysicalKey::Code(KeyCode::ArrowRight) => Some(encode_csi_key(b'C', self.modifiers)),
            PhysicalKey::Code(KeyCode::ArrowLeft) => Some(encode_csi_key(b'D', self.modifiers)),
            PhysicalKey::Code(KeyCode::Home) => Some(encode_csi_key(b'H', self.modifiers)),
            PhysicalKey::Code(KeyCode::End) => Some(encode_csi_key(b'F', self.modifiers)),
            PhysicalKey::Code(KeyCode::Delete) => Some(b"\x1b[3~".to_vec()),
            PhysicalKey::Code(KeyCode::PageUp) => Some(b"\x1b[5~".to_vec()),
            PhysicalKey::Code(KeyCode::PageDown) => Some(b"\x1b[6~".to_vec()),
            PhysicalKey::Code(KeyCode::Insert) => Some(b"\x1b[2~".to_vec()),
            _ if self.modifiers.control_key() => physical_ascii_key(event.physical_key)
                .map(control_byte)
                .map(|byte| {
                    let mut bytes = Vec::with_capacity(2);
                    if self.modifiers.alt_key() {
                        bytes.push(0x1b);
                    }
                    bytes.push(byte);
                    bytes
                }),
            _ if self.ime_preedit_active => None,
            _ => event.text.as_ref().and_then(|text| {
                let text = text.as_str();
                if text.chars().any(char::is_control) {
                    return None;
                }
                let mut bytes = Vec::with_capacity(text.len() + 1);
                if self.modifiers.alt_key() {
                    bytes.push(0x1b);
                }
                bytes.extend_from_slice(text.as_bytes());
                (!bytes.is_empty()).then_some(bytes)
            }),
        };

        if let Some(bytes) = bytes {
            if let Err(error) = pty.write_all(&bytes) {
                eprintln!("Failed to write to terminal: {error}");
            }
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}

impl ApplicationHandler for WellApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = match event_loop
            .create_window(Window::default_attributes().with_title("Well Terminal"))
        {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("Failed to create Well window: {error}");
                event_loop.exit();
                return;
            }
        };

        let state = match create_render_state(window.clone()) {
            Ok(state) => state,
            Err(error) => {
                eprintln!("Failed to initialize Well rendering: {error}");
                event_loop.exit();
                return;
            }
        };

        let (rows, cols) = terminal_dimensions(&window);
        let redraw_window = Arc::clone(&window);
        let pty = match PtySession::spawn(rows, cols, move || redraw_window.request_redraw()) {
            Ok(pty) => pty,
            Err(error) => {
                eprintln!("Failed to start terminal shell: {error}");
                event_loop.exit();
                return;
            }
        };

        window.set_ime_allowed(true);
        window.request_redraw();
        self.window = Some(window);
        self.state = Some(state);
        self.pty = Some(pty);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::Ime(winit::event::Ime::Preedit(text, _)) => {
                self.ime_preedit_active = !text.is_empty();
            }
            WindowEvent::Ime(winit::event::Ime::Commit(text)) => {
                self.ime_preedit_active = false;
                self.send_text(&text);
            }
            WindowEvent::Ime(winit::event::Ime::Enabled | winit::event::Ime::Disabled) => {
                self.ime_preedit_active = false;
            }
            WindowEvent::KeyboardInput { event, .. } => self.send_key(&event),
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    state.size = size;
                    if size.width > 0 && size.height > 0 {
                        state.config.width = size.width;
                        state.config.height = size.height;
                        state.surface.configure(&state.device, &state.config);
                    }
                }
                if let (Some(pty), Some(window)) = (&self.pty, &self.window) {
                    let (rows, cols) = terminal_dimensions(window);
                    if let Err(error) = pty.resize(rows, cols) {
                        eprintln!("Failed to resize terminal: {error}");
                    }
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.render(event_loop),
            _ => {}
        }
    }
}

impl WellApp {
    fn send_text(&self, text: &str) {
        let Some(pty) = &self.pty else {
            return;
        };
        if let Err(error) = pty.write_all(text.as_bytes()) {
            eprintln!("Failed to write text to terminal: {error}");
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn render(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(window), Some(state), Some(pty)) = (&self.window, &mut self.state, &self.pty)
        else {
            return;
        };
        if state.size.width == 0 || state.size.height == 0 {
            return;
        }

        let terminal_contents = match pty.screen_contents() {
            Ok(contents) => contents,
            Err(error) => {
                eprintln!("Terminal screen state is unavailable: {error}");
                event_loop.exit();
                return;
            }
        };

        let scale = window.scale_factor() as f32;
        let logical_size = state.size.to_logical::<f32>(window.scale_factor());
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(logical_size.width, logical_size.height),
            )),
            focused: true,
            ..Default::default()
        };
        let output = state.egui_context.run(input, |context| {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(13, 17, 23))
                        .inner_margin(egui::Margin::same(12.0)),
                )
                .show(context, |ui| {
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(terminal_contents)
                                        .monospace()
                                        .color(egui::Color32::from_rgb(220, 225, 232)),
                                )
                                .extend(),
                            );
                        });
                });
        });
        let paint_jobs = state
            .egui_context
            .tessellate(output.shapes, output.pixels_per_point);
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [state.size.width, state.size.height],
            pixels_per_point: scale,
        };

        for (texture_id, delta) in &output.textures_delta.set {
            state
                .egui_renderer
                .update_texture(&state.device, &state.queue, *texture_id, delta);
        }

        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Well Terminal UI Encoder"),
            });
        let callback_commands = state.egui_renderer.update_buffers(
            &state.device,
            &state.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        let output_frame = match state.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                state.surface.configure(&state.device, &state.config);
                window.request_redraw();
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                eprintln!("Well rendering ran out of GPU memory");
                event_loop.exit();
                return;
            }
            Err(wgpu::SurfaceError::Timeout) => return,
        };
        let view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Well Terminal UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.067,
                            b: 0.09,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            state
                .egui_renderer
                .render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }

        state.queue.submit(
            callback_commands
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        output_frame.present();
        for texture_id in &output.textures_delta.free {
            state.egui_renderer.free_texture(texture_id);
        }
    }
}

fn terminal_dimensions(window: &Window) -> (u16, u16) {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let cols = (size.width / 9.0).floor().clamp(1.0, f64::from(u16::MAX)) as u16;
    let rows = (size.height / 18.0).floor().clamp(1.0, f64::from(u16::MAX)) as u16;
    (rows, cols)
}

fn encode_csi_key(suffix: u8, modifiers: ModifiersState) -> Vec<u8> {
    let modifier = 1
        + u8::from(modifiers.shift_key())
        + 2 * u8::from(modifiers.alt_key())
        + 4 * u8::from(modifiers.control_key());
    if modifier == 1 {
        vec![0x1b, b'[', suffix]
    } else {
        format!("\x1b[1;{modifier}{}", suffix as char).into_bytes()
    }
}

fn physical_ascii_key(key: PhysicalKey) -> Option<u8> {
    let PhysicalKey::Code(code) = key else {
        return None;
    };
    match code {
        KeyCode::KeyA => Some(b'a'),
        KeyCode::KeyB => Some(b'b'),
        KeyCode::KeyC => Some(b'c'),
        KeyCode::KeyD => Some(b'd'),
        KeyCode::KeyE => Some(b'e'),
        KeyCode::KeyF => Some(b'f'),
        KeyCode::KeyG => Some(b'g'),
        KeyCode::KeyH => Some(b'h'),
        KeyCode::KeyI => Some(b'i'),
        KeyCode::KeyJ => Some(b'j'),
        KeyCode::KeyK => Some(b'k'),
        KeyCode::KeyL => Some(b'l'),
        KeyCode::KeyM => Some(b'm'),
        KeyCode::KeyN => Some(b'n'),
        KeyCode::KeyO => Some(b'o'),
        KeyCode::KeyP => Some(b'p'),
        KeyCode::KeyQ => Some(b'q'),
        KeyCode::KeyR => Some(b'r'),
        KeyCode::KeyS => Some(b's'),
        KeyCode::KeyT => Some(b't'),
        KeyCode::KeyU => Some(b'u'),
        KeyCode::KeyV => Some(b'v'),
        KeyCode::KeyW => Some(b'w'),
        KeyCode::KeyX => Some(b'x'),
        KeyCode::KeyY => Some(b'y'),
        KeyCode::KeyZ => Some(b'z'),
        KeyCode::Digit0 => Some(b'0'),
        KeyCode::Digit1 => Some(b'1'),
        KeyCode::Digit2 => Some(b'2'),
        KeyCode::Digit3 => Some(b'3'),
        KeyCode::Digit4 => Some(b'4'),
        KeyCode::Digit5 => Some(b'5'),
        KeyCode::Digit6 => Some(b'6'),
        KeyCode::Digit7 => Some(b'7'),
        KeyCode::Digit8 => Some(b'8'),
        KeyCode::Digit9 => Some(b'9'),
        KeyCode::Space => Some(b' '),
        KeyCode::BracketLeft => Some(b'['),
        KeyCode::BracketRight => Some(b']'),
        KeyCode::Backslash => Some(b'\\'),
        KeyCode::Minus => Some(b'-'),
        KeyCode::Equal => Some(b'='),
        KeyCode::Backquote => Some(b'`'),
        KeyCode::Semicolon => Some(b';'),
        KeyCode::Quote => Some(b'\''),
        KeyCode::Comma => Some(b','),
        KeyCode::Period => Some(b'.'),
        KeyCode::Slash => Some(b'/'),
        _ => None,
    }
}

fn encode_function_key(key: PhysicalKey, modifiers: ModifiersState) -> Option<Vec<u8>> {
    let PhysicalKey::Code(code) = key else {
        return None;
    };
    let number = match code {
        KeyCode::F1 => 1_usize,
        KeyCode::F2 => 2,
        KeyCode::F3 => 3,
        KeyCode::F4 => 4,
        KeyCode::F5 => 5,
        KeyCode::F6 => 6,
        KeyCode::F7 => 7,
        KeyCode::F8 => 8,
        KeyCode::F9 => 9,
        KeyCode::F10 => 10,
        KeyCode::F11 => 11,
        KeyCode::F12 => 12,
        _ => return None,
    };
    let modifier = 1
        + u8::from(modifiers.shift_key())
        + 2 * u8::from(modifiers.alt_key())
        + 4 * u8::from(modifiers.control_key());
    let sequence = match number {
        1..=4 => {
            let suffix = b'P' + (number as u8) - 1;
            if modifier == 1 {
                vec![0x1b, b'O', suffix]
            } else {
                format!("\x1b[1;{modifier}{}", suffix as char).into_bytes()
            }
        }
        _ => {
            let code = [15, 17, 18, 19, 20, 21, 23, 24][number - 5];
            if modifier == 1 {
                format!("\x1b[{code}~").into_bytes()
            } else {
                format!("\x1b[{code};{modifier}~").into_bytes()
            }
        }
    };
    Some(sequence)
}

fn control_byte(key: u8) -> u8 {
    match key {
        b' ' => 0,
        b'a'..=b'z' => key - b'a' + 1,
        b'@'..=b'_' => key & 0x1f,
        b'?' => 0x7f,
        _ => key,
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

    let capabilities = surface.get_capabilities(&adapter);
    let format = capabilities
        .formats
        .iter()
        .copied()
        .find(|format| format.is_srgb())
        .or_else(|| capabilities.formats.first().copied())
        .ok_or_else(|| "graphics surface reports no supported formats".to_owned())?;
    let present_mode = capabilities
        .present_modes
        .iter()
        .copied()
        .find(|mode| *mode == wgpu::PresentMode::Fifo)
        .or_else(|| capabilities.present_modes.first().copied())
        .ok_or_else(|| "graphics surface reports no present modes".to_owned())?;
    let alpha_mode = capabilities
        .alpha_modes
        .first()
        .copied()
        .ok_or_else(|| "graphics surface reports no alpha modes".to_owned())?;
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode,
        alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    let egui_context = egui::Context::default();
    egui_context.set_visuals(egui::Visuals::dark());
    let egui_renderer = egui_wgpu::Renderer::new(&device, format, None, 1);

    Ok(RenderState {
        surface,
        device,
        queue,
        config,
        size,
        egui_context,
        egui_renderer,
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

    if let Err(error) = event_loop.run_app(&mut WellApp::default()) {
        eprintln!("Well event loop failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::{control_byte, encode_csi_key, encode_function_key, physical_ascii_key};
    use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

    #[test]
    fn encodes_terminal_control_and_modified_arrow_keys() {
        assert_eq!(control_byte(b'c'), 3);
        assert_eq!(control_byte(b'?'), 127);
        assert_eq!(encode_csi_key(b'A', ModifiersState::SHIFT), b"\x1b[1;2A");
    }

    #[test]
    fn maps_physical_letter_keys_for_control_shortcuts() {
        assert_eq!(
            physical_ascii_key(PhysicalKey::Code(KeyCode::KeyC)),
            Some(b'c')
        );
    }

    #[test]
    fn encodes_function_keys_and_modifiers() {
        assert_eq!(
            encode_function_key(PhysicalKey::Code(KeyCode::F1), ModifiersState::empty()),
            Some(b"\x1bOP".to_vec())
        );
        assert_eq!(
            encode_function_key(PhysicalKey::Code(KeyCode::F5), ModifiersState::SHIFT),
            Some(b"\x1b[15;2~".to_vec())
        );
    }
}
