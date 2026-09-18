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

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use well_config::TheiasPrismPanel;
use well_editor::MnemeEditor;
use well_ipc::{HermesChannel, TheiaConfigPayload};
use well_prompt::{AstraeaStateVector, PromptCompiler};
mod pane_manager;
mod tab_bar;
mod pythia;
use crate::pane_manager::PaneManager;
use crate::tab_bar::TabBar;
use well_render::{CellInstance, OrpheusRenderer, SelectionRange};
use crate::pythia::PythiaClient;
use well_shell::{MetisExecutor, pty::PtySession};
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent, Modifiers, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    window::WindowBuilder,
};

#[derive(Debug, Clone, Copy)]
pub enum PtyEvent {
    NewOutput,
}

pub struct WellTerminalState {
    pub pythia: PythiaClient,
    pub channel: Arc<HermesChannel<TheiaConfigPayload>>,
    pub shell: MetisExecutor,
    pub editor: MnemeEditor,
    pub prompt_compiler: PromptCompiler,
    pub config_panel: TheiasPrismPanel,
    pub pty_session: Arc<PtySession>,
    pub pane_manager: PaneManager,
    pub tab_bar: TabBar,
    pub is_vcr_mode: bool,
}

impl WellTerminalState {
    pub fn new(pty_session: Arc<PtySession>) -> Self {
        let channel = Arc::new(HermesChannel::new(TheiaConfigPayload::default()));
        let prompt_state = Arc::new(AstraeaStateVector::new());
        let prompt_compiler = PromptCompiler::new(prompt_state);
        let config_panel = TheiasPrismPanel::new(Arc::clone(&channel));
        let pane_manager = PaneManager::new(Arc::clone(&pty_session));
        let tab_bar = TabBar::new();
        let pythia = PythiaClient::new();

        Self {
            channel,
            shell: MetisExecutor::new(),
            editor: MnemeEditor::default(),
            prompt_compiler,
            config_panel,
            pty_session,
            pane_manager,
            tab_bar,
            pythia,
            is_vcr_mode: false,
        }
    }
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
extern "C" {
    fn CGEventSourceFlagsState(state: i32) -> u64;
    fn CGEventCreate(source: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn CGEventGetLocation(event: *mut std::ffi::c_void) -> CGPoint;
    fn CFRelease(cf: *mut std::ffi::c_void);
}

#[cfg(target_os = "macos")]
fn get_global_mouse_location() -> (f64, f64) {
    unsafe {
        let event = CGEventCreate(std::ptr::null_mut());
        if event.is_null() {
            return (0.0, 0.0);
        }
        let loc = CGEventGetLocation(event);
        CFRelease(event);
        (loc.x, loc.y)
    }
}

#[cfg(not(target_os = "macos"))]
fn get_global_mouse_location() -> (f64, f64) {
    (0.0, 0.0)
}

pub fn query_macos_modifiers() -> (bool, bool, bool, bool) {
    #[cfg(target_os = "macos")]
    {
        let flags = unsafe { CGEventSourceFlagsState(0) };
        let is_super = (flags & (1 << 20)) != 0;
        let is_shift = (flags & (1 << 17)) != 0;
        let is_ctrl = (flags & (1 << 18)) != 0;
        let is_alt = (flags & (1 << 19)) != 0;
        (is_super, is_ctrl, is_shift, is_alt)
    }
    #[cfg(not(target_os = "macos"))]
    {
        (false, false, false, false)
    }
}

fn get_max_scrollback(screen: &mut vt100::Screen) -> usize {
    let cur = screen.scrollback();
    screen.set_scrollback(usize::MAX);
    let max = screen.scrollback();
    screen.set_scrollback(cur);
    max
}

static CLIPBOARD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn copy_to_clipboard(text: &str) {
    if text.is_empty() {
        return;
    }
    let _guard = CLIPBOARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut success = false;
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if cb.set_text(text).is_ok() {
            success = true;
        }
    }
    // Native macOS pbcopy fallback if arboard encounters an issue
    if !success && cfg!(target_os = "macos") {
        if let Ok(mut child) = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }
}

fn get_from_clipboard() -> Option<String> {
    let _guard = CLIPBOARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if let Ok(text) = cb.get_text() {
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    // Native macOS pbpaste fallback
    if cfg!(target_os = "macos") {
        if let Ok(output) = std::process::Command::new("pbpaste").output() {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
        }
    }
    None
}

fn paste_into_terminal(pty_session: &PtySession, text: &str) {
    if text.is_empty() {
        return;
    }
    let bracketed = if let Ok(parser) = pty_session.parser.lock() {
        parser.screen().bracketed_paste()
    } else {
        false
    };

    let mut payload = Vec::new();
    let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
    if bracketed {
        payload.extend_from_slice(b"\x1b[200~");
        payload.extend_from_slice(normalized.as_bytes());
        payload.extend_from_slice(b"\x1b[201~");
    } else {
        payload.extend_from_slice(normalized.as_bytes());
    }

    let _ = pty_session.write_all(&payload);
}

fn run_go_demo() {
    let bin_path = std::path::Path::new("go-demo/go-demo");
    match Command::new(bin_path).output() {
        Ok(output) => {
            println!(
                "Go demo output: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        Err(e) => eprintln!("Failed to run go demo: {}", e),
    }
}

fn egui_modifiers_from_winit(modifiers: &Modifiers) -> egui::Modifiers {
    let state = modifiers.state();
    egui::Modifiers {
        alt: state.alt_key(),
        ctrl: state.control_key(),
        shift: state.shift_key(),
        mac_cmd: state.super_key(),
        command: state.super_key() || state.control_key(),
    }
}

fn get_line_text_from_screen(screen: &vt100::Screen, row: u16) -> String {
    let (_, screen_cols) = screen.size();
    let mut line = String::new();
    for c in 0..screen_cols {
        if let Some(cell) = screen.cell(row, c) {
            let s = cell.contents();
            if s.is_empty() {
                line.push(' ');
            } else {
                line.push_str(s);
            }
        } else {
            line.push(' ');
        }
    }
    line.trim_end().to_string()
}

fn detect_url_in_line(line: &str) -> Option<(usize, usize, String)> {
    for prefix in &["https://", "http://", "file://"] {
        if let Some(start) = line.find(prefix) {
            let tail = &line[start..];
            let end_offset = tail
                .find(|c: char| c.is_whitespace() || ['"', '\'', ')', ']', '}', '>', ','].contains(&c))
                .unwrap_or(tail.len());
            let url = &tail[..end_offset];
            if url.len() > prefix.len() {
                return Some((start, start + end_offset, url.to_string()));
            }
        }
    }
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn,egui_extras=trace,egui=trace")).init();
    let event_loop = EventLoopBuilder::<PtyEvent>::with_user_event().build()?;

    #[cfg(target_os = "macos")]
    use winit::platform::macos::WindowBuilderExtMacOS;

    let window_builder = WindowBuilder::new()
        .with_title("Well-Shell")
        .with_inner_size(LogicalSize::new(1280.0, 720.0))
        .with_min_inner_size(LogicalSize::new(640.0, 360.0));

    #[cfg(target_os = "macos")]
    let window_builder = window_builder
        .with_fullsize_content_view(true)
        .with_titlebar_transparent(true)
        .with_titlebar_buttons_hidden(true)
        .with_title_hidden(true);

    let window = Arc::new(window_builder.build(&event_loop)?);

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
    let mut scale_factor = window.scale_factor() as f32;
    let initial_config = well_ipc::TheiaConfigPayload::default();
    let current_font_size = initial_config.font_size;
    let current_line_height = initial_config.line_height;
    let mut cell_width = (current_font_size * 0.65 * scale_factor).max(6.0);
    let mut cell_height = (current_font_size * current_line_height * scale_factor).max(10.0);
    let top_bar_h = well_config::TOP_BAR_HEIGHT * scale_factor;
    let margin = well_config::FRAME_MARGIN * scale_factor;
    let usable_w = (size.width as f32 - 2.0 * margin).max(cell_width);
    let usable_h = (size.height as f32 - top_bar_h - margin).max(cell_height);
    let mut grid_cols = (usable_w / cell_width).floor() as u32;
    let mut grid_rows = (usable_h / cell_height).floor() as u32;

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

    let present_mode = if surface_caps.present_modes.contains(&wgpu::PresentMode::AutoNoVsync) {
        wgpu::PresentMode::AutoNoVsync
    } else if surface_caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
        wgpu::PresentMode::Immediate
    } else {
        surface_caps.present_modes[0]
    };

    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&renderer.device, &surface_config);
    renderer.set_offsets(margin, top_bar_h, size.width as f32, size.height as f32);

    println!(
        "Atlas Window created: {}x{} (scale factor: {})",
        size.width, size.height, scale_factor
    );
    println!("Surface format: {:?}", surface_format);
    println!(
        "Grid matrix: {} cols x {} rows (cell: {}x{})",
        grid_cols, grid_rows, cell_width, cell_height
    );

    // 5. Initialize interactive PTY session with coalesced redraw signaling
    let proxy = event_loop.create_proxy();
    let pty_dirty = Arc::new(AtomicBool::new(false));
    let pty_dirty_clone = Arc::clone(&pty_dirty);
    let pty_session = Arc::new(PtySession::spawn(
        grid_rows.max(1) as u16,
        grid_cols.max(1) as u16,
        move || {
            // Atomic coalescing: only notify Winit when dirty transitions false -> true
            if !pty_dirty_clone.swap(true, Ordering::AcqRel) {
                let _ = proxy.send_event(PtyEvent::NewOutput);
            }
        },
    )?);

    // 6. Initialize egui context & egui-wgpu renderer for Theia's Prism overlay
    let egui_ctx = egui::Context::default();
    egui_extras::install_image_loaders(&egui_ctx);
    egui_ctx.set_pixels_per_point(scale_factor);
    let mut egui_renderer =
        egui_wgpu::Renderer::new(&renderer.device, surface_format, None, 1, false);
    let mut raw_input = egui::RawInput::default();
    let mut last_cursor_pos = egui::Pos2::ZERO;
    let start_time = std::time::Instant::now();

    let mut state = WellTerminalState::new(Arc::clone(&pty_session));
    let mut modifiers_state = Modifiers::default();
    let mut scroll_pixel_accumulator: f32 = 0.0;
    let mut scroll_line_accumulator: f32 = 0.0;
    let mut last_blink_reset = std::time::Instant::now();
    let mut current_font_size = state.channel.read_state().font_size;
    let mut current_line_height = state.channel.read_state().line_height;
    let mut next_repaint_delay = std::time::Duration::from_millis(530);
    let mut cells: Vec<CellInstance> = Vec::with_capacity((grid_rows * grid_cols) as usize);
    let mut terminal_selection: Option<SelectionRange> = None;
    let mut is_selecting: bool = false;
    let mut mouse_dragged: bool = false;
    let mut is_dragging_window: bool = false;
    let mut drag_offset_x: i32 = 0;
    let mut drag_offset_y: i32 = 0;
    let mut context_menu_pos: Option<egui::Pos2> = None;
    let mut last_click_time = std::time::Instant::now();
    let mut last_click_pos = egui::Pos2::ZERO;
    let mut click_count: u8 = 0;
    let mut is_super_down = false;
    let mut is_ctrl_down = false;
    let mut is_shift_down = false;
    let mut is_alt_down = false;
    let mut is_dragging_scrollbar = false;
    let mut hud_notification: Option<(String, std::time::Instant)> = None;
    let mut search_active = false;
    let mut search_query = String::new();
    let mut search_matches: Vec<usize> = Vec::new();
    let mut search_match_idx: usize = 0;
    let mut palette_active = false;
    let mut palette_query = String::new();
    let mut palette_selected_idx: usize = 0;
    let mut hovered_hyperlink: Option<String> = None;
    let mut last_channel_seq = state.channel.sequence();

    let (app_tx, mut app_rx) = tokio::sync::mpsc::channel::<well_ipc::HermesAppCommand>(32);
    let socket_path = std::path::PathBuf::from("/tmp/well-caduceus.sock");
    tokio::spawn(async move {
        well_ipc::rpc::CaduceusServer::start(&socket_path, app_tx).await;
    });

    window.request_redraw();

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        match event {
            Event::UserEvent(PtyEvent::NewOutput) => {
                window.request_redraw();
            }
            Event::WindowEvent {
                window_id,
                event: window_event,
            } if window_id == window.id() => {
                match window_event {
                    WindowEvent::CloseRequested => {
                        println!("Window close requested, exiting cleanly.");
                        elwt.exit();
                    }
                    WindowEvent::Resized(new_size) => {
                        if new_size.width == 0 || new_size.height == 0 {
                            return;
                        }
                        surface_config.width = new_size.width;
                        surface_config.height = new_size.height;
                        surface.configure(&renderer.device, &surface_config);
                        let top_bar_h = well_config::TOP_BAR_HEIGHT * scale_factor;
                        let margin = well_config::FRAME_MARGIN * scale_factor;
                        let usable_w = (new_size.width as f32 - 2.0 * margin).max(cell_width);
                        let usable_h = (new_size.height as f32 - top_bar_h - margin).max(cell_height);
                        let new_cols = (usable_w / cell_width).floor() as u32;
                        let new_rows = (usable_h / cell_height).floor() as u32;
                        if new_cols != grid_cols || new_rows != grid_rows {
                            grid_cols = new_cols;
                            grid_rows = new_rows;
                            let _ = pty_session.resize(grid_rows.max(1) as u16, grid_cols.max(1) as u16);
                        }
                        renderer.set_offsets(margin, top_bar_h, new_size.width as f32, new_size.height as f32);
                        renderer.set_dimensions(
                            new_size.width as f32,
                            new_size.height as f32,
                            grid_cols.max(1),
                            grid_rows.max(1),
                        );
                        is_dragging_window = false;
                        is_selecting = false;
                        // Reset any stuck pointer state in egui during window resize
                        raw_input.events.push(egui::Event::PointerButton {
                            pos: last_cursor_pos,
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            modifiers: egui_modifiers_from_winit(&modifiers_state),
                        });
                        window.request_redraw();
                    }
                    WindowEvent::ScaleFactorChanged {
                        scale_factor: new_scale,
                        ..
                    } => {
                        scale_factor = new_scale as f32;
                        egui_ctx.set_pixels_per_point(scale_factor);
                        let new_size = window.inner_size();
                        if new_size.width == 0 || new_size.height == 0 {
                            return;
                        }
                        surface_config.width = new_size.width;
                        surface_config.height = new_size.height;
                        surface.configure(&renderer.device, &surface_config);
                        cell_width = (current_font_size * 0.65 * scale_factor).max(6.0);
                        cell_height =
                            (current_font_size * current_line_height * scale_factor).max(10.0);
                        let top_bar_h = well_config::TOP_BAR_HEIGHT * scale_factor;
                        let margin = well_config::FRAME_MARGIN * scale_factor;
                        let usable_w = (new_size.width as f32 - 2.0 * margin).max(cell_width);
                        let usable_h = (new_size.height as f32 - top_bar_h - margin).max(cell_height);
                        grid_cols = (usable_w / cell_width).floor() as u32;
                        grid_rows = (usable_h / cell_height).floor() as u32;
                        let _ =
                            pty_session.resize(grid_rows.max(1) as u16, grid_cols.max(1) as u16);
                        renderer.set_offsets(margin, top_bar_h, new_size.width as f32, new_size.height as f32);
                        renderer.set_dimensions(
                            new_size.width as f32,
                            new_size.height as f32,
                            grid_cols.max(1),
                            grid_rows.max(1),
                        );
                        window.request_redraw();
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        let pos = egui::pos2(
                            position.x as f32 / scale_factor,
                            position.y as f32 / scale_factor,
                        );
                        last_cursor_pos = pos;

                        let screen_w = surface_config.width as f32 / scale_factor;
                        let screen_h = surface_config.height as f32 / scale_factor;

                        // Continuous smooth native window dragging
                        if is_dragging_window {
                            let (global_x, global_y) = get_global_mouse_location();
                            let new_x = (global_x * scale_factor as f64) as i32 - drag_offset_x;
                            let new_y = (global_y * scale_factor as f64) as i32 - drag_offset_y;
                            window.set_outer_position(winit::dpi::PhysicalPosition::new(new_x, new_y));
                            return;
                        }

                        raw_input.events.push(egui::Event::PointerMoved(pos));

                        if is_dragging_scrollbar {
                            if let Ok(mut parser) = pty_session.parser.lock() {
                                let max = get_max_scrollback(parser.screen_mut());
                                if max > 0 {
                                    let ratio = 1.0 - (pos.y / screen_h.max(1.0)).clamp(0.0, 1.0);
                                    let target_offset = (ratio * max as f32).round() as usize;
                                    parser.screen_mut().set_scrollback(target_offset);
                                    window.request_redraw();
                                }
                            }
                        } else if is_selecting {
                            mouse_dragged = true;
                            let top_bar_h = well_config::TOP_BAR_HEIGHT * scale_factor;
                            let margin = well_config::FRAME_MARGIN * scale_factor;
                            let col = (((pos.x * scale_factor - margin) / cell_width).floor() as i32)
                                .clamp(0, grid_cols.saturating_sub(1) as i32) as u16;
                            let row = (((pos.y * scale_factor - top_bar_h) / cell_height).floor() as i32)
                                .clamp(0, grid_rows.saturating_sub(1) as i32) as u16;
                            if let Some(sel) = &mut terminal_selection {
                                if sel.end_col != col || sel.end_row != row {
                                    sel.end_col = col;
                                    sel.end_row = row;
                                    window.request_redraw();
                                }
                            }
                        }

                        let in_top_bar = pos.y < well_config::TOP_BAR_HEIGHT;
                        let on_scrollbar = pos.x >= (screen_w - 18.0);
                        let near_left = pos.x < 6.0;
                        let near_right = pos.x > (screen_w - 6.0);
                        let near_top = pos.y < 4.0;
                        let near_bottom = pos.y > (screen_h - 6.0);
                        let on_resize_border = near_left || near_right || near_top || near_bottom;

                        if state.config_panel.is_open
                            || state.config_panel.show_pythia_hud
                            || search_active
                            || palette_active
                            || context_menu_pos.is_some()
                            || is_dragging_scrollbar
                            || in_top_bar
                        {
                            window.request_redraw();
                        }

                        let in_terminal_content = !in_top_bar
                            && !on_scrollbar
                            && !on_resize_border
                            && !state.config_panel.is_open
                            && !state.config_panel.show_pythia_hud
                            && !search_active
                            && !palette_active
                            && context_menu_pos.is_none();

                        if in_terminal_content {
                            let top_bar_h = well_config::TOP_BAR_HEIGHT;
                            let margin = well_config::FRAME_MARGIN;
                            let cell_w = cell_width / scale_factor;
                            let cell_h = cell_height / scale_factor;
                            if pos.y >= top_bar_h && pos.y < screen_h - margin && pos.x >= margin && pos.x < screen_w - margin {
                                let row = ((pos.y - top_bar_h) / cell_h).floor() as u16;
                                let col = ((pos.x - margin) / cell_w).floor() as usize;

                                if let Ok(parser) = pty_session.parser.lock() {
                                    let line_text = get_line_text_from_screen(parser.screen(), row);
                                    if let Some((start_col, end_col, url)) = detect_url_in_line(&line_text) {
                                        if col >= start_col && col <= end_col {
                                            hovered_hyperlink = Some(url);
                                        } else {
                                            hovered_hyperlink = None;
                                        }
                                    } else {
                                        hovered_hyperlink = None;
                                    }
                                }
                            } else {
                                hovered_hyperlink = None;
                            }
                        } else {
                            hovered_hyperlink = None;
                        }

                        if hovered_hyperlink.is_some() {
                            window.set_cursor_icon(winit::window::CursorIcon::Pointer);
                        } else if in_terminal_content {
                            window.set_cursor_icon(winit::window::CursorIcon::Text);
                        } else {
                            window.set_cursor_icon(winit::window::CursorIcon::Default);
                        }
                    }
                    WindowEvent::MouseInput {
                        state: btn_state,
                        button,
                        ..
                    } => {
                        let pressed = btn_state == ElementState::Pressed;
                        let screen_w = surface_config.width as f32 / scale_factor;
                        let screen_h = surface_config.height as f32 / scale_factor;

                        let near_left = last_cursor_pos.x < 6.0;
                        let near_right = last_cursor_pos.x > (screen_w - 6.0);
                        let near_top = last_cursor_pos.y < 4.0;
                        let near_bottom = last_cursor_pos.y > (screen_h - 6.0);
                        let on_resize_border = near_left || near_right || near_top || near_bottom;

                        // 1. Smooth Native Window Dragging on Top Bar (excludes edge resize borders)
                        if button == MouseButton::Left {
                            if pressed {
                                if last_cursor_pos.y >= 4.0
                                    && last_cursor_pos.y < well_config::TOP_BAR_HEIGHT
                                    && !near_left
                                    && !near_right
                                {
                                    let in_center = (last_cursor_pos.x - screen_w * 0.5).abs() < 50.0;
                                    let in_right = last_cursor_pos.x > (screen_w - 140.0);
                                    let in_left = last_cursor_pos.x < 115.0;

                                    if !in_center && !in_right && !in_left {
                                        let (global_x, global_y) = get_global_mouse_location();
                                        if let Ok(win_pos) = window.outer_position() {
                                            is_dragging_window = true;
                                            drag_offset_x = (global_x * scale_factor as f64) as i32 - win_pos.x;
                                            drag_offset_y = (global_y * scale_factor as f64) as i32 - win_pos.y;
                                        }
                                        return;
                                    }
                                }
                            } else {
                                is_dragging_window = false;
                            }
                        }

                        // When user clicks the resize border, let macOS handle window resizing without interference
                        if on_resize_border && pressed {
                            return;
                        }

                        let btn = match button {
                            MouseButton::Left => egui::PointerButton::Primary,
                            MouseButton::Right => egui::PointerButton::Secondary,
                            MouseButton::Middle => egui::PointerButton::Middle,
                            _ => egui::PointerButton::Primary,
                        };
                        raw_input.events.push(egui::Event::PointerButton {
                            pos: last_cursor_pos,
                            button: btn,
                            pressed,
                            modifiers: egui_modifiers_from_winit(&modifiers_state),
                        });

                        let in_theia = (state.config_panel.is_open || state.config_panel.show_pythia_hud) && egui_ctx.wants_pointer_input();

                        if !in_theia {
                            let (_mac_super, _, _, _) = query_macos_modifiers();
                            let is_right_click = button == MouseButton::Right
                                || (button == MouseButton::Left && (modifiers_state.state().control_key() || is_ctrl_down));

                            if is_right_click {
                                if pressed && last_cursor_pos.y >= well_config::TOP_BAR_HEIGHT {
                                    context_menu_pos = Some(last_cursor_pos);
                                    window.request_redraw();
                                }
                                return;
                            }

                            // Middle-click to paste clipboard directly into terminal (Unix/X11 & modern terminal ergonomics)
                            if button == MouseButton::Middle && pressed {
                                if let Some(text) = get_from_clipboard() {
                                    paste_into_terminal(&pty_session, &text);
                                    hud_notification = Some((
                                        format!("📋 Pasted {} chars", text.chars().count()),
                                        std::time::Instant::now(),
                                    ));
                                    window.request_redraw();
                                }
                                return;
                            }

                            // Check if click intersects active context menu
                            let menu_w = 190.0_f32;
                            let menu_h = 140.0_f32;
                            let in_context_menu = if let Some(mpos) = context_menu_pos {
                                let clamped_x = mpos.x.min(screen_w - menu_w - 10.0).max(10.0);
                                let clamped_y = mpos.y.min(screen_h - menu_h - 10.0).max(10.0);
                                let menu_rect = egui::Rect::from_min_size(
                                    egui::pos2(clamped_x, clamped_y),
                                    egui::vec2(menu_w, menu_h),
                                );
                                menu_rect.contains(last_cursor_pos)
                            } else {
                                false
                            };

                            if in_context_menu {
                                // User clicked inside the context menu; allow egui to handle button click
                                window.request_redraw();
                                return;
                            } else if context_menu_pos.is_some() && pressed {
                                // Clicked outside context menu: dismiss it
                                context_menu_pos = None;
                                window.request_redraw();
                            }

                            let on_scrollbar_track = last_cursor_pos.x >= (screen_w - 18.0);

                            if button == MouseButton::Left {
                                if pressed {
                                    if last_cursor_pos.y < well_config::TOP_BAR_HEIGHT || on_resize_border {
                                        return;
                                    }

                                    if on_scrollbar_track {
                                        if let Ok(mut parser) = pty_session.parser.lock() {
                                            let max = get_max_scrollback(parser.screen_mut());
                                            if max > 0 {
                                                is_dragging_scrollbar = true;
                                                let ratio = 1.0 - (last_cursor_pos.y / screen_h.max(1.0)).clamp(0.0, 1.0);
                                                let target_offset = (ratio * max as f32).round() as usize;
                                                parser.screen_mut().set_scrollback(target_offset);
                                                window.request_redraw();
                                                return;
                                            }
                                        }
                                    }

                                    let top_bar_h = well_config::TOP_BAR_HEIGHT * scale_factor;
                                    let margin = well_config::FRAME_MARGIN * scale_factor;
                                    let col = (((last_cursor_pos.x * scale_factor - margin) / cell_width).floor() as i32)
                                        .clamp(0, grid_cols.saturating_sub(1) as i32) as u16;
                                    let row = (((last_cursor_pos.y * scale_factor - top_bar_h) / cell_height).floor() as i32)
                                        .clamp(0, grid_rows.saturating_sub(1) as i32) as u16;

                                    let now = std::time::Instant::now();
                                    let is_quick_click = now.duration_since(last_click_time).as_millis() < 450
                                        && (last_cursor_pos.x - last_click_pos.x).abs() < 8.0
                                        && (last_cursor_pos.y - last_click_pos.y).abs() < 8.0;

                                    if is_quick_click {
                                        click_count = (click_count + 1).min(3);
                                    } else {
                                        click_count = 1;
                                    }
                                    last_click_time = now;
                                    last_click_pos = last_cursor_pos;

                                    if click_count == 3 {
                                        // Triple-click: Select entire line!
                                        let line_sel = SelectionRange::new(
                                            0,
                                            row,
                                            grid_cols.saturating_sub(1) as u16,
                                            row,
                                        );
                                        terminal_selection = Some(line_sel);
                                        is_selecting = false;
                                        mouse_dragged = false;
                                        context_menu_pos = None;
                                        if let Ok(parser) = pty_session.parser.lock() {
                                            let text = well_render::extract_text_from_screen(parser.screen(), line_sel);
                                            if !text.is_empty() {
                                                copy_to_clipboard(&text);
                                                hud_notification = Some((
                                                    format!("📋 Copied line ({} chars)", text.chars().count()),
                                                    std::time::Instant::now(),
                                                ));
                                            }
                                        }
                                        window.request_redraw();
                                        return;
                                    } else if click_count == 2 {
                                        // Double-click: Select word!
                                        if let Ok(parser) = pty_session.parser.lock() {
                                            if let Some(word_sel) = well_render::find_word_bounds(parser.screen(), col, row) {
                                                terminal_selection = Some(word_sel);
                                                is_selecting = false;
                                                mouse_dragged = false;
                                                context_menu_pos = None;
                                                let text = well_render::extract_text_from_screen(parser.screen(), word_sel);
                                                if !text.is_empty() {
                                                    copy_to_clipboard(&text);
                                                    hud_notification = Some((
                                                        format!("📋 Copied \"{}\"", text),
                                                        std::time::Instant::now(),
                                                    ));
                                                }
                                                window.request_redraw();
                                                return;
                                            }
                                        }
                                    }

                                    terminal_selection = Some(SelectionRange::new(col, row, col, row));
                                    is_selecting = true;
                                    mouse_dragged = false;
                                    window.request_redraw();
                                } else {
                                    is_dragging_scrollbar = false;
                                    is_selecting = false;
                                    if !mouse_dragged {
                                        let is_recent_multi = click_count >= 2 && last_click_time.elapsed().as_millis() < 450;
                                        if !is_recent_multi {
                                            terminal_selection = None;
                                            window.request_redraw();
                                        }
                                    } else if let Some(sel) = terminal_selection {
                                        // Auto-copy on select completion!
                                        if let Ok(parser) = pty_session.parser.lock() {
                                            let text = well_render::extract_text_from_screen(parser.screen(), sel);
                                            if !text.is_empty() {
                                                copy_to_clipboard(&text);
                                                hud_notification = Some((
                                                    format!("📋 Copied {} chars", text.chars().count()),
                                                    std::time::Instant::now(),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        window.request_redraw();
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        last_blink_reset = std::time::Instant::now();
                        let (egui_unit, egui_delta) = match delta {
                            MouseScrollDelta::LineDelta(x, y) => {
                                (egui::MouseWheelUnit::Line, egui::vec2(x, y))
                            }
                            MouseScrollDelta::PixelDelta(pos) => (
                                egui::MouseWheelUnit::Point,
                                egui::vec2(pos.x as f32, pos.y as f32) / scale_factor,
                            ),
                        };

                        if (state.config_panel.is_open || state.config_panel.show_pythia_hud) && egui_ctx.wants_pointer_input() {
                            raw_input.events.push(egui::Event::MouseWheel {
                                unit: egui_unit,
                                delta: egui_delta,
                                modifiers: egui_modifiers_from_winit(&modifiers_state),
                            });
                            window.request_redraw();
                        } else {
                            // Calculate discrete lines to scroll in terminal
                            let lines = match delta {
                                MouseScrollDelta::LineDelta(_x, y) => {
                                    scroll_line_accumulator += y * 3.0;
                                    if scroll_line_accumulator.abs() >= 1.0 {
                                        let c = scroll_line_accumulator as i32;
                                        scroll_line_accumulator -= c as f32;
                                        c
                                    } else if y != 0.0 {
                                        // Guarantee that a discrete mouse wheel notch always scrolls at least 1 line
                                        let c = if y > 0.0 { 1 } else { -1 };
                                        scroll_line_accumulator = 0.0;
                                        c
                                    } else {
                                        0
                                    }
                                }
                                MouseScrollDelta::PixelDelta(pos) => {
                                    scroll_pixel_accumulator += pos.y as f32;
                                    // Highly responsive scrolling step for high-DPI trackpads / smooth wheels
                                    let step = (cell_height / 2.5).clamp(8.0, 18.0);
                                    let mut count = (scroll_pixel_accumulator / step) as i32;
                                    if count != 0 {
                                        scroll_pixel_accumulator -= count as f32 * step;
                                    } else if pos.y.abs() >= 1.0 && scroll_pixel_accumulator.abs() >= (step * 0.5) {
                                        count = if scroll_pixel_accumulator > 0.0 { 1 } else { -1 };
                                        scroll_pixel_accumulator = 0.0;
                                    }
                                    count
                                }
                            };

                            if lines != 0 {
                                if let Ok(mut parser) = pty_session.parser.lock() {
                                    let mouse_mode = parser.screen().mouse_protocol_mode();
                                    let mouse_encoding = parser.screen().mouse_protocol_encoding();
                                    let is_alt_screen = parser.screen().alternate_screen();

                                    if mouse_mode != vt100::MouseProtocolMode::None {
                                        // Application mouse reporting mode (vim, htop, tmux, lazygit)
                                        let col = ((last_cursor_pos.x * scale_factor / cell_width)
                                            .floor()
                                            as u16
                                            + 1)
                                        .min(grid_cols.max(1) as u16);
                                        let row = ((last_cursor_pos.y * scale_factor / cell_height)
                                            .floor()
                                            as u16
                                            + 1)
                                        .min(grid_rows.max(1) as u16);
                                        let btn = if lines > 0 { 64 } else { 65 }; // 64 = Wheel Up, 65 = Wheel Down
                                        let count = lines.abs();
                                        let mut bytes = Vec::new();

                                        for _ in 0..count {
                                            match mouse_encoding {
                                                vt100::MouseProtocolEncoding::Sgr => {
                                                    let seq =
                                                        format!("\x1b[<{};{};{}M", btn, col, row);
                                                    bytes.extend_from_slice(seq.as_bytes());
                                                }
                                                vt100::MouseProtocolEncoding::Default
                                                | vt100::MouseProtocolEncoding::Utf8 => {
                                                    let b = 32u8.saturating_add(btn);
                                                    let c = 32u8.saturating_add(col.min(223) as u8);
                                                    let r = 32u8.saturating_add(row.min(223) as u8);
                                                    bytes.extend_from_slice(&[
                                                        0x1b, b'[', b'M', b, c, r,
                                                    ]);
                                                }
                                            }
                                        }
                                        drop(parser);
                                        let _ = pty_session.write_all(&bytes);
                                        window.request_redraw();
                                    } else if is_alt_screen {
                                        // Alternate scroll mode (less, man, git diff, info)
                                        let count = lines.abs();
                                        let seq = if lines > 0 { b"\x1b[A" } else { b"\x1b[B" };
                                        let mut bytes =
                                            Vec::with_capacity(count as usize * seq.len());
                                        for _ in 0..count {
                                            bytes.extend_from_slice(seq);
                                        }
                                        drop(parser);
                                        let _ = pty_session.write_all(&bytes);
                                        window.request_redraw();
                                    } else {
                                        // Standard terminal scrollback buffer
                                        let cur = parser.screen().scrollback();
                                        if lines > 0 {
                                            parser
                                                .screen_mut()
                                                .set_scrollback(cur.saturating_add(lines as usize));
                                        } else {
                                            parser.screen_mut().set_scrollback(
                                                cur.saturating_sub((-lines) as usize),
                                            );
                                        }
                                        window.request_redraw();
                                    }
                                }
                            }
                        }
                    }
                    WindowEvent::ModifiersChanged(new_mods) => {
                        modifiers_state = new_mods;
                    }
                    WindowEvent::Focused(focused) => {
                        if !focused {
                            is_dragging_window = false;
                            is_selecting = false;
                            modifiers_state = Modifiers::default();
                            // Reset stuck mouse button states in egui on focus lost
                            raw_input.events.push(egui::Event::PointerButton {
                                pos: last_cursor_pos,
                                button: egui::PointerButton::Primary,
                                pressed: false,
                                modifiers: Default::default(),
                            });
                        }
                        window.request_redraw();
                    }
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                state: key_state,
                                logical_key,
                                text,
                                physical_key,
                                ..
                            },
                        ..
                    } => {
                        let pressed = key_state == ElementState::Pressed;
                        match physical_key {
                            PhysicalKey::Code(KeyCode::SuperLeft | KeyCode::SuperRight) => {
                                is_super_down = pressed;
                            }
                            PhysicalKey::Code(KeyCode::ControlLeft | KeyCode::ControlRight) => {
                                is_ctrl_down = pressed;
                            }
                            PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight) => {
                                is_shift_down = pressed;
                            }
                            PhysicalKey::Code(KeyCode::AltLeft | KeyCode::AltRight) => {
                                is_alt_down = pressed;
                            }
                            _ => {}
                        }

                        if !pressed {
                            return;
                        }

                        let (mac_super, mac_ctrl, mac_shift, mac_alt) = query_macos_modifiers();
                        let is_super = modifiers_state.state().super_key() || is_super_down || mac_super;
                        let is_ctrl = modifiers_state.state().control_key() || is_ctrl_down || mac_ctrl;
                        let is_shift = modifiers_state.state().shift_key() || is_shift_down || mac_shift;
                        let is_alt = modifiers_state.state().alt_key() || is_alt_down || mac_alt;

                        let is_key_c = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyC))
                            || matches!(&logical_key, Key::Character(c) if c.eq_ignore_ascii_case("c"));
                        let is_key_v = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyV))
                            || matches!(&logical_key, Key::Character(c) if c.eq_ignore_ascii_case("v"));
                        let is_key_a = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyA))
                            || matches!(&logical_key, Key::Character(c) if c.eq_ignore_ascii_case("a"));
                        let is_key_f = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyF))
                            || matches!(&logical_key, Key::Character(c) if c.eq_ignore_ascii_case("f"));
                        let is_key_p = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyP))
                            || matches!(&logical_key, Key::Character(c) if c.eq_ignore_ascii_case("p"));
                        let is_key_k = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyK))
                            || matches!(&logical_key, Key::Character(c) if c.eq_ignore_ascii_case("k"));

                        let is_key_t = matches!(physical_key, PhysicalKey::Code(KeyCode::KeyT))
                            || matches!(&logical_key, Key::Character(c) if c.eq_ignore_ascii_case("t"));

                        // Toggle VCR Mode
                        if is_ctrl && is_shift && is_key_t {
                            state.is_vcr_mode = !state.is_vcr_mode;
                            println!("VCR Mode: {}", state.is_vcr_mode);
                            window.request_redraw();
                            return;
                        }

                        // Check Cmd+, or Ctrl+, or F12 / F1 settings drawer toggle
                        if let Key::Named(NamedKey::F5) = logical_key {
                            run_go_demo();
                            return;
                        }
                        let toggle_drawer = match &logical_key {
                            Key::Character(c)
                                if (c == "," || c == "<") && (is_super || is_ctrl) =>
                            {
                                true
                            }
                            Key::Named(NamedKey::F12) | Key::Named(NamedKey::F1) => true,
                            _ => false,
                        };

                        if toggle_drawer {
                            state.config_panel.is_open = !state.config_panel.is_open;
                            println!(
                                "Theia's Prism drawer toggled: is_open = {}",
                                state.config_panel.is_open
                            );
                            window.request_redraw();
                            return;
                        }

                        // Cmd+I / Ctrl+I toggles Pythia LLM Translation HUD
                        let toggle_pythia = match &logical_key {
                            Key::Character(c) if (c == "i" || c == "I") && (is_super || is_ctrl) => true,
                            _ => false,
                        };

                        if toggle_pythia {
                            state.config_panel.show_pythia_hud = !state.config_panel.show_pythia_hud;
                            window.request_redraw();
                            return;
                        }

                        // Toggle Command Palette on Cmd+Shift+P / Ctrl+Shift+P or Cmd+P
                        let toggle_palette = (is_super && is_shift && is_key_p)
                            || (is_ctrl && is_shift && is_key_p)
                            || (is_super && is_key_p && !is_shift && !palette_active);
                        if toggle_palette {
                            palette_active = !palette_active;
                            if palette_active {
                                search_active = false;
                                palette_query.clear();
                                palette_selected_idx = 0;
                            }
                            window.request_redraw();
                            return;
                        }

                        // Toggle Scrollback Search on Cmd+F / Ctrl+F
                        let toggle_search = (is_super && is_key_f) || (is_ctrl && is_key_f);
                        if toggle_search {
                            search_active = !search_active;
                            if search_active {
                                palette_active = false;
                                search_matches.clear();
                                search_match_idx = 0;
                            }
                            window.request_redraw();
                            return;
                        }

                        // Clear Terminal Buffer on Cmd+K
                        if is_super && is_key_k {
                            let _ = pty_session.write_all(b"\x1b[2J\x1b[H\x1b[3J");
                            if let Ok(mut parser) = pty_session.parser.lock() {
                                parser.screen_mut().set_scrollback(0);
                            }
                            terminal_selection = None;
                            hud_notification = Some(("✨ Buffer Cleared".to_string(), std::time::Instant::now()));
                            window.request_redraw();
                            return;
                        }

                        // Increase Font Size on Cmd+= / Cmd++
                        let font_inc = is_super && (physical_key == PhysicalKey::Code(KeyCode::Equal) || matches!(&logical_key, Key::Character(c) if c == "=" || c == "+"));
                        if font_inc {
                            let mut cfg = state.channel.read_state();
                            cfg.font_size = (cfg.font_size + 1.0).min(32.0);
                            state.channel.sync_state(cfg.clone());
                            hud_notification = Some((format!("🔍 Font Size: {:.1} pt", cfg.font_size), std::time::Instant::now()));
                            window.request_redraw();
                            return;
                        }

                        // Decrease Font Size on Cmd+-
                        let font_dec = is_super && (physical_key == PhysicalKey::Code(KeyCode::Minus) || matches!(&logical_key, Key::Character(c) if c == "-"));
                        if font_dec {
                            let mut cfg = state.channel.read_state();
                            cfg.font_size = (cfg.font_size - 1.0).max(8.0);
                            state.channel.sync_state(cfg.clone());
                            hud_notification = Some((format!("🔍 Font Size: {:.1} pt", cfg.font_size), std::time::Instant::now()));
                            window.request_redraw();
                            return;
                        }

                        // Reset Font Size on Cmd+0
                        let font_reset = is_super && (physical_key == PhysicalKey::Code(KeyCode::Digit0) || matches!(&logical_key, Key::Character(c) if c == "0"));
                        if font_reset {
                            let mut cfg = state.channel.read_state();
                            cfg.font_size = 14.0;
                            state.channel.sync_state(cfg.clone());
                            hud_notification = Some(("↺ Font Reset: 14.0 pt".to_string(), std::time::Instant::now()));
                            window.request_redraw();
                            return;
                        }

                        // Escape key closes Theia, Pythia HUD, Search, or Command Palette
                        if matches!(logical_key, Key::Named(NamedKey::Escape)) {
                            let mut handled = false;
                            if state.config_panel.is_open || state.config_panel.show_pythia_hud {
                                state.config_panel.is_open = false;
                                state.config_panel.show_pythia_hud = false;
                                handled = true;
                            }
                            if search_active {
                                search_active = false;
                                handled = true;
                            }
                            if palette_active {
                                palette_active = false;
                                handled = true;
                            }
                            if context_menu_pos.is_some() {
                                context_menu_pos = None;
                                handled = true;
                            }
                            if terminal_selection.is_some() {
                                terminal_selection = None;
                                handled = true;
                            }
                            if handled {
                                window.request_redraw();
                                return;
                            }
                        }

                        last_blink_reset = std::time::Instant::now();

                        // If config drawer, Pythia HUD, Search, or Palette is open, route all keyboard inputs to egui
                        if state.config_panel.is_open || state.config_panel.show_pythia_hud || search_active || palette_active {
                            if let Some(txt) = &text {
                                if !is_super && !is_ctrl {
                                    raw_input.events.push(egui::Event::Text(txt.to_string()));
                                }
                            }

                            let egui_key = match &logical_key {
                                Key::Named(NamedKey::Backspace) => Some(egui::Key::Backspace),
                                Key::Named(NamedKey::Enter) => Some(egui::Key::Enter),
                                Key::Named(NamedKey::Tab) => Some(egui::Key::Tab),
                                Key::Named(NamedKey::Delete) => Some(egui::Key::Delete),
                                Key::Named(NamedKey::ArrowLeft) => Some(egui::Key::ArrowLeft),
                                Key::Named(NamedKey::ArrowRight) => Some(egui::Key::ArrowRight),
                                Key::Named(NamedKey::ArrowUp) => Some(egui::Key::ArrowUp),
                                Key::Named(NamedKey::ArrowDown) => Some(egui::Key::ArrowDown),
                                Key::Named(NamedKey::Home) => Some(egui::Key::Home),
                                Key::Named(NamedKey::End) => Some(egui::Key::End),
                                Key::Named(NamedKey::PageUp) => Some(egui::Key::PageUp),
                                Key::Named(NamedKey::PageDown) => Some(egui::Key::PageDown),
                                Key::Character(c) => match c.to_ascii_lowercase().as_str() {
                                    "a" if is_super || is_ctrl => Some(egui::Key::A),
                                    "c" if is_super || is_ctrl => Some(egui::Key::C),
                                    "v" if is_super || is_ctrl => Some(egui::Key::V),
                                    "x" if is_super || is_ctrl => Some(egui::Key::X),
                                    "z" if is_super || is_ctrl => Some(egui::Key::Z),
                                    _ => None,
                                },
                                _ => None,
                            };

                            if let Some(k) = egui_key {
                                raw_input.events.push(egui::Event::Key {
                                    key: k,
                                    physical_key: None,
                                    pressed: true,
                                    repeat: false,
                                    modifiers: egui_modifiers_from_winit(&modifiers_state),
                                });
                            }

                            if (is_super || is_ctrl) && is_key_v {
                                if let Some(txt) = get_from_clipboard() {
                                    raw_input.events.push(egui::Event::Paste(txt));
                                }
                            }

                            window.request_redraw();
                            return;
                        }

                        // Escape closes context menu or clears active terminal selection
                        if matches!(logical_key, Key::Named(NamedKey::Escape)) {
                            let mut handled = false;
                            if context_menu_pos.is_some() {
                                context_menu_pos = None;
                                handled = true;
                            }
                            if terminal_selection.is_some() {
                                terminal_selection = None;
                                handled = true;
                            }
                            if handled {
                                window.request_redraw();
                                return;
                            }
                        }

                        // Terminal Copy Shortcut (Cmd+C on macOS, Ctrl+Shift+C on Linux/Windows)
                        let is_copy = (is_super && is_key_c) || (is_ctrl && is_shift && is_key_c);
                        if is_copy {
                            if let Some(sel) = terminal_selection {
                                if let Ok(parser) = pty_session.parser.lock() {
                                    let text = well_render::extract_text_from_screen(parser.screen(), sel);
                                    if !text.is_empty() {
                                        copy_to_clipboard(&text);
                                        hud_notification = Some((
                                            format!("📋 Copied {} chars", text.chars().count()),
                                            std::time::Instant::now(),
                                        ));
                                    }
                                }
                            }
                            window.request_redraw();
                            return;
                        }

                        // Terminal Paste Shortcut (Cmd+V on macOS, Ctrl+Shift+V on Linux/Windows, or Ctrl+V on Windows/Linux)
                        let is_paste = (is_super && is_key_v)
                            || (is_ctrl && is_shift && is_key_v)
                            || (!cfg!(target_os = "macos") && is_ctrl && is_key_v);
                        if is_paste {
                            if let Some(text) = get_from_clipboard() {
                                paste_into_terminal(&pty_session, &text);
                                hud_notification = Some((
                                    format!("📋 Pasted {} chars", text.chars().count()),
                                    std::time::Instant::now(),
                                ));
                                window.request_redraw();
                            }
                            return;
                        }

                        // Terminal Select All Shortcut (Cmd+A on macOS, Ctrl+Shift+A on Linux/Windows)
                        let is_select_all = (is_super && is_key_a) || (is_ctrl && is_shift && is_key_a);
                        if is_select_all {
                            terminal_selection = Some(SelectionRange::new(
                                0,
                                0,
                                grid_cols.saturating_sub(1) as u16,
                                grid_rows.saturating_sub(1) as u16,
                            ));
                            window.request_redraw();
                            return;
                        }

                        // Terminal Scrollback Shortcuts (Shift+PgUp, Shift+PgDn, Cmd+Home, Cmd+End, Cmd+Up, Cmd+Down, Shift+Up, Shift+Down)
                        if !state.config_panel.is_open && !state.config_panel.show_pythia_hud {
                            let mut handled_scroll = false;
                            if let Ok(mut parser) = pty_session.parser.lock() {
                                let cur = parser.screen().scrollback();
                                let page_step = (grid_rows.saturating_sub(2)).max(1) as usize;

                                match &logical_key {
                                    Key::Named(NamedKey::PageUp) => {
                                        let step = if is_super { page_step * 2 } else { page_step };
                                        parser
                                            .screen_mut()
                                            .set_scrollback(cur.saturating_add(step));
                                        handled_scroll = true;
                                    }
                                    Key::Named(NamedKey::PageDown) => {
                                        let step = if is_super { page_step * 2 } else { page_step };
                                        parser
                                            .screen_mut()
                                            .set_scrollback(cur.saturating_sub(step));
                                        handled_scroll = true;
                                    }
                                    Key::Named(NamedKey::Home) if is_super || is_shift => {
                                        parser.screen_mut().set_scrollback(usize::MAX);
                                        handled_scroll = true;
                                    }
                                    Key::Named(NamedKey::End) if is_super || is_shift => {
                                        parser.screen_mut().set_scrollback(0);
                                        handled_scroll = true;
                                    }
                                    Key::Named(NamedKey::ArrowUp) if is_super || is_shift => {
                                        let step = if is_super { page_step } else { 3 };
                                        parser.screen_mut().set_scrollback(cur.saturating_add(step));
                                        handled_scroll = true;
                                    }
                                    Key::Named(NamedKey::ArrowDown) if is_super || is_shift => {
                                        let step = if is_super { page_step } else { 3 };
                                        parser.screen_mut().set_scrollback(cur.saturating_sub(step));
                                        handled_scroll = true;
                                    }
                                    _ => {
                                        if is_super || is_shift {
                                            match physical_key {
                                                PhysicalKey::Code(KeyCode::ArrowUp) => {
                                                    let step = if is_super { page_step } else { 3 };
                                                    parser.screen_mut().set_scrollback(cur.saturating_add(step));
                                                    handled_scroll = true;
                                                }
                                                PhysicalKey::Code(KeyCode::ArrowDown) => {
                                                    let step = if is_super { page_step } else { 3 };
                                                    parser.screen_mut().set_scrollback(cur.saturating_sub(step));
                                                    handled_scroll = true;
                                                }
                                                PhysicalKey::Code(KeyCode::PageUp) => {
                                                    let step = if is_super { page_step * 2 } else { page_step };
                                                    parser.screen_mut().set_scrollback(cur.saturating_add(step));
                                                    handled_scroll = true;
                                                }
                                                PhysicalKey::Code(KeyCode::PageDown) => {
                                                    let step = if is_super { page_step * 2 } else { page_step };
                                                    parser.screen_mut().set_scrollback(cur.saturating_sub(step));
                                                    handled_scroll = true;
                                                }
                                                PhysicalKey::Code(KeyCode::Home) => {
                                                    parser.screen_mut().set_scrollback(usize::MAX);
                                                    handled_scroll = true;
                                                }
                                                PhysicalKey::Code(KeyCode::End) => {
                                                    parser.screen_mut().set_scrollback(0);
                                                    handled_scroll = true;
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                            if handled_scroll {
                                window.request_redraw();
                                return;
                            }
                        }

                        // Otherwise route to interactive PTY session
                        let kitty_seq = if state.channel.read_state().enable_kitty_keyboard {
                            let mut kitty_mods = 0;
                            if is_shift { kitty_mods |= 1; }
                            if is_alt { kitty_mods |= 2; }
                            if is_ctrl { kitty_mods |= 4; }
                            if is_super { kitty_mods |= 8; }
                            let modifier_val = 1 + kitty_mods;
                            
                            if modifier_val > 1 {
                                match &logical_key {
                                    Key::Named(NamedKey::ArrowUp) => Some(format!("\x1b[1;{}A", modifier_val).into_bytes()),
                                    Key::Named(NamedKey::ArrowDown) => Some(format!("\x1b[1;{}B", modifier_val).into_bytes()),
                                    Key::Named(NamedKey::ArrowRight) => Some(format!("\x1b[1;{}C", modifier_val).into_bytes()),
                                    Key::Named(NamedKey::ArrowLeft) => Some(format!("\x1b[1;{}D", modifier_val).into_bytes()),
                                    Key::Named(NamedKey::Home) => Some(format!("\x1b[1;{}H", modifier_val).into_bytes()),
                                    Key::Named(NamedKey::End) => Some(format!("\x1b[1;{}F", modifier_val).into_bytes()),
                                    _ => {
                                        if let Some(txt) = &text {
                                            if let Some(ch) = txt.chars().next() {
                                                let codepoint = ch as u32;
                                                Some(format!("\x1b[{};{}u", codepoint, modifier_val).into_bytes())
                                            } else { None }
                                        } else { None }
                                    }
                                }
                            } else { None }
                        } else { None };

                        let input_bytes: Option<Vec<u8>> = kitty_seq.or_else(|| match logical_key {
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
                                                let ctrl_code =
                                                    (ch.to_ascii_lowercase() as u8) - b'a' + 1;
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
                        });

                        // Automatic scroll-to-bottom ONLY when typing actual input characters/commands
                        if let Some(bytes) = input_bytes {
                            if let Ok(mut parser) = pty_session.parser.lock() {
                                if parser.screen().scrollback() > 0 {
                                    parser.screen_mut().set_scrollback(0);
                                    window.request_redraw();
                                }
                            }
                            let _ = pty_session.write_all(&bytes);
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        pty_dirty.store(false, Ordering::Release);

                        let frame = match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                surface.configure(&renderer.device, &surface_config);
                                window.request_redraw();
                                return;
                            }
                            Err(wgpu::SurfaceError::Timeout) => {
                                return;
                            }
                            Err(wgpu::SurfaceError::OutOfMemory) => {
                                elwt.exit();
                                return;
                            }
                        };

                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder = renderer.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("AtlasRenderEncoder"),
                            },
                        );

                        // Read live configuration vector via Hermes Seqlock (atomic zero-lock synchronization)
                        let live_config = state.channel.read_state();

                        // Dynamically update font sizing and line height when changed in Theia Typography menu
                        if (live_config.font_size - current_font_size).abs() > 0.05
                            || (live_config.line_height - current_line_height).abs() > 0.02
                        {
                            current_font_size = live_config.font_size;
                            current_line_height = live_config.line_height;
                            cell_width = (current_font_size * 0.65 * scale_factor).max(6.0);
                            cell_height =
                                (current_font_size * current_line_height * scale_factor).max(10.0);
                            let top_bar_h = well_config::TOP_BAR_HEIGHT * scale_factor;
                            let margin = well_config::FRAME_MARGIN * scale_factor;
                            let usable_w = (surface_config.width as f32 - 2.0 * margin).max(cell_width);
                            let usable_h = (surface_config.height as f32 - top_bar_h - margin).max(cell_height);
                            grid_cols = (usable_w / cell_width).floor() as u32;
                            grid_rows = (usable_h / cell_height).floor() as u32;
                            cells.reserve((grid_rows * grid_cols) as usize);
                            let _ = pty_session
                                .resize(grid_rows.max(1) as u16, grid_cols.max(1) as u16);
                            renderer.set_offsets(margin, top_bar_h, surface_config.width as f32, surface_config.height as f32);
                            renderer.update_cell_metrics(
                                cell_width,
                                cell_height,
                                grid_cols.max(1),
                                grid_rows.max(1),
                                surface_config.width as f32,
                                surface_config.height as f32,
                            );
                        }

                        // Determine cursor blink state (530ms visible, 530ms hidden)
                        let cursor_visible = if live_config.cursor_blink {
                            let elapsed_ms = last_blink_reset.elapsed().as_millis();
                            (elapsed_ms % 1060) < 530
                        } else {
                            true
                        };

                        // Pass 1: Assemble and render text cells from live vt100 virtual screen
                        cells.clear();
                        if let Ok(parser) = pty_session.parser.lock() {
                            renderer.build_cells_from_vt100_ext(
                                parser.screen(),
                                &mut cells,
                                grid_rows,
                                grid_cols,
                                cursor_visible,
                                live_config.cursor_style,
                                live_config.theme_id,
                                0,
                                terminal_selection,
                            );
                        }

                        let op = (live_config.background_opacity as f64).clamp(0.05, 1.0);
                        let clear_color = match live_config.theme_id {
                            1 => wgpu::Color {
                                r: 0.10 * op,
                                g: 0.11 * op,
                                b: 0.15 * op,
                                a: op,
                            }, // Tokyo Night
                            2 => wgpu::Color {
                                r: 0.01 * op,
                                g: 0.06 * op,
                                b: 0.02 * op,
                                a: op,
                            }, // Matrix Green
                            3 => wgpu::Color {
                                r: 0.12 * op,
                                g: 0.04 * op,
                                b: 0.14 * op,
                                a: op,
                            }, // Synthwave '84
                            _ => wgpu::Color {
                                r: 0.02 * op,
                                g: 0.03 * op,
                                b: 0.05 * op,
                                a: op,
                            }, // Cyber-Neon
                        };

                        renderer.draw_frame(&view, &mut encoder, &cells, Some(clear_color));

                        // Pass 2: Overlay egui UI (Persistent Settings HUD button & Control Center)
                        {
                            raw_input.time = Some(start_time.elapsed().as_secs_f64());
                            raw_input.predicted_dt = 1.0 / 60.0;
                            raw_input.screen_rect = Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(
                                    surface_config.width as f32 / scale_factor,
                                    surface_config.height as f32 / scale_factor,
                                ),
                            ));
                            let input = std::mem::take(&mut raw_input);
                            let pty_session_clone = Arc::clone(&state.pty_session);
                            let win_clone = Arc::clone(&window);
                            let full_output = egui_ctx.run(input, |ctx| {
                                state.config_panel.render_window(ctx);

                                let (scrollback_offset, max_scrollback) =
                                    if let Ok(mut parser) = pty_session_clone.parser.lock() {
                                        let cur = parser.screen().scrollback();
                                        let max = get_max_scrollback(parser.screen_mut());
                                        (cur, max)
                                    } else {
                                        (0, 0)
                                    };
                                
                                if state.is_vcr_mode {
                                    egui::Window::new("Chronos VCR Timeline")
                                        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -40.0))
                                        .collapsible(false)
                                        .resizable(false)
                                        .title_bar(false)
                                        .show(ctx, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label("⏱ Time-Travel Scrubbing Active");
                                                
                                                let step = (max_scrollback / 10).max(1);
                                                
                                                if ui.button("⏪").clicked() {
                                                    if let Ok(mut parser) = pty_session_clone.parser.lock() {
                                                        let new_val = scrollback_offset.saturating_add(step).min(max_scrollback);
                                                        parser.screen_mut().set_scrollback(new_val);
                                                    }
                                                }
                                                
                                                // Slider for scrubbing (0 is present, max_scrollback is oldest).
                                                // We invert it for UI: 100% is present, 0% is oldest.
                                                let mut scrub_percent = if max_scrollback > 0 {
                                                    100.0 * (1.0 - (scrollback_offset as f64 / max_scrollback as f64))
                                                } else {
                                                    100.0
                                                };
                                                
                                                let slider = ui.add(egui::Slider::new(&mut scrub_percent, 0.0..=100.0).text("Time %").show_value(true));
                                                
                                                if slider.changed() {
                                                    if max_scrollback > 0 {
                                                        let target_offset = ((1.0 - (scrub_percent / 100.0)) * max_scrollback as f64).round() as usize;
                                                        if let Ok(mut parser) = pty_session_clone.parser.lock() {
                                                            parser.screen_mut().set_scrollback(target_offset.min(max_scrollback));
                                                        }
                                                    }
                                                }

                                                if ui.button("⏩").clicked() {
                                                    if let Ok(mut parser) = pty_session_clone.parser.lock() {
                                                        let new_val = scrollback_offset.saturating_sub(step);
                                                        parser.screen_mut().set_scrollback(new_val);
                                                    }
                                                }
                                                if ui.button("Fork to Sandbox").clicked() {
                                                    println!("Forking sandbox...");
                                                }
                                                if ui.button("Exit (Ctrl+Shift+T)").clicked() {
                                                    state.is_vcr_mode = false;
                                                    // Reset scrollback on exit
                                                    if let Ok(mut parser) = pty_session_clone.parser.lock() {
                                                        parser.screen_mut().set_scrollback(0);
                                                    }
                                                }
                                            });
                                        });
                                }

                                let screen_w = surface_config.width as f32 / scale_factor;
                                let screen_h = surface_config.height as f32 / scale_factor;

                                // 1. Sleek floating jump-to-bottom badge at bottom-right
                                if scrollback_offset > 0 {
                                    egui::Area::new(egui::Id::new("terminal_scrollback_hud"))
                                        .anchor(
                                            egui::Align2::RIGHT_BOTTOM,
                                            egui::vec2(-24.0, -16.0),
                                        )
                                        .order(egui::Order::Foreground)
                                        .show(ctx, |ui| {
                                            egui::Frame::none()
                                                .fill(egui::Color32::from_rgba_premultiplied(
                                                    10, 15, 26, 235,
                                                ))
                                                .stroke(egui::Stroke::new(
                                                    1.0_f32,
                                                    egui::Color32::from_rgb(56, 189, 248),
                                                ))
                                                .rounding(egui::Rounding::same(8.0))
                                                .inner_margin(egui::Margin::symmetric(10.0, 5.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(
                                                            egui::RichText::new(format!(
                                                                "▲ -{} lines",
                                                                scrollback_offset
                                                            ))
                                                            .color(egui::Color32::from_rgb(
                                                                56, 189, 248,
                                                            ))
                                                            .strong()
                                                            .size(11.0),
                                                        );
                                                        ui.add_space(4.0);
                                                        if ui
                                                            .button(
                                                                egui::RichText::new("Bottom ↵")
                                                                    .color(egui::Color32::from_rgb(
                                                                        57, 255, 20,
                                                                    ))
                                                                    .strong()
                                                                    .size(10.5),
                                                            )
                                                            .clicked()
                                                        {
                                                            if let Ok(mut parser) =
                                                                pty_session_clone.parser.lock()
                                                            {
                                                                parser
                                                                    .screen_mut()
                                                                    .set_scrollback(0);
                                                                win_clone.request_redraw();
                                                            }
                                                        }
                                                    });
                                                });
                                        });
                                }

                                // 2. Floating HUD Toast notification (Copied / Pasted)
                                if let Some((msg, ts)) = &hud_notification {
                                    if ts.elapsed().as_millis() < 2200 {
                                        egui::Area::new(egui::Id::new("terminal_toast_hud"))
                                            .anchor(
                                                egui::Align2::CENTER_TOP,
                                                egui::vec2(0.0, 24.0),
                                            )
                                            .order(egui::Order::Tooltip)
                                            .show(ctx, |ui| {
                                                egui::Frame::none()
                                                    .fill(egui::Color32::from_rgba_premultiplied(15, 23, 42, 245))
                                                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(56, 189, 248)))
                                                    .rounding(egui::Rounding::same(8.0))
                                                    .inner_margin(egui::Margin::symmetric(14.0, 7.0))
                                                    .show(ui, |ui| {
                                                        ui.label(
                                                            egui::RichText::new(msg)
                                                                .color(egui::Color32::WHITE)
                                                                .strong()
                                                                .size(12.0),
                                                        );
                                                    });
                                            });
                                        ctx.request_repaint_after(std::time::Duration::from_millis(100));
                                    }
                                }

                                // 3. Interactive Terminal Scrollbar on right edge
                                if max_scrollback > 0 {
                                    let track_w = 6.0;
                                    let track_x = screen_w - track_w - 2.0;
                                    let total_lines = max_scrollback + grid_rows as usize;
                                    let thumb_h = ((grid_rows as f32 / total_lines as f32) * screen_h).clamp(24.0, screen_h * 0.7);
                                    let scroll_ratio = (scrollback_offset as f32 / max_scrollback as f32).clamp(0.0, 1.0);
                                    let thumb_y = (screen_h - thumb_h) * (1.0 - scroll_ratio);

                                    let painter = ctx.layer_painter(egui::LayerId::new(
                                        egui::Order::Foreground,
                                        egui::Id::new("terminal_scrollbar_layer"),
                                    ));

                                    // Track background
                                    painter.rect_filled(
                                        egui::Rect::from_min_size(egui::pos2(track_x, 0.0), egui::vec2(track_w, screen_h)),
                                        egui::Rounding::same(3.0),
                                        egui::Color32::from_rgba_premultiplied(255, 255, 255, 15),
                                    );

                                    // Thumb
                                    painter.rect_filled(
                                        egui::Rect::from_min_size(egui::pos2(track_x, thumb_y), egui::vec2(track_w, thumb_h)),
                                        egui::Rounding::same(3.0),
                                        if scrollback_offset > 0 || is_dragging_scrollbar {
                                            egui::Color32::from_rgb(56, 189, 248)
                                        } else {
                                            egui::Color32::from_rgba_premultiplied(148, 163, 184, 120)
                                        },
                                    );
                                }

                                // 3. Interactive Right-Click Context Menu for Terminal Actions
                                if let Some(pos) = context_menu_pos {
                                    let has_selection = terminal_selection.map_or(false, |s| !s.is_empty());
                                    let mut close_menu = false;
                                    let mut do_copy = false;
                                    let mut do_paste = false;
                                    let mut do_select_all = false;
                                    let mut do_clear_sel = false;

                                    let screen_w = surface_config.width as f32 / scale_factor;
                                    let screen_h = surface_config.height as f32 / scale_factor;
                                    let menu_w = 190.0_f32;
                                    let menu_h = 140.0_f32;
                                    let clamped_x = pos.x.min(screen_w - menu_w - 10.0).max(10.0);
                                    let clamped_y = pos.y.min(screen_h - menu_h - 10.0).max(10.0);
                                    let menu_pos = egui::pos2(clamped_x, clamped_y);
                                    let menu_rect = egui::Rect::from_min_size(menu_pos, egui::vec2(menu_w, menu_h));

                                    egui::Area::new(egui::Id::new("terminal_right_click_menu"))
                                        .fixed_pos(menu_pos)
                                        .order(egui::Order::Foreground)
                                        .interactable(true)
                                        .show(ctx, |ui| {
                                            egui::Frame::none()
                                                .fill(egui::Color32::from_rgba_premultiplied(12, 16, 26, 252))
                                                .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(56, 189, 248)))
                                                .rounding(egui::Rounding::same(8.0))
                                                .shadow(egui::epaint::Shadow {
                                                    offset: egui::vec2(0.0, 4.0),
                                                    blur: 14.0,
                                                    spread: 1.0,
                                                    color: egui::Color32::from_black_alpha(200),
                                                })
                                                .inner_margin(egui::Margin::symmetric(6.0, 6.0))
                                                .show(ui, |ui| {
                                                    ui.set_width(menu_w - 12.0);
                                                    ui.vertical(|ui| {
                                                        let copy_sc = if cfg!(target_os = "macos") { "⌘C" } else { "Ctrl+Shift+C" };
                                                        let paste_sc = if cfg!(target_os = "macos") { "⌘V" } else { "Ctrl+Shift+V" };
                                                        let sel_all_sc = if cfg!(target_os = "macos") { "⌘A" } else { "Ctrl+Shift+A" };

                                                        ui.style_mut().spacing.button_padding = egui::vec2(8.0, 5.0);

                                                        let copy_btn = ui.add_sized(
                                                            [ui.available_width(), 26.0],
                                                            egui::Button::new(
                                                                egui::RichText::new("📋  Copy")
                                                                    .color(if has_selection {
                                                                        egui::Color32::from_rgb(255, 255, 255)
                                                                    } else {
                                                                        egui::Color32::from_rgb(100, 116, 139)
                                                                    })
                                                                    .size(13.0),
                                                            )
                                                            .shortcut_text(copy_sc)
                                                            .fill(egui::Color32::TRANSPARENT),
                                                        );
                                                        if copy_btn.clicked() && has_selection {
                                                            do_copy = true;
                                                            close_menu = true;
                                                        }

                                                        let paste_btn = ui.add_sized(
                                                            [ui.available_width(), 26.0],
                                                            egui::Button::new(
                                                                egui::RichText::new("📥  Paste")
                                                                    .color(egui::Color32::from_rgb(255, 255, 255))
                                                                    .size(13.0),
                                                            )
                                                            .shortcut_text(paste_sc)
                                                            .fill(egui::Color32::TRANSPARENT),
                                                        );
                                                        if paste_btn.clicked() {
                                                            do_paste = true;
                                                            close_menu = true;
                                                        }

                                                        ui.separator();

                                                        let select_all_btn = ui.add_sized(
                                                            [ui.available_width(), 26.0],
                                                            egui::Button::new(
                                                                egui::RichText::new("🔤  Select All")
                                                                    .color(egui::Color32::from_rgb(203, 213, 225))
                                                                    .size(12.5),
                                                            )
                                                            .shortcut_text(sel_all_sc)
                                                            .fill(egui::Color32::TRANSPARENT),
                                                        );
                                                        if select_all_btn.clicked() {
                                                            do_select_all = true;
                                                            close_menu = true;
                                                        }

                                                        if has_selection {
                                                            let clear_btn = ui.add_sized(
                                                                [ui.available_width(), 26.0],
                                                                egui::Button::new(
                                                                    egui::RichText::new("❌  Clear Selection")
                                                                        .color(egui::Color32::from_rgb(248, 113, 113))
                                                                        .size(12.5),
                                                                )
                                                                .fill(egui::Color32::TRANSPARENT),
                                                            );
                                                            if clear_btn.clicked() {
                                                                do_clear_sel = true;
                                                                close_menu = true;
                                                            }
                                                        }
                                                    });
                                                });
                                        });

                                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        close_menu = true;
                                    }

                                    if ctx.input(|i| i.pointer.any_pressed()) {
                                        if let Some(interact_pos) = ctx.input(|i| i.pointer.interact_pos()) {
                                            if !menu_rect.contains(interact_pos) {
                                                close_menu = true;
                                            }
                                        }
                                    }

                                    if do_copy {
                                        if let Some(sel) = terminal_selection {
                                            if let Ok(parser) = pty_session_clone.parser.lock() {
                                                let text = well_render::extract_text_from_screen(parser.screen(), sel);
                                                if !text.is_empty() {
                                                    copy_to_clipboard(&text);
                                                    hud_notification = Some((
                                                        format!("📋 Copied {} chars", text.chars().count()),
                                                        std::time::Instant::now(),
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                    if do_paste {
                                        if let Some(text) = get_from_clipboard() {
                                            paste_into_terminal(&pty_session_clone, &text);
                                            hud_notification = Some((
                                                format!("📋 Pasted {} chars", text.chars().count()),
                                                std::time::Instant::now(),
                                            ));
                                            win_clone.request_redraw();
                                        }
                                    }
                                    if do_select_all {
                                        terminal_selection = Some(SelectionRange::new(
                                            0,
                                            0,
                                            grid_cols.saturating_sub(1) as u16,
                                            grid_rows.saturating_sub(1) as u16,
                                        ));
                                        win_clone.request_redraw();
                                    }
                                    if do_clear_sel {
                                        terminal_selection = None;
                                        win_clone.request_redraw();
                                    }
                                    if close_menu {
                                        context_menu_pos = None;
                                        win_clone.request_redraw();
                                    }
                                }

                                // 4. Interactive In-Terminal Search Overlay (Cmd+F / Ctrl+F)
                                if search_active {
                                    let mut do_prev = false;
                                    let mut do_next = false;
                                    let mut close_search = false;

                                    egui::Area::new(egui::Id::new("terminal_search_overlay"))
                                        .anchor(
                                            egui::Align2::RIGHT_TOP,
                                            egui::vec2(-16.0, well_config::TOP_BAR_HEIGHT + 8.0),
                                        )
                                        .order(egui::Order::Foreground)
                                        .show(ctx, |ui| {
                                            egui::Frame::none()
                                                .fill(egui::Color32::from_rgba_premultiplied(10, 14, 24, 250))
                                                .stroke(egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(56, 189, 248)))
                                                .rounding(egui::Rounding::same(8.0))
                                                .shadow(egui::epaint::Shadow {
                                                    offset: egui::vec2(0.0, 4.0),
                                                    blur: 12.0,
                                                    spread: 1.0,
                                                    color: egui::Color32::from_black_alpha(200),
                                                })
                                                .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(egui::RichText::new("🔍").size(12.0));
                                                        let edit = ui.add(
                                                            egui::TextEdit::singleline(&mut search_query)
                                                                .hint_text("Find in scrollback...")
                                                                .desired_width(160.0),
                                                        );
                                                        if edit.changed() {
                                                            search_matches.clear();
                                                            if !search_query.is_empty() {
                                                                if let Ok(parser) = pty_session_clone.parser.lock() {
                                                                    let total_rows = grid_rows as u16;
                                                                    for r in 0..total_rows {
                                                                        let text = get_line_text_from_screen(parser.screen(), r);
                                                                        if text.to_lowercase().contains(&search_query.to_lowercase()) {
                                                                            search_matches.push(r as usize);
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            search_match_idx = 0;
                                                        }

                                                        let match_badge = if search_query.is_empty() {
                                                            "".to_string()
                                                        } else if search_matches.is_empty() {
                                                            "0/0".to_string()
                                                        } else {
                                                            format!("{}/{}", search_match_idx + 1, search_matches.len())
                                                        };

                                                        if !match_badge.is_empty() {
                                                            ui.label(
                                                                egui::RichText::new(match_badge)
                                                                    .color(egui::Color32::from_rgb(148, 163, 184))
                                                                    .size(11.0),
                                                            );
                                                        }

                                                        if ui.button("▲").on_hover_text("Previous match").clicked() {
                                                            do_prev = true;
                                                        }
                                                        if ui.button("▼").on_hover_text("Next match").clicked() {
                                                            do_next = true;
                                                        }
                                                        if ui.button("✕").clicked() {
                                                            close_search = true;
                                                        }
                                                    });
                                                });
                                        });

                                    if do_prev && !search_matches.is_empty() {
                                        search_match_idx = if search_match_idx == 0 { search_matches.len() - 1 } else { search_match_idx - 1 };
                                    }
                                    if do_next && !search_matches.is_empty() {
                                        search_match_idx = (search_match_idx + 1) % search_matches.len();
                                    }
                                    if close_search {
                                        search_active = false;
                                        win_clone.request_redraw();
                                    }
                                }

                                // 5. Universal Command Palette Modal (Cmd+Shift+P / Cmd+P)
                                if palette_active {
                                    let mut execute_cmd: Option<&str> = None;
                                    let mut close_palette = false;

                                    egui::Area::new(egui::Id::new("terminal_command_palette_modal"))
                                        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
                                        .order(egui::Order::Tooltip)
                                        .show(ctx, |ui| {
                                            egui::Frame::none()
                                                .fill(egui::Color32::from_rgba_premultiplied(10, 14, 26, 252))
                                                .stroke(egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(168, 85, 247)))
                                                .rounding(egui::Rounding::same(10.0))
                                                .shadow(egui::epaint::Shadow {
                                                    offset: egui::vec2(0.0, 8.0),
                                                    blur: 24.0,
                                                    spread: 2.0,
                                                    color: egui::Color32::from_black_alpha(220),
                                                })
                                                .inner_margin(egui::Margin::symmetric(14.0, 12.0))
                                                .show(ui, |ui| {
                                                    ui.set_width(420.0);
                                                    ui.horizontal(|ui| {
                                                        ui.label(
                                                            egui::RichText::new("⌘")
                                                                .size(14.0)
                                                                .color(egui::Color32::from_rgb(168, 85, 247)),
                                                        );
                                                        let input_resp = ui.add(
                                                            egui::TextEdit::singleline(&mut palette_query)
                                                                .hint_text("Search terminal actions...")
                                                                .desired_width(380.0),
                                                        );
                                                        input_resp.request_focus();
                                                    });
                                                    ui.add_space(6.0);
                                                    ui.separator();
                                                    ui.add_space(4.0);

                                                    let actions: [(&str, &str); 16] = [
                                                        ("Clear Terminal Buffer", "Cmd+K"),
                                                        ("Find in Scrollback", "Cmd+F"),
                                                        ("Increase Font Size", "Cmd+="),
                                                        ("Decrease Font Size", "Cmd+-"),
                                                        ("Reset Font Size (14pt)", "Cmd+0"),
                                                        ("Generate Image: Synthwave Grid", "AI Gen"),
                                                        ("Generate Image: Cyberpunk City", "AI Gen"),
                                                        ("Open Generated Images Folder", "Finder"),
                                                        ("Theme: Cyber-Neon", "Preset 0"),
                                                        ("Theme: Tokyo Night", "Preset 1"),
                                                        ("Theme: Matrix Green", "Preset 2"),
                                                        ("Theme: Synthwave '84", "Preset 3"),
                                                        ("Toggle CRT Shaders", "CRT FX"),
                                                        ("Open Pythia AI Copilot", "Cmd+I"),
                                                        ("Open Control Center Settings", "Cmd+,"),
                                                        ("Scroll to Top of Scrollback", "Home"),
                                                    ];

                                                    let filtered: Vec<(&str, &str)> = actions
                                                        .iter()
                                                        .copied()
                                                        .filter(|(name, _)| {
                                                            palette_query.is_empty()
                                                                || name.to_lowercase().contains(&palette_query.to_lowercase())
                                                        })
                                                        .collect();

                                                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                                                        if !filtered.is_empty() {
                                                            palette_selected_idx = (palette_selected_idx + 1) % filtered.len();
                                                        }
                                                    }
                                                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                                                        if !filtered.is_empty() {
                                                            palette_selected_idx = if palette_selected_idx == 0 {
                                                                filtered.len() - 1
                                                            } else {
                                                                palette_selected_idx - 1
                                                            };
                                                        }
                                                    }
                                                    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                                                        if let Some(&(name, _)) = filtered.get(palette_selected_idx) {
                                                            execute_cmd = Some(name);
                                                        }
                                                    }
                                                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                                                        close_palette = true;
                                                    }

                                                    egui::ScrollArea::vertical()
                                                        .max_height(240.0)
                                                        .show(ui, |ui| {
                                                            for (idx, (name, sc)) in filtered.iter().enumerate() {
                                                                let is_selected = idx == palette_selected_idx;
                                                                let text_col = if is_selected {
                                                                    egui::Color32::from_rgb(192, 132, 252)
                                                                } else {
                                                                    egui::Color32::from_rgb(220, 230, 242)
                                                                };
                                                                let fill = if is_selected {
                                                                    egui::Color32::from_rgba_premultiplied(168, 85, 247, 40)
                                                                } else {
                                                                    egui::Color32::TRANSPARENT
                                                                };

                                                                let row_btn = egui::Button::new(
                                                                    egui::RichText::new(*name)
                                                                        .color(text_col)
                                                                        .strong()
                                                                        .size(12.0),
                                                                )
                                                                .shortcut_text(*sc)
                                                                .fill(fill)
                                                                .rounding(egui::Rounding::same(4.0));

                                                                if ui.add_sized([ui.available_width(), 24.0], row_btn).clicked() {
                                                                    execute_cmd = Some(*name);
                                                                }
                                                            }
                                                        });
                                                });
                                        });

                                    if let Some(cmd) = execute_cmd {
                                        match cmd {
                                            "Clear Terminal Buffer" => {
                                                let _ = pty_session_clone.write_all(b"\x1b[2J\x1b[H\x1b[3J");
                                                if let Ok(mut parser) = pty_session_clone.parser.lock() {
                                                    parser.screen_mut().set_scrollback(0);
                                                }
                                                hud_notification = Some(("✨ Buffer Cleared".to_string(), std::time::Instant::now()));
                                            }
                                            "Find in Scrollback" => {
                                                search_active = true;
                                            }
                                            "Increase Font Size" => {
                                                let mut cfg = state.channel.read_state();
                                                cfg.font_size = (cfg.font_size + 1.0).min(32.0);
                                                state.channel.sync_state(cfg.clone());
                                                hud_notification = Some((format!("🔍 Font Size: {:.1} pt", cfg.font_size), std::time::Instant::now()));
                                            }
                                            "Decrease Font Size" => {
                                                let mut cfg = state.channel.read_state();
                                                cfg.font_size = (cfg.font_size - 1.0).max(8.0);
                                                state.channel.sync_state(cfg.clone());
                                                hud_notification = Some((format!("🔍 Font Size: {:.1} pt", cfg.font_size), std::time::Instant::now()));
                                            }
                                            "Reset Font Size (14pt)" => {
                                                let mut cfg = state.channel.read_state();
                                                cfg.font_size = 14.0;
                                                state.channel.sync_state(cfg.clone());
                                                hud_notification = Some(("↺ Font Reset: 14.0 pt".to_string(), std::time::Instant::now()));
                                            }
                                            "Theme: Cyber-Neon" => {
                                                let mut cfg = state.channel.read_state();
                                                cfg.theme_id = 0;
                                                state.channel.sync_state(cfg);
                                            }
                                            "Theme: Tokyo Night" => {
                                                let mut cfg = state.channel.read_state();
                                                cfg.theme_id = 1;
                                                state.channel.sync_state(cfg);
                                            }
                                            "Theme: Matrix Green" => {
                                                let mut cfg = state.channel.read_state();
                                                cfg.theme_id = 2;
                                                state.channel.sync_state(cfg);
                                            }
                                            "Theme: Synthwave '84" => {
                                                let mut cfg = state.channel.read_state();
                                                cfg.theme_id = 3;
                                                state.channel.sync_state(cfg);
                                            }
                                            "Toggle CRT Shaders" => {
                                                let mut cfg = state.channel.read_state();
                                                if cfg.screen_curvature > 0.01 {
                                                    cfg.screen_curvature = 0.0;
                                                    cfg.scanline_frequency = 0.0;
                                                } else {
                                                    cfg.screen_curvature = 0.15;
                                                    cfg.scanline_frequency = 240.0;
                                                }
                                                state.channel.sync_state(cfg);
                                            }
                                            "Generate Image: Synthwave Grid" => {
                                                state.config_panel.show_pythia_hud = true;
                                                state.config_panel.pythia_query = "/image synthwave neon grid wireframe sunset".to_string();
                                                state.config_panel.trigger_image_generation("synthwave neon grid wireframe sunset", 1024, 1024);
                                                hud_notification = Some(("🖼 Generating Synthwave Grid...".to_string(), std::time::Instant::now()));
                                            }
                                            "Generate Image: Cyberpunk City" => {
                                                state.config_panel.show_pythia_hud = true;
                                                state.config_panel.pythia_query = "/image cyberpunk matrix rain city skyline".to_string();
                                                state.config_panel.trigger_image_generation("cyberpunk matrix rain city skyline", 1024, 1024);
                                                hud_notification = Some(("🖼 Generating Cyberpunk City...".to_string(), std::time::Instant::now()));
                                            }
                                            "Open Generated Images Folder" => {
                                                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                                                let cache_dir = std::path::Path::new(&home).join(".well").join("generated");
                                                let _ = std::process::Command::new("open").arg(&cache_dir).spawn();
                                                hud_notification = Some(("📁 Opening Generated Images Folder".to_string(), std::time::Instant::now()));
                                            }
                                            "Open Pythia AI Copilot" => {
                                                state.config_panel.show_pythia_hud = true;
                                            }
                                            "Open Control Center Settings" => {
                                                state.config_panel.is_open = true;
                                            }
                                            "Scroll to Top of Scrollback" => {
                                                if let Ok(mut parser) = pty_session_clone.parser.lock() {
                                                    parser.screen_mut().set_scrollback(usize::MAX);
                                                }
                                            }
                                            _ => {}
                                        }
                                        palette_active = false;
                                        win_clone.request_redraw();
                                    }

                                    if close_palette {
                                        palette_active = false;
                                        win_clone.request_redraw();
                                    }
                                }

                                // 6. Hyperlink Hover Tooltip
                                if let Some(ref url) = hovered_hyperlink {
                                    if !search_active && !palette_active && !state.config_panel.is_open && !state.config_panel.show_pythia_hud {
                                        egui::Area::new(egui::Id::new("hyperlink_hover_tooltip"))
                                            .fixed_pos(egui::pos2(last_cursor_pos.x + 12.0, last_cursor_pos.y + 14.0))
                                            .order(egui::Order::Tooltip)
                                            .show(ctx, |ui| {
                                                egui::Frame::none()
                                                    .fill(egui::Color32::from_rgba_premultiplied(12, 16, 28, 245))
                                                    .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(56, 189, 248)))
                                                    .rounding(egui::Rounding::same(6.0))
                                                    .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                                    .show(ui, |ui| {
                                                        ui.horizontal(|ui| {
                                                            ui.label(egui::RichText::new("🔗").size(10.5));
                                                            ui.label(
                                                                egui::RichText::new(format!("Cmd+Click to open {}", url))
                                                                    .color(egui::Color32::from_rgb(56, 189, 248))
                                                                    .size(11.0),
                                                            );
                                                        });
                                                    });
                                            });
                                    }
                                }

                                // 7. Floating Image Gen Quick Action in Terminal Viewport
                                if !search_active && !palette_active && !state.config_panel.is_open && !state.config_panel.show_pythia_hud {
                                    let screen_w = surface_config.width as f32 / scale_factor;
                                    let screen_h = surface_config.height as f32 / scale_factor;
                                    egui::Area::new(egui::Id::new("floating_image_gen_pill"))
                                        .fixed_pos(egui::pos2(screen_w - 145.0, screen_h - 34.0))
                                        .show(ctx, |ui| {
                                            let btn = egui::Button::new(
                                                egui::RichText::new("🖼 Gen Image")
                                                    .color(egui::Color32::from_rgb(56, 189, 248))
                                                    .strong()
                                                    .size(11.0),
                                            )
                                            .fill(egui::Color32::from_rgba_premultiplied(12, 18, 30, 230))
                                            .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_premultiplied(56, 189, 248, 140)))
                                            .rounding(egui::Rounding::same(12.0));
                                            if ui.add(btn).on_hover_text("Open Cybernetic Image Generator (Cmd+I)").clicked() {
                                                state.config_panel.show_pythia_hud = true;
                                                if state.config_panel.pythia_query.trim().is_empty() {
                                                    state.config_panel.pythia_query = "/image synthwave neon grid wireframe sunset".to_string();
                                                }
                                            }
                                        });
                                }
                            });

                            if !full_output.platform_output.copied_text.is_empty() {
                                copy_to_clipboard(&full_output.platform_output.copied_text);
                            }

                            // Handle pending top-left window control actions (Close, Minimize, Fullscreen)
                            match state.config_panel.pending_window_action {
                                well_config::WindowControlAction::Close => {
                                    println!("Window close requested via HUD, exiting cleanly.");
                                    elwt.exit();
                                    return;
                                }
                                well_config::WindowControlAction::Minimize => {
                                    window.set_minimized(true);
                                    state.config_panel.pending_window_action =
                                        well_config::WindowControlAction::None;
                                }
                                well_config::WindowControlAction::ToggleFullscreen => {
                                    let is_max = window.is_maximized();
                                    window.set_maximized(!is_max);
                                    state.config_panel.pending_window_action =
                                        well_config::WindowControlAction::None;
                                }
                                well_config::WindowControlAction::None => {}
                            }

                            if let Some(injection) = state.config_panel.pending_pty_injection.take() {
                                let _ = pty_session.write_all(injection.as_bytes());
                                window.request_redraw();
                            }

                            let repaint_delay = full_output
                                .viewport_output
                                .get(&egui::ViewportId::ROOT)
                                .map(|v| v.repaint_delay)
                                .unwrap_or(std::time::Duration::from_millis(530));
                            next_repaint_delay = repaint_delay;
                            if repaint_delay.is_zero() {
                                window.request_redraw();
                            }

                            let paint_jobs = egui_ctx.tessellate(full_output.shapes, scale_factor);
                            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                                size_in_pixels: [surface_config.width, surface_config.height],
                                pixels_per_point: scale_factor,
                            };

                            for (id, image_delta) in &full_output.textures_delta.set {
                                egui_renderer.update_texture(
                                    &renderer.device,
                                    &renderer.queue,
                                    *id,
                                    image_delta,
                                );
                            }

                            let _cmd_bufs = egui_renderer.update_buffers(
                                &renderer.device,
                                &renderer.queue,
                                &mut encoder,
                                &paint_jobs,
                                &screen_descriptor,
                            );

                            {
                                let mut rpass = encoder
                                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                                        label: Some("EguiRenderPass"),
                                        color_attachments: &[Some(
                                            wgpu::RenderPassColorAttachment {
                                                view: &view,
                                                resolve_target: None,
                                                ops: wgpu::Operations {
                                                    load: wgpu::LoadOp::Load, // Preserve Orpheus terminal text underneath
                                                    store: wgpu::StoreOp::Store,
                                                },
                                            },
                                        )],
                                        depth_stencil_attachment: None,
                                        timestamp_writes: None,
                                        occlusion_query_set: None,
                                    })
                                    .forget_lifetime();
                                egui_renderer.render(&mut rpass, &paint_jobs, &screen_descriptor);
                            }

                            for id in &full_output.textures_delta.free {
                                egui_renderer.free_texture(id);
                            }
                        }

                        renderer.queue.submit(std::iter::once(encoder.finish()));
                        frame.present();
                    }
                    _ => (),
                }
            }
            Event::AboutToWait => {
                let current_seq = state.channel.sequence();
                if current_seq != last_channel_seq {
                    last_channel_seq = current_seq;
                    window.request_redraw();
                }

                if let Ok(rx) = state.pty_session.graphic_events_rx.lock() {
                    while let Ok(event) = rx.try_recv() {
                        match event {
                            well_render::graphics_protocol::GraphicEvent::KittyImage { id, placement_id: _, format, action: _, payload_base64: _, is_complete: _ } => {
                                // TODO: Buffer and send to pipeline.rs
                                println!("Received Kitty Image: id={} format={}", id, format);
                                window.request_redraw();
                            }
                            well_render::graphics_protocol::GraphicEvent::WorkingDirUpdate(uri) => {
                                println!("CWD updated: {}", uri);
                            }
                            well_render::graphics_protocol::GraphicEvent::Hyperlink(uri_opt) => {
                                if let Some(uri) = uri_opt {
                                    hovered_hyperlink = Some(uri);
                                } else {
                                    hovered_hyperlink = None;
                                }
                            }
                        }
                    }
                }

                while let Ok(cmd) = app_rx.try_recv() {
                    match cmd {
                        well_ipc::HermesAppCommand::PublishAgentLifecycle { event_name, payload, respond_to } => {
                            state.config_panel.agent_events.push(well_config::AgentEvent {
                                event_type: event_name,
                                payload,
                                timestamp: Some(std::time::Instant::now()),
                            });
                            window.request_redraw();
                            let _ = respond_to.send(Ok(serde_json::Value::Null));
                        }
                        _ => {}
                    }
                }

                let live_config = state.channel.read_state();
                let blink_remaining = if live_config.cursor_blink {
                    let elapsed_ms = last_blink_reset.elapsed().as_millis();
                    let phase_ms = elapsed_ms % 530;
                    std::time::Duration::from_millis((530 - phase_ms) as u64)
                } else {
                    std::time::Duration::from_millis(5_000)
                };

                let wait_duration = next_repaint_delay.min(blink_remaining).min(std::time::Duration::from_millis(16));
                if wait_duration.is_zero() {
                    window.request_redraw();
                } else {
                    elwt.set_control_flow(ControlFlow::WaitUntil(
                        std::time::Instant::now() + wait_duration,
                    ));
                }
            }
            _ => (),
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard() {
        copy_to_clipboard("well_test_123");
        let val = get_from_clipboard();
        println!("CLIPBOARD VAL: {:?}", val);
        assert_eq!(val.as_deref(), Some("well_test_123"));
    }

    #[test]
    fn test_pty_paste() {
        let session = PtySession::spawn(24, 80, || {}).expect("PTY");
        std::thread::sleep(std::time::Duration::from_millis(600));
        paste_into_terminal(&session, "echo paste_works\r");
        std::thread::sleep(std::time::Duration::from_millis(600));
        let p = session.parser.lock().unwrap();
        let contents = p.screen().contents();
        println!("CONTENTS AFTER PASTE:\n{}", contents);
        assert!(contents.contains("paste_works"));
    }

    #[test]
    fn test_scrollback_max_calculation() {
        let mut parser = vt100::Parser::new(24, 80, 100);
        for i in 0..50 {
            parser.process(format!("Line {}\r\n", i).as_bytes());
        }
        let max = get_max_scrollback(parser.screen_mut());
        assert!(max > 0, "max scrollback should be > 0 when lines have scrolled off screen");
        assert_eq!(parser.screen().scrollback(), 0, "current scrollback offset should be restored");
        parser.screen_mut().set_scrollback(10);
        let max_after = get_max_scrollback(parser.screen_mut());
        assert_eq!(max_after, max);
        assert_eq!(parser.screen().scrollback(), 10, "scrollback offset should remain 10 after querying max");
    }

    #[test]
    fn test_mouse_wheel_line_accumulation() {
        let mut scroll_line_accumulator: f32 = 0.0;
        let small_deltas = [0.1f32, 0.15, 0.2];
        let mut total_lines = 0;
        for &dy in &small_deltas {
            scroll_line_accumulator += dy * 3.0;
            if scroll_line_accumulator.abs() >= 1.0 {
                let c = scroll_line_accumulator as i32;
                scroll_line_accumulator -= c as f32;
                total_lines += c;
            } else if dy != 0.0 {
                let c = if dy > 0.0 { 1 } else { -1 };
                scroll_line_accumulator = 0.0;
                total_lines += c;
            }
        }
        assert!(total_lines >= 3, "Each discrete notch must produce at least 1 line of scroll");
    }

    #[test]
    fn test_viewport_dimensions_and_offsets() {
        let win_w = 1280.0_f32;
        let win_h = 720.0_f32;
        let scale = 2.0_f32; // Retina 2x
        let font_size = 14.0_f32;
        let line_height = 1.2_f32;

        let cell_w = (font_size * 0.65 * scale).max(6.0);
        let cell_h = (font_size * line_height * scale).max(10.0);

        let top_bar_h = well_config::TOP_BAR_HEIGHT * scale;
        let margin = well_config::FRAME_MARGIN * scale;

        let usable_w = (win_w - 2.0 * margin).max(cell_w);
        let usable_h = (win_h - top_bar_h - margin).max(cell_h);

        let cols = (usable_w / cell_w).floor() as u32;
        let rows = (usable_h / cell_h).floor() as u32;

        assert!(cols > 0 && cols < 200, "Cols should be a reasonable grid dimension: {}", cols);
        assert!(rows > 0 && rows < 100, "Rows should be a reasonable grid dimension: {}", rows);
        assert!(top_bar_h > 0.0, "Top bar height must be positive");
        assert!(margin > 0.0, "Frame margin must be positive");
    }

    #[test]
    fn test_top_bar_hit_test_zones() {
        let screen_w = 800.0_f32;
        let top_bar_h = well_config::TOP_BAR_HEIGHT;

        // Helper to check if a position in the top bar is considered draggable
        let is_draggable = |x: f32, y: f32| -> bool {
            if y >= top_bar_h {
                return false;
            }
            let in_center = (x - screen_w * 0.5).abs() < 50.0;
            let in_right = x > (screen_w - 140.0);
            let in_left = x < 115.0;
            !in_center && !in_right && !in_left
        };

        // 1. Logo area (left) -> NOT draggable
        assert!(!is_draggable(50.0, 15.0));
        // 2. Window controls pill (center) -> NOT draggable
        assert!(!is_draggable(screen_w * 0.5, 15.0));
        assert!(!is_draggable(screen_w * 0.5 - 20.0, 15.0));
        // 3. Settings button (right) -> NOT draggable
        assert!(!is_draggable(screen_w - 50.0, 15.0));
        // 4. Empty space between logo and center -> DRAGGABLE!
        assert!(is_draggable(200.0, 15.0));
        // 5. Empty space between center and settings -> DRAGGABLE!
        assert!(is_draggable(550.0, 15.0));
        // 6. Below top bar (terminal space) -> NOT top-bar draggable
        assert!(!is_draggable(200.0, 50.0));
    }

    #[test]
    fn test_context_menu_boundary_clamping() {
        let screen_w = 1000.0_f32;
        let screen_h = 700.0_f32;
        let menu_w = 190.0_f32;
        let menu_h = 140.0_f32;

        let clamp_menu = |pos_x: f32, pos_y: f32| -> (f32, f32) {
            let cx = pos_x.min(screen_w - menu_w - 10.0).max(10.0);
            let cy = pos_y.min(screen_h - menu_h - 10.0).max(10.0);
            (cx, cy)
        };

        // Click near center stays where clicked
        let (cx, cy) = clamp_menu(300.0, 300.0);
        assert_eq!((cx, cy), (300.0, 300.0));

        // Click off bottom-right edge is safely clamped inside screen
        let (cx_edge, cy_edge) = clamp_menu(990.0, 690.0);
        assert!(cx_edge + menu_w <= screen_w, "Menu should not overflow right edge");
        assert!(cy_edge + menu_h <= screen_h, "Menu should not overflow bottom edge");

        // Click off top-left is clamped away from borders
        let (cx_zero, cy_zero) = clamp_menu(-10.0, -10.0);
        assert_eq!(cx_zero, 10.0);
        assert_eq!(cy_zero, 10.0);
    }

    #[test]
    fn test_selection_and_copy_workflow() {
        let mut parser = vt100::Parser::new(24, 80, 100);
        parser.process(b"hello world from well-shell\r\nsecond line here\r\n");

        // Select "hello world" (row 0, col 0 to 10)
        let sel = well_render::SelectionRange::new(0, 0, 10, 0);
        let extracted = well_render::extract_text_from_screen(parser.screen(), sel);
        assert_eq!(extracted.trim(), "hello world");

        // Copy to system clipboard
        copy_to_clipboard(&extracted);
        let pasted = get_from_clipboard();
        assert_eq!(pasted.as_deref(), Some("hello world"));
    }
}
