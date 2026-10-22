mod input;
mod rpc;
mod selection;
mod ui;
mod workspace;

use std::sync::Arc;
use std::time::{Duration, Instant};
use ui::Gui;
use well_config::{ConfigStore, Preferences};
use well_ipc::HermesAppCommand;
use well_render::TextRenderer;
use well_shell::pty::{MouseProtocolMode, SessionEvent};
use workspace::{Action, Workspace};

#[derive(Debug)]
pub enum AppEvent {
    Session { id: u64, event: SessionEvent },
    RpcWake,
    RpcStopped(Result<(), String>),
}
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

struct RenderState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    text: TextRenderer,
    fonts: well_render::FontCatalog,
    gui: Gui,
}
impl RenderState {
    fn new(
        window: Arc<Window>,
        preferences: &Preferences,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or("No compatible GPU adapter")?;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Well terminal"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))?;
        let caps = surface.get_capabilities(&adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps
                .formats
                .iter()
                .copied()
                .find(|f| f.is_srgb())
                .unwrap_or(caps.formats[0]),
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let fonts = well_render::FontCatalog::default();
        let text = TextRenderer::with_appearance(
            &device,
            &config,
            window.scale_factor(),
            fonts.clone(),
            &preferences.appearance,
        )?;
        let gui = Gui::new(&device, config.format, preferences);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            text,
            fonts,
            gui,
        })
    }
}

struct WellApp {
    proxy: EventLoopProxy<AppEvent>,
    rpc: rpc::RpcHost,
    window: Option<Arc<Window>>,
    state: Option<RenderState>,
    session: Option<Workspace>,
    modifiers: ModifiersState,
    composing: bool,
    preedit: String,
    redraw_at: Option<Instant>,
    clipboard: Option<arboard::Clipboard>,
    exited: bool,
    wheel_pixels: f64,
    fatal_error: Option<String>,
    preferences: Preferences,
    config_store: ConfigStore,
    selection: Option<selection::Selection>,
    pointer: winit::dpi::PhysicalPosition<f64>,
    mouse_buttons: u8,
    mouse_cell: Option<(u16, u16)>,
}
impl WellApp {
    fn perform_action(
        &mut self,
        action: Action,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), String> {
        let workspace = self.session.as_ref().ok_or("Workspace is not ready")?;
        match action {
            Action::SwitchTab(index) if index >= workspace.len() => {
                return Err(format!("Tab index {index} does not exist"))
            }
            Action::SwitchTab(index) if index == workspace.active_index() => return Ok(()),
            Action::NewTab if workspace.len() >= workspace::MAX_TABS => {
                return Err(format!("The limit is {} tabs", workspace::MAX_TABS))
            }
            _ => {}
        }
        for button in 0..3 {
            if self.mouse_buttons & (1 << button) != 0 {
                self.report_mouse(button, false);
            }
        }
        let workspace = self.session.as_mut().ok_or("Workspace is not ready")?;
        match action {
            Action::NewTab => {
                workspace.new_tab()?;
            }
            Action::SwitchTab(index) => workspace.switch_tab(index)?,
            Action::CloseTab => {
                if !workspace.close_tab() {
                    self.session = None;
                    event_loop.exit();
                    return Ok(());
                }
            }
        }
        self.exited = workspace.exited();
        if let Some(window) = &self.window {
            window.set_title(&format!(
                "Well — Shell {}{}",
                workspace.active_id(),
                if self.exited { " (exited)" } else { "" }
            ));
        }
        self.selection = None;
        self.mouse_buttons = 0;
        self.mouse_cell = None;
        self.wheel_pixels = 0.0;
        self.composing = false;
        self.preedit.clear();
        self.resize();
        Ok(())
    }
    fn dispatch_rpc(&mut self, event_loop: &ActiveEventLoop) {
        // Bound one dispatch turn so a busy automation client cannot starve input.
        for _ in 0..64 {
            let Ok(command) = self.rpc.commands.try_recv() else {
                break;
            };
            match command {
                HermesAppCommand::ReadSurface {
                    surface_id,
                    respond_to,
                } => {
                    if !respond_to.is_closed() {
                        let result = self
                            .session
                            .as_ref()
                            .ok_or("Workspace is not ready".into())
                            .and_then(|s| s.read_surface(surface_id));
                        let _ = respond_to.send(result);
                    }
                }
                HermesAppCommand::WorkspaceState { respond_to } => {
                    if !respond_to.is_closed() {
                        let _ = respond_to.send(
                            self.session
                                .as_ref()
                                .map(|s| s.state())
                                .ok_or("Workspace is not ready".into()),
                        );
                    }
                }
                HermesAppCommand::NewTab { respond_to } => {
                    if !respond_to.is_closed() {
                        let result = self
                            .perform_action(Action::NewTab, event_loop)
                            .map(|_| self.session.as_ref().unwrap().state());
                        let _ = respond_to.send(result);
                    }
                }
                HermesAppCommand::SwitchTab {
                    tab_index,
                    respond_to,
                } => {
                    if !respond_to.is_closed() {
                        let result = self
                            .perform_action(Action::SwitchTab(tab_index), event_loop)
                            .map(|_| self.session.as_ref().unwrap().state());
                        let _ = respond_to.send(result);
                    }
                }
                HermesAppCommand::SendShellInput {
                    input_text,
                    respond_to,
                } => {
                    if !respond_to.is_closed() {
                        let result = self
                            .session
                            .as_ref()
                            .ok_or("Workspace is not ready".into())
                            .and_then(|s| s.send_to(None, &input_text));
                        if result.is_ok() {
                            self.selection = None;
                            self.dirty();
                        }
                        let _ = respond_to.send(result);
                    }
                }
                HermesAppCommand::SendSurfaceInput {
                    surface_id,
                    input_text,
                    respond_to,
                } => {
                    if !respond_to.is_closed() {
                        let result = self
                            .session
                            .as_ref()
                            .ok_or("Workspace is not ready".into())
                            .and_then(|s| s.send_to(Some(surface_id), &input_text));
                        if result.is_ok() {
                            self.dirty();
                        }
                        let _ = respond_to.send(result);
                    }
                }
                HermesAppCommand::SplitPane { respond_to, .. }
                | HermesAppCommand::InsertEditorText { respond_to, .. }
                | HermesAppCommand::PublishAgentLifecycle { respond_to, .. } => {
                    let _ = respond_to
                        .send(Err("This operation is not registered by the desktop".into()));
                }
            }
        }
        if !self.rpc.commands.is_empty() {
            let _ = self.proxy.send_event(AppEvent::RpcWake);
        }
    }
    fn dirty(&mut self) {
        let deadline = Instant::now() + Duration::from_millis(4);
        self.redraw_at = Some(self.redraw_at.map_or(deadline, |old| old.min(deadline)));
    }
    fn error(&mut self, message: &str) {
        eprintln!("Well: {message}");
        if let Some(window) = &self.window {
            window.set_title(&format!("Well — {message}"));
        }
        if let Some(session) = &self.session {
            session
                .parser
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .process(format!("\r\n[Well: {message}]\r\n").as_bytes());
        }
        self.dirty();
    }
    fn send(&mut self, bytes: &[u8]) {
        if self.exited {
            return;
        }
        self.selection = None;
        if let Some(session) = &self.session {
            session.scroll_to_bottom();
            if let Err(e) = session.write_all(bytes) {
                self.error(&e.to_string());
            }
        }
        self.dirty();
    }
    fn resize(&mut self) {
        self.selection = None;
        let (Some(state), Some(window)) = (&mut self.state, &self.window) else {
            return;
        };
        state.size = window.inner_size();
        if state.size.width == 0 || state.size.height == 0 {
            return;
        }
        state.config.width = state.size.width;
        state.config.height = state.size.height;
        state.surface.configure(&state.device, &state.config);
        let region = Gui::region(
            window,
            &self.preferences,
            self.session.as_ref().map_or(1, |s| s.len()),
        );
        let (rows, cols) = state.text.metrics.grid(region[2], region[3]);
        if let Some(session) = &mut self.session {
            if let Err(e) = session.resize(rows, cols) {
                self.error(&e.to_string());
            }
        }
        self.dirty();
    }
    fn draw(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(workspace) = &self.session {
            workspace.rendered();
        }
        self.redraw_at = None;
        let (Some(state), Some(window)) = (&mut self.state, &self.window) else {
            return;
        };
        if state.size.width == 0 || state.size.height == 0 {
            return;
        }
        let terminal_cursor = self.session.as_ref().and_then(|session| {
            let parser = session.parser.lock().unwrap_or_else(|p| p.into_inner());
            if parser.screen().hide_cursor() || parser.screen().scrollback() != 0 {
                return None;
            }
            let (row, col) = parser.screen().cursor_position();
            let region = Gui::region(
                window,
                &self.preferences,
                self.session.as_ref().map_or(1, |s| s.len()),
            );
            let m = state.text.metrics;
            let scale = window.scale_factor() as f32;
            Some(egui::Rect::from_min_size(
                egui::pos2(
                    (region[0] + m.padding + col as u32 * m.width) as f32 / scale,
                    (region[1] + m.padding + row as u32 * m.height) as f32 / scale,
                ),
                egui::vec2(m.width as f32 / scale, m.height as f32 / scale),
            ))
        });
        let (ui_output, candidate) = state.gui.frame(
            window,
            &self.preferences,
            self.config_store.path(),
            self.exited,
            self.selection.as_ref().is_some_and(|s| s.range().is_some()),
            &self.preedit,
            terminal_cursor,
            &self
                .session
                .as_ref()
                .map(|s| s.tab_info())
                .unwrap_or_default(),
            self.session.as_ref().map_or(0, |s| s.active_index()),
            &self.rpc,
        );
        if state.gui.settings.open {
            self.preedit.clear();
            self.composing = false;
        }
        let actions = std::mem::take(&mut state.gui.actions);
        for action in actions {
            if let Err(error) = self.perform_action(action, event_loop) {
                self.error(&error);
            }
            if self.session.is_none() {
                return;
            }
        }
        if let Some(preferences) = candidate {
            self.apply_preferences(preferences);
        }
        let (Some(state), Some(session), Some(window)) =
            (&mut self.state, &self.session, &self.window)
        else {
            return;
        };
        if !ui_output.platform_output.copied_text.is_empty() {
            if self.clipboard.is_none() {
                self.clipboard = arboard::Clipboard::new().ok();
            }
            let result = self
                .clipboard
                .as_mut()
                .ok_or_else(|| "Clipboard unavailable".to_string())
                .and_then(|c| {
                    c.set_text(ui_output.platform_output.copied_text.clone())
                        .map_err(|e| e.to_string())
                });
            if let Err(error) = result {
                state.gui.settings.error = Some(error);
            }
        }
        let output = match state.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                state.surface.configure(&state.device, &state.config);
                self.dirty();
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                self.error("GPU ran out of memory");
                self.fatal_error = Some("GPU ran out of memory".into());
                event_loop.exit();
                return;
            }
            Err(wgpu::SurfaceError::Timeout) => {
                self.dirty();
                return;
            }
        };
        let view = output.texture.create_view(&Default::default());
        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Well frame"),
            });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Terminal background"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(state.text.background_color()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        let screen = self
            .selection
            .as_ref()
            .filter(|s| s.range().is_some())
            .map(|s| s.screen.clone())
            .unwrap_or_else(|| session.snapshot());
        // Graphics and hyperlink display are deferred; consume notifications so
        // shells emitting OSC 7/8 do not accumulate an unbounded event backlog.
        if let Ok(events) = session.graphic_events_rx.lock() {
            while events.try_recv().is_ok() {}
        }
        state.text.draw_selected(
            &state.device,
            &state.queue,
            &mut encoder,
            &view,
            [state.size.width, state.size.height],
            Gui::region(
                window,
                &self.preferences,
                self.session.as_ref().map_or(1, |s| s.len()),
            ),
            &screen,
            self.selection.as_ref().and_then(|s| s.range()),
        );
        let repaint = Gui::repaint_after(&ui_output);
        let jobs = state
            .gui
            .ctx
            .tessellate(ui_output.shapes, ui_output.pixels_per_point);
        for (id, delta) in &ui_output.textures_delta.set {
            state
                .gui
                .renderer
                .update_texture(&state.device, &state.queue, *id, delta);
        }
        let descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [state.size.width, state.size.height],
            pixels_per_point: ui_output.pixels_per_point,
        };
        let commands = state.gui.renderer.update_buffers(
            &state.device,
            &state.queue,
            &mut encoder,
            &jobs,
            &descriptor,
        );
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Well controls"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            state.gui.renderer.render(&mut pass, &jobs, &descriptor);
        }
        state.queue.submit(
            commands
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        for id in ui_output.textures_delta.free {
            state.gui.renderer.free_texture(&id);
        }
        window_pre_present(&self.window);
        output.present();
        if let Some(delay) = repaint {
            let deadline = Instant::now() + delay.max(Duration::from_millis(4));
            self.redraw_at = Some(self.redraw_at.map_or(deadline, |old| old.min(deadline)));
        }
    }
    fn apply_preferences(&mut self, preferences: Preferences) {
        let (Some(state), Some(window)) = (&mut self.state, &self.window) else {
            return;
        };
        let prepared = TextRenderer::with_appearance(
            &state.device,
            &state.config,
            window.scale_factor(),
            state.fonts.clone(),
            &preferences.appearance,
        );
        match prepared {
            Ok(text) => match self.config_store.save(&preferences) {
                Ok(()) => {
                    state.text = text;
                    if let Some(session) = &mut self.session {
                        session.set_appearance(&preferences.appearance);
                    }
                    self.preferences = preferences;
                    state.gui.settings.applied();
                    self.resize();
                }
                Err(e) => state.gui.settings.error = Some(e.to_string()),
            },
            Err(e) => state.gui.settings.error = Some(e.to_string()),
        }
        self.dirty();
    }
    fn cell_at_pointer(&self) -> Option<(u16, u16)> {
        let (Some(state), Some(window), Some(session)) = (&self.state, &self.window, &self.session)
        else {
            return None;
        };
        let region = Gui::region(
            window,
            &self.preferences,
            self.session.as_ref().map_or(1, |s| s.len()),
        );
        let metrics = state.text.metrics;
        let (rows, cols) = session
            .parser
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .screen()
            .size();
        let col = ((self.pointer.x - region[0] as f64 - metrics.padding as f64)
            / metrics.width as f64)
            .floor()
            .max(0.0) as u16;
        let row = ((self.pointer.y - region[1] as f64 - metrics.padding as f64)
            / metrics.height as f64)
            .floor()
            .max(0.0) as u16;
        Some((row.min(rows - 1), col.min(cols - 1)))
    }
    fn report_mouse(&mut self, code: u8, pressed: bool) {
        let Some((row, col)) = self.cell_at_pointer() else {
            return;
        };
        let Some(session) = &self.session else {
            return;
        };
        let (mode, encoding) = session.mouse_modes();
        if !pressed && mode == MouseProtocolMode::Press {
            return;
        }
        let modifiers = if mode == MouseProtocolMode::Press {
            ModifiersState::empty()
        } else {
            self.modifiers
        };
        if let Some(bytes) = input::mouse(code, pressed, row, col, modifiers, encoding) {
            self.send(&bytes);
        }
    }
    fn mouse_reporting(&self) -> bool {
        !self.modifiers.shift_key()
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.mouse_modes().0 != MouseProtocolMode::None)
    }
    fn copy_selection(&mut self) {
        let Some(text) = self.selection.as_ref().and_then(|s| s.text()) else {
            return;
        };
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        let result = self
            .clipboard
            .as_mut()
            .ok_or_else(|| "Clipboard unavailable".to_owned())
            .and_then(|c| c.set_text(text).map_err(|e| e.to_string()));
        if let Err(error) = result {
            self.error(&error);
        }
    }
}
fn window_pre_present(window: &Option<Arc<Window>>) {
    if let Some(window) = window {
        window.pre_present_notify();
    }
}

impl ApplicationHandler<AppEvent> for WellApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let attributes = Window::default_attributes()
                .with_title("Well Shell")
                .with_inner_size(winit::dpi::LogicalSize::new(960.0, 640.0))
                .with_min_inner_size(winit::dpi::LogicalSize::new(480.0, 360.0));
            #[cfg(target_os = "linux")]
            let attributes = {
                let attributes = winit::platform::x11::WindowAttributesExtX11::with_name(
                    attributes, "well", "well",
                );
                winit::platform::wayland::WindowAttributesExtWayland::with_name(
                    attributes,
                    "org.well.Terminal",
                    "well",
                )
            };
            let window = Arc::new(event_loop.create_window(attributes)?);
            window.set_ime_allowed(true);
            let state = RenderState::new(window.clone(), &self.preferences)?;
            let region = Gui::region(&window, &self.preferences, 1);
            let (rows, cols) = state.text.metrics.grid(region[2], region[3]);
            let session = Workspace::new(
                self.proxy.clone(),
                self.rpc.path.clone(),
                self.preferences.appearance.clone(),
                rows,
                cols,
            )?;
            self.window = Some(window);
            self.state = Some(state);
            self.session = Some(session);
            self.dirty();
            Ok(())
        })();
        if let Err(e) = result {
            self.error(&e.to_string());
            self.fatal_error = Some(e.to_string());
            event_loop.exit();
        }
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::Session { id, event } => {
                if let Some(workspace) = &mut self.session {
                    let changed = workspace.apply_event(id, event);
                    self.exited = workspace.exited();
                    if changed {
                        self.dirty();
                    }
                }
            }
            AppEvent::RpcWake => self.dispatch_rpc(event_loop),
            AppEvent::RpcStopped(result) => {
                let error = result
                    .err()
                    .unwrap_or_else(|| "Automation server stopped".into());
                self.rpc.error = Some(error.clone());
                self.error(&error);
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let WindowEvent::CursorMoved { position, .. } = &event {
            self.pointer = *position;
        }
        if let WindowEvent::KeyboardInput {
            event,
            is_synthetic: false,
            ..
        } = &event
        {
            let app_shortcut = if cfg!(target_os = "macos") {
                self.modifiers.super_key()
            } else {
                self.modifiers.control_key() && self.modifiers.shift_key()
            };
            if event.state == ElementState::Pressed
                && self.modifiers.control_key()
                && !self.modifiers.alt_key()
                && event.logical_key == Key::Named(NamedKey::Tab)
            {
                if let Some(workspace) = &self.session {
                    let count = workspace.len();
                    let index = (workspace.active_index()
                        + if self.modifiers.shift_key() {
                            count - 1
                        } else {
                            1
                        })
                        % count;
                    if let Err(error) = self.perform_action(Action::SwitchTab(index), event_loop) {
                        self.error(&error);
                    }
                }
                return;
            }
            if event.state == ElementState::Pressed
                && app_shortcut
                && matches!(&event.logical_key, Key::Character(ch) if ch.eq_ignore_ascii_case("q"))
            {
                event_loop.exit();
                return;
            }
            if event.state == ElementState::Pressed && app_shortcut {
                let action = match &event.logical_key {
                    Key::Character(ch) if ch.eq_ignore_ascii_case("t") => Some(Action::NewTab),
                    Key::Character(ch) if ch.eq_ignore_ascii_case("w") => Some(Action::CloseTab),
                    Key::Character(ch) => ch
                        .parse::<usize>()
                        .ok()
                        .filter(|n| (1..=9).contains(n))
                        .map(|n| Action::SwitchTab(n - 1)),
                    _ => None,
                };
                if let Some(action) = action {
                    if let Err(error) = self.perform_action(action, event_loop) {
                        self.error(&error);
                    }
                    return;
                }
            }
            if event.state == ElementState::Pressed
                && (event.logical_key == Key::Named(NamedKey::F12)
                    || ((if cfg!(target_os = "macos") {
                        self.modifiers.super_key()
                    } else {
                        self.modifiers.control_key()
                    }) && event.logical_key == Key::Character(",".into())))
            {
                if let Some(state) = &mut self.state {
                    state.gui.settings.toggle(&self.preferences);
                }
                self.dirty();
                return;
            }
        }
        let consumed = if let (Some(state), Some(window)) = (&mut self.state, &self.window) {
            state.gui.event(window, &event, &mut self.clipboard)
        } else {
            false
        };
        if matches!(
            event,
            WindowEvent::CursorMoved { .. }
                | WindowEvent::CursorLeft { .. }
                | WindowEvent::MouseInput { .. }
                | WindowEvent::KeyboardInput { .. }
                | WindowEvent::Ime(_)
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::Focused(_)
        ) {
            self.dirty();
        }
        let terminal_capture = (self.selection.as_ref().is_some_and(|s| s.dragging)
            || self.mouse_buttons != 0)
            && matches!(
                event,
                WindowEvent::CursorMoved { .. } | WindowEvent::MouseInput { .. }
            );
        if consumed && !terminal_capture {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) => self.resize(),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(state) = &mut self.state {
                    match TextRenderer::with_appearance(
                        &state.device,
                        &state.config,
                        scale_factor,
                        state.fonts.clone(),
                        &self.preferences.appearance,
                    ) {
                        Ok(text) => state.text = text,
                        Err(e) => {
                            self.error(&e.to_string());
                            return;
                        }
                    }
                }
                self.resize();
            }
            WindowEvent::RedrawRequested => self.draw(event_loop),
            WindowEvent::ModifiersChanged(mods) => self.modifiers = mods.state(),
            WindowEvent::Focused(false) => {
                for button in 0..3 {
                    if self.mouse_buttons & (1 << button) != 0 {
                        self.report_mouse(button, false);
                    }
                }
                self.modifiers = ModifiersState::empty();
                self.composing = false;
                self.preedit.clear();
                self.mouse_buttons = 0;
                if let Some(selection) = &mut self.selection {
                    selection.dragging = false;
                }
            }
            WindowEvent::Ime(Ime::Preedit(text, _)) => {
                self.composing = !text.is_empty();
                self.preedit = text;
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.composing = false;
                self.preedit.clear();
                self.send(text.as_bytes());
            }
            WindowEvent::Ime(Ime::Disabled) => {
                self.composing = false;
                self.preedit.clear();
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } if event.state == ElementState::Pressed => {
                if self.composing {
                    return;
                }
                if event.logical_key == Key::Named(NamedKey::Escape)
                    && self.selection.take().is_some()
                {
                    self.dirty();
                    return;
                }
                if (cfg!(target_os = "macos") && self.modifiers.super_key())
                    || (!cfg!(target_os = "macos")
                        && self.modifiers.control_key()
                        && self.modifiers.shift_key())
                {
                    if let Key::Character(ch) = &event.logical_key {
                        if ch.eq_ignore_ascii_case("q") {
                            event_loop.exit();
                        }
                        if ch.eq_ignore_ascii_case("c") {
                            self.copy_selection();
                        }
                        if ch.eq_ignore_ascii_case("v") {
                            if self.clipboard.is_none() {
                                self.clipboard = arboard::Clipboard::new().ok();
                            }
                            let text = self
                                .clipboard
                                .as_mut()
                                .ok_or("Clipboard unavailable".to_owned())
                                .and_then(|c| c.get_text().map_err(|e| e.to_string()));
                            match text {
                                Ok(text) => {
                                    let bracketed = self
                                        .session
                                        .as_ref()
                                        .map(|s| s.input_modes().1)
                                        .unwrap_or(false);
                                    self.send(&input::paste(&text, bracketed));
                                }
                                Err(e) => self.error(&e),
                            }
                        }
                    }
                    return;
                }
                if self.modifiers.shift_key()
                    && matches!(
                        event.logical_key,
                        Key::Named(NamedKey::PageUp | NamedKey::PageDown)
                    )
                {
                    if let Some(session) = &self.session {
                        let rows = session.snapshot().rows as i32;
                        session.scroll(if event.logical_key == Key::Named(NamedKey::PageUp) {
                            rows
                        } else {
                            -rows
                        });
                    }
                    self.dirty();
                    return;
                }
                let app_cursor = self
                    .session
                    .as_ref()
                    .map(|s| s.input_modes().0)
                    .unwrap_or(false);
                if let Some(bytes) = input::encode(
                    &event.logical_key,
                    event.text.as_deref(),
                    self.modifiers,
                    app_cursor,
                ) {
                    self.send(&bytes);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => {
                        (y * self.preferences.scroll_lines as f32).round() as i32
                    }
                    MouseScrollDelta::PixelDelta(p) => {
                        self.wheel_pixels += p.y;
                        let height = self
                            .state
                            .as_ref()
                            .map(|s| s.text.metrics.height)
                            .unwrap_or(16) as f64;
                        let lines = (self.wheel_pixels / height).trunc() as i32;
                        self.wheel_pixels -= lines as f64 * height;
                        lines
                    }
                };
                if self.mouse_reporting() {
                    for _ in 0..lines.unsigned_abs().min(100) {
                        self.report_mouse(if lines > 0 { 64 } else { 65 }, true);
                    }
                    return;
                }
                self.selection = None;
                if let Some(session) = &self.session {
                    session.scroll(lines);
                }
                self.dirty();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                if button == MouseButton::Left
                    && !pressed
                    && self.selection.as_ref().is_some_and(|s| s.dragging)
                {
                    if let Some(selection) = &mut self.selection {
                        selection.dragging = false;
                    }
                    if self.selection.as_ref().is_some_and(|s| s.range().is_none()) {
                        self.selection = None;
                    }
                    self.dirty();
                    return;
                }
                let code = match button {
                    MouseButton::Left => 0,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return,
                };
                if self.mouse_reporting() {
                    if pressed {
                        self.mouse_buttons |= 1 << code;
                    } else {
                        self.mouse_buttons &= !(1 << code);
                    }
                    let mode = self.session.as_ref().unwrap().mouse_modes().0;
                    if pressed || mode != MouseProtocolMode::Press {
                        self.report_mouse(code, pressed);
                    }
                } else if button == MouseButton::Left && pressed {
                    if let (Some(session), Some((row, col))) =
                        (&self.session, self.cell_at_pointer())
                    {
                        self.selection =
                            Some(selection::Selection::new(session.snapshot(), row, col));
                    }
                }
                self.dirty();
            }
            WindowEvent::CursorMoved { .. } => {
                let cell = self.cell_at_pointer();
                if let Some(selection) = &mut self.selection {
                    if selection.dragging {
                        if let Some((row, col)) = cell {
                            selection.extend(row, col);
                        }
                        self.dirty();
                        return;
                    }
                }
                if self.mouse_reporting() && cell != self.mouse_cell {
                    self.mouse_cell = cell;
                    let mode = self.session.as_ref().unwrap().mouse_modes().0;
                    if mode == MouseProtocolMode::AnyMotion
                        || (mode == MouseProtocolMode::ButtonMotion && self.mouse_buttons != 0)
                    {
                        let button = (0..3)
                            .find(|b| self.mouse_buttons & (1 << b) != 0)
                            .unwrap_or(3);
                        self.report_mouse(32 + button, true);
                    }
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(deadline) = self.redraw_at {
            if Instant::now() >= deadline {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.rpc.shutdown();
        if let Some(mut session) = self.session.take() {
            session.shutdown();
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = ConfigStore::default_path()?;
    let (config_store, preferences) = ConfigStore::load(path.clone())
        .map_err(|e| format!("Cannot load settings at {}: {e}", path.display()))?;
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let rpc = rpc::RpcHost::new(proxy.clone())?;
    let mut app = WellApp {
        proxy,
        rpc,
        window: None,
        state: None,
        session: None,
        modifiers: ModifiersState::empty(),
        composing: false,
        preedit: String::new(),
        redraw_at: None,
        clipboard: None,
        exited: false,
        wheel_pixels: 0.0,
        fatal_error: None,
        preferences,
        config_store,
        selection: None,
        pointer: winit::dpi::PhysicalPosition::new(0.0, 0.0),
        mouse_buttons: 0,
        mouse_cell: None,
    };
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.fatal_error {
        return Err(error.into());
    }
    Ok(())
}
