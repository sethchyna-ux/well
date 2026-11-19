//! Small native-window UI layer, with explicit input routing above the terminal.
use crate::workspace::{Action, TabInfo};
use egui::{Color32, Event, Pos2, Rect, Stroke, Vec2};
use std::time::{Duration, Instant};
use well_config::{Preferences, TheiasPrismPanel};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Window};

pub const HEADER: f32 = 38.0;
const TABSTRIP: f32 = 32.0;
const CYAN: Color32 = Color32::from_rgb(38, 195, 247);
const VIOLET: Color32 = Color32::from_rgb(181, 112, 246);

pub struct Gui {
    pub ctx: egui::Context,
    pub renderer: egui_wgpu::Renderer,
    pub settings: TheiasPrismPanel,
    pub editor: well_editor::MnemeEditor,
    pub editor_open: bool,
    pub editor_buffer: String,
    pub agent_banner: Option<String>,
    pub search_open: bool,
    pub search_query: String,
    pub search_matches: usize,
    pub metis_output: Option<String>,
    raw: egui::RawInput,
    pointer: Pos2,
    modifiers: ModifiersState,
    started: Instant,
    pub actions: Vec<Action>,
    automation: bool,
    header_height: f32,
    pub header_buttons: Vec<Rect>,
}
impl Gui {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, prefs: &Preferences) -> Self {
        let ctx = egui::Context::default();
        ctx.style_mut(|style| {
            let mut visuals = egui::Visuals::dark();
            visuals.override_text_color = Some(Color32::from_rgb(228, 234, 243));
            visuals.window_fill = Color32::from_rgb(13, 17, 24);
            visuals.panel_fill = Color32::from_rgb(11, 15, 22);
            visuals.window_stroke = Stroke::new(1.5_f32, Color32::from_rgb(38, 52, 75));
            visuals.window_rounding = 12.0.into();
            visuals.menu_rounding = 8.0.into();

            visuals.widgets.noninteractive.rounding = 8.0.into();
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(16, 21, 30);
            visuals.widgets.noninteractive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(28, 38, 55));

            visuals.widgets.inactive.rounding = 8.0.into();
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(20, 27, 39);
            visuals.widgets.inactive.bg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(35, 48, 70));
            visuals.widgets.inactive.fg_stroke =
                Stroke::new(1.0_f32, Color32::from_rgb(205, 215, 228));

            visuals.widgets.hovered.rounding = 8.0.into();
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(30, 42, 62);
            visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, CYAN);
            visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);

            visuals.widgets.active.rounding = 8.0.into();
            visuals.widgets.active.bg_fill = Color32::from_rgb(42, 58, 86);
            visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, VIOLET);
            visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);

            visuals.selection.bg_fill = Color32::from_rgb(35, 65, 110);
            visuals.selection.stroke = Stroke::new(1.5_f32, CYAN);

            style.visuals = visuals;
            style.spacing.item_spacing = Vec2::new(8.0, 6.0);
            style.spacing.button_padding = Vec2::new(10.0, 5.0);
        });
        Self {
            ctx,
            renderer: egui_wgpu::Renderer::new(device, format, None, 1),
            settings: TheiasPrismPanel::new(prefs),
            editor: well_editor::MnemeEditor::default(),
            editor_open: false,
            editor_buffer: String::new(),
            agent_banner: None,
            search_open: false,
            search_query: String::new(),
            search_matches: 0,
            metis_output: None,
            raw: egui::RawInput::default(),
            pointer: Pos2::ZERO,
            modifiers: ModifiersState::empty(),
            started: Instant::now(),
            actions: Vec::new(),
            automation: false,
            header_height: HEADER,
            header_buttons: Vec::new(),
        }
    }
    pub fn is_draggable_header(&self, pos: Pos2) -> bool {
        // Reserve the top 5px margin so the native OS top-edge resize handle works
        if pos.y < 5.0 || pos.y >= self.header_height {
            return false;
        }
        #[cfg(target_os = "macos")]
        if pos.x < 76.0 {
            // Reserve space for native macOS traffic lights (close, minimize, zoom)
            return false;
        }
        if self.settings.open || self.automation || self.editor_open || self.search_open {
            return false;
        }
        !self
            .header_buttons
            .iter()
            .any(|r| r.expand(3.0).contains(pos))
    }
    pub fn region(window: &Window, prefs: &Preferences, tab_count: usize) -> [u32; 4] {
        let scale = window.scale_factor() as f32;
        let size = window.inner_size();
        let border = (if prefs.appearance.geometric_border {
            12.0
        } else {
            1.0
        } * scale)
            .round() as u32;
        let header = HEADER + if tab_count > 1 { TABSTRIP } else { 0.0 };
        let top_padding = if prefs.appearance.geometric_border {
            14.0
        } else {
            4.0
        };
        let top = ((header + top_padding) * scale).round() as u32;
        [
            border,
            top,
            size.width.saturating_sub(border * 2),
            size.height.saturating_sub(top + border),
        ]
    }
    /// Return true when this event belongs to a UI control, so it cannot reach the shell.
    pub fn event(
        &mut self,
        window: &Window,
        event: &WindowEvent,
        clipboard: &mut Option<arboard::Clipboard>,
    ) -> bool {
        let scale = window.scale_factor() as f32;
        let mods = modifiers(self.modifiers);
        let keyboard =
            self.settings.open || self.automation || self.editor_open || self.search_open;
        let pointer =
            keyboard || self.pointer.y < self.header_height || self.ctx.is_using_pointer();
        match event {
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
                self.raw.modifiers = modifiers(self.modifiers);
            }
            WindowEvent::Focused(focused) => {
                self.raw.focused = *focused;
                self.raw.events.push(Event::WindowFocused(*focused));
                if !focused {
                    self.modifiers = ModifiersState::empty();
                    self.raw.modifiers = egui::Modifiers::default();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = Pos2::new(position.x as f32 / scale, position.y as f32 / scale);
                self.raw.events.push(Event::PointerMoved(self.pointer));
                return keyboard
                    || self.pointer.y < self.header_height
                    || self.ctx.is_using_pointer();
            }
            WindowEvent::CursorLeft { .. } => self.raw.events.push(Event::PointerGone),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => Some(egui::PointerButton::Primary),
                    MouseButton::Right => Some(egui::PointerButton::Secondary),
                    MouseButton::Middle => Some(egui::PointerButton::Middle),
                    _ => None,
                };
                if let Some(button) = button {
                    self.raw.events.push(Event::PointerButton {
                        pos: self.pointer,
                        button,
                        pressed: *state == ElementState::Pressed,
                        modifiers: mods,
                    });
                }
                return pointer;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (unit, delta) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        (egui::MouseWheelUnit::Line, Vec2::new(*x, *y))
                    }
                    MouseScrollDelta::PixelDelta(p) => (
                        egui::MouseWheelUnit::Point,
                        Vec2::new(p.x as f32 / scale, p.y as f32 / scale),
                    ),
                };
                self.raw.events.push(Event::MouseWheel {
                    unit,
                    delta,
                    modifiers: mods,
                });
                return pointer;
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } if keyboard => {
                let pressed = event.state == ElementState::Pressed;
                if pressed && event.logical_key == Key::Named(NamedKey::Escape) {
                    self.settings.open = false;
                    self.automation = false;
                    self.editor_open = false;
                    self.search_open = false;
                    self.ctx.memory_mut(|m| {
                        if let Some(id) = m.focused() {
                            m.surrender_focus(id);
                        }
                    });
                    return true;
                }
                if pressed && mods.command {
                    if let Key::Character(ch) = &event.logical_key {
                        match ch.to_ascii_lowercase().as_str() {
                            "c" => self.raw.events.push(Event::Copy),
                            "x" => self.raw.events.push(Event::Cut),
                            "v" => {
                                if clipboard.is_none() {
                                    *clipboard = arboard::Clipboard::new().ok();
                                }
                                match clipboard
                                    .as_mut()
                                    .ok_or_else(|| "Clipboard unavailable".to_owned())
                                    .and_then(|c| c.get_text().map_err(|e| e.to_string()))
                                {
                                    Ok(text) => self.raw.events.push(Event::Paste(text)),
                                    Err(e) => self.settings.error = Some(e),
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(key) = egui_key(&event.logical_key) {
                    self.raw.events.push(Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: event.repeat,
                        modifiers: mods,
                    });
                }
                if pressed
                    && (!(mods.command || mods.ctrl || mods.mac_cmd) || (mods.alt && !mods.mac_cmd))
                {
                    if let Some(text) = &event.text {
                        let text: String = text.chars().filter(|c| !c.is_control()).collect();
                        if !text.is_empty() {
                            self.raw.events.push(Event::Text(text));
                        }
                    }
                }
                return true;
            }
            WindowEvent::Ime(ime) if keyboard => {
                let ime = match ime {
                    Ime::Enabled => egui::ImeEvent::Enabled,
                    Ime::Disabled => egui::ImeEvent::Disabled,
                    Ime::Preedit(text, _) => egui::ImeEvent::Preedit(text.clone()),
                    Ime::Commit(text) => egui::ImeEvent::Commit(text.clone()),
                };
                self.raw.events.push(Event::Ime(ime));
                return true;
            }
            _ => {}
        }
        false
    }
    // Legacy frontend API retained until GPUI parity is verified.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        window: &Window,
        prefs: &Preferences,
        path: &std::path::Path,
        exited: bool,
        selecting: bool,
        preedit: &str,
        ghost_suggestion: Option<&str>,
        terminal_cursor: Option<Rect>,
        tabs: &[TabInfo],
        active_tab: usize,
        rpc: &crate::rpc::RpcHost,
        viewport_state: Option<&well_ipc::HermesSeqlock<well_ipc::HermesViewportState>>,
    ) -> (egui::FullOutput, Option<Preferences>) {
        let scale = window.scale_factor() as f32;
        self.header_height = HEADER + if tabs.len() > 1 { TABSTRIP } else { 0.0 };
        let size = window.inner_size();
        self.raw.screen_rect = Some(Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(size.width as f32 / scale, size.height as f32 / scale),
        ));
        self.raw.time = Some(self.started.elapsed().as_secs_f64());
        if let Some(viewport) = self.raw.viewports.get_mut(&egui::ViewportId::ROOT) {
            viewport.native_pixels_per_point = Some(scale);
        }
        self.settings.telemetry.rpc_socket = rpc.path.display().to_string();
        self.settings.telemetry.rpc_requests = rpc.stats.total_requests();
        self.settings.telemetry.rpc_connections = rpc.stats.active_connections();
        let raw = self.raw.take();
        let mut candidate = None;
        let output = self.ctx.run(raw, |ctx| {
            let header_resp = egui::TopBottomPanel::top("well_header")
                .exact_height(HEADER)
                .frame(
                    egui::Frame::none()
                        .fill(Color32::from_rgb(11, 15, 23))
                        .inner_margin(egui::Margin::symmetric(10.0, 4.0)),
                )
                .show(ctx, |ui| {
                    self.header_buttons.clear();
                    let full_rect = ui.max_rect();
                    let center_x = full_rect.center().x;
                    let is_macos = cfg!(target_os = "macos");

                    let active_info = tabs.get(active_tab);
                    let active_title = active_info
                        .and_then(|t| t.title.as_deref())
                        .unwrap_or_else(|| {
                            active_info
                                .and_then(|t| t.cwd.as_ref())
                                .and_then(|p| p.file_name())
                                .and_then(|n| n.to_str())
                                .unwrap_or("zsh")
                        });
                    let is_ssh = active_title.to_lowercase().contains("ssh");

                    let capsule_width = if is_macos {
                        (active_title.len() as f32 * 7.5 + 48.0).clamp(110.0, 260.0)
                    } else {
                        104.0
                    };
                    let gap = 8.0;

                    let left_rect = Rect::from_min_max(
                        full_rect.left_top(),
                        Pos2::new(center_x - (capsule_width / 2.0) - gap, full_rect.bottom()),
                    );
                    let center_rect = Rect::from_center_size(
                        Pos2::new(center_x, full_rect.center().y),
                        Vec2::new(capsule_width, 24.0),
                    );
                    let right_rect = Rect::from_min_max(
                        Pos2::new(center_x + (capsule_width / 2.0) + gap, full_rect.top()),
                        full_rect.right_bottom(),
                    );

                    // --- Left Section: Native Traffic Light clearance, Brand, Engine status, New Tab ---
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            if is_macos {
                                ui.add_space(76.0); // Reserve clearance for native macOS traffic lights [● ● ●]
                            } else {
                                ui.add_space(4.0);
                            }
                            ui.label(egui::RichText::new("❖").size(18.0).color(CYAN));
                            ui.label(egui::RichText::new("WELL").size(14.0).strong().color(Color32::WHITE));

                            ui.add_space(4.0);
                            egui::Frame::none()
                                .fill(Color32::from_rgb(18, 25, 38))
                                .stroke(Stroke::new(0.5_f32, CYAN.gamma_multiply(0.7)))
                                .rounding(5.0)
                                .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(6.0), egui::Sense::hover());
                                        ui.painter().circle_filled(dot_rect.center(), 2.5, Color32::from_rgb(34, 197, 94));
                                        ui.label(egui::RichText::new("ATLAS").size(10.0).color(CYAN));
                                    });
                                });

                            let plus_resp = ui.add_enabled(
                                tabs.len() < crate::workspace::MAX_TABS,
                                egui::Button::new(egui::RichText::new("+").size(12.0).strong())
                                    .min_size(Vec2::new(22.0, 18.0))
                                    .rounding(4.0),
                            );
                            self.header_buttons.push(plus_resp.rect);
                            if plus_resp.on_hover_text("New tab (Cmd+T / Ctrl+Shift+T)").clicked() {
                                self.actions.push(Action::NewTab);
                            }
                        });
                    });

                    // --- Center Section: Contextual Session Breadcrumb (macOS) / Control Capsule (Other OS) ---
                    if is_macos {
                        ui.allocate_ui_at_rect(center_rect, |ui| {
                            egui::Frame::none()
                                .fill(Color32::from_rgba_premultiplied(18, 25, 38, 175))
                                .stroke(Stroke::new(0.5_f32, Color32::from_rgba_premultiplied(255, 255, 255, 22)))
                                .rounding(6.0)
                                .inner_margin(egui::Margin::symmetric(8.0, 3.0))
                                .show(ui, |ui| {
                                    ui.horizontal_centered(|ui| {
                                        if is_ssh {
                                            let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(6.0), egui::Sense::hover());
                                            ui.painter().circle_filled(dot_rect.center(), 2.5, Color32::from_rgb(34, 197, 94));
                                            ui.label(egui::RichText::new("SSH").size(10.0).color(CYAN));
                                        } else {
                                            ui.label(egui::RichText::new("~").size(11.0).color(CYAN));
                                        }
                                        ui.label(
                                            egui::RichText::new(active_title)
                                                .size(11.0)
                                                .monospace()
                                                .color(Color32::from_rgb(220, 230, 245)),
                                        );
                                    });
                                });
                        });
                    } else {
                        ui.allocate_ui_at_rect(center_rect, |ui| {
                            egui::Frame::none()
                                .fill(Color32::from_rgb(15, 20, 30))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(34, 46, 68)))
                                .rounding(6.0)
                                .inner_margin(egui::Margin::symmetric(4.0, 2.0))
                                .show(ui, |ui| {
                                    ui.horizontal_centered(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

                                        // Minimize Button
                                        let min_btn = egui::Button::new(
                                            egui::RichText::new("−").size(13.0).strong().color(Color32::from_rgb(200, 215, 230))
                                        )
                                        .min_size(Vec2::new(24.0, 18.0))
                                        .fill(Color32::TRANSPARENT)
                                        .rounding(4.0);
                                        let min_resp = ui.add(min_btn);
                                        self.header_buttons.push(min_resp.rect);
                                        if min_resp.on_hover_text("Minimize Window").clicked() {
                                            self.actions.push(Action::MinimizeWindow);
                                        }

                                        ui.label(egui::RichText::new("·").size(10.0).color(Color32::from_rgb(45, 60, 85)));

                                        // Maximize / Zoom Button
                                        let max_btn = egui::Button::new(
                                            egui::RichText::new("□").size(11.0).strong().color(Color32::from_rgb(200, 215, 230))
                                        )
                                        .min_size(Vec2::new(24.0, 18.0))
                                        .fill(Color32::TRANSPARENT)
                                        .rounding(4.0);
                                        let max_resp = ui.add(max_btn);
                                        self.header_buttons.push(max_resp.rect);
                                        if max_resp.on_hover_text("Maximize / Restore Window").clicked() {
                                            self.actions.push(Action::MaximizeWindow);
                                        }

                                        ui.label(egui::RichText::new("·").size(10.0).color(Color32::from_rgb(45, 60, 85)));

                                        // Close Button
                                        let close_btn = egui::Button::new(
                                            egui::RichText::new("✕").size(11.0).strong().color(Color32::from_rgb(245, 110, 130))
                                        )
                                        .min_size(Vec2::new(24.0, 18.0))
                                        .fill(Color32::TRANSPARENT)
                                        .rounding(4.0);
                                        let close_resp = ui.add(close_btn);
                                        self.header_buttons.push(close_resp.rect);
                                        if close_resp.on_hover_text("Close Well (Cmd+Q)").clicked() {
                                            self.actions.push(Action::CloseWindow);
                                        }
                                    });
                                });
                        });
                    }

                    // --- Right Section: Action pills, context badge, Metis indicator ---
                    ui.allocate_ui_at_rect(right_rect, |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(4.0);

                            // Theia's Prism
                            let prism_btn = egui::Button::new(
                                egui::RichText::new("🎨 Theia").size(11.0).color(Color32::from_rgb(230, 200, 255)),
                            )
                            .stroke(Stroke::new(1.0_f32, VIOLET))
                            .fill(if self.settings.open { Color32::from_rgb(45, 25, 65) } else { Color32::from_rgb(20, 16, 28) })
                            .rounding(6.0);
                            let prism_resp = ui.add(prism_btn);
                            self.header_buttons.push(prism_resp.rect);

                            if prism_resp.clicked() {
                                self.settings.toggle(prefs);
                                self.automation = false;
                                self.editor_open = false;
                                self.search_open = false;
                            }

                            // Mneme Editor
                            let mneme_btn = egui::Button::new(
                                egui::RichText::new("🧠 Mneme").size(11.0).color(CYAN),
                            )
                            .stroke(Stroke::new(1.0_f32, CYAN.gamma_multiply(0.6)))
                            .fill(if self.editor_open { Color32::from_rgb(18, 40, 55) } else { Color32::from_rgb(14, 22, 34) })
                            .rounding(6.0);
                            let mneme_resp = ui.add(mneme_btn);
                            self.header_buttons.push(mneme_resp.rect);

                            if mneme_resp.on_hover_text("Mneme inline composition editor (Cmd+E / Ctrl+Shift+E)").clicked() {
                                self.editor_open = !self.editor_open;
                                self.settings.open = false;
                                self.automation = false;
                                self.search_open = false;
                            }

                            // Search
                            let search_btn = egui::Button::new(
                                egui::RichText::new("🔍 Search").size(11.0).color(Color32::from_rgb(180, 220, 255)),
                            )
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 65, 95)))
                            .fill(if self.search_open { Color32::from_rgb(18, 34, 54) } else { Color32::from_rgb(15, 20, 30) })
                            .rounding(6.0);
                            let search_resp = ui.add(search_btn);
                            self.header_buttons.push(search_resp.rect);

                            if search_resp.on_hover_text("Search terminal output (Cmd+F / Ctrl+Shift+F)").clicked() {
                                self.search_open = !self.search_open;
                                self.settings.open = false;
                                self.automation = false;
                                self.editor_open = false;
                            }

                            // Automation
                            let auto_btn = egui::Button::new(
                                egui::RichText::new("⚙️ Tasks").size(11.0).color(Color32::from_rgb(190, 205, 225)),
                            )
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(35, 48, 68)))
                            .fill(if self.automation { Color32::from_rgb(22, 30, 44) } else { Color32::from_rgb(14, 18, 28) })
                            .rounding(6.0);
                            let auto_resp = ui.add(auto_btn);
                            self.header_buttons.push(auto_resp.rect);

                            if auto_resp.clicked() {
                                self.automation = !self.automation;
                                self.settings.open = false;
                                self.editor_open = false;
                                self.search_open = false;
                            }

                            // Context Capsule (Git branch & Language toolchain)
                            if let Some(active_info) = tabs.get(active_tab) {
                                let astraea = &active_info.astraea;
                                if !astraea.git_branch.is_empty() || astraea.rust_version.is_some() || !astraea.cwd.is_empty() {
                                    egui::Frame::none()
                                        .fill(Color32::from_rgb(14, 20, 30))
                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(28, 40, 58)))
                                        .rounding(5.0)
                                        .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing = Vec2::new(5.0, 0.0);
                                                if !astraea.git_branch.is_empty() {
                                                    let dirty = if astraea.is_dirty { "*" } else { "" };
                                                    ui.label(egui::RichText::new(format!("🌿 {}{dirty}", astraea.git_branch)).size(10.5).color(Color32::from_rgb(100, 210, 255)));
                                                }
                                                if let Some(rust) = &astraea.rust_version {
                                                    ui.label(egui::RichText::new(rust).size(10.5).color(Color32::from_rgb(255, 170, 90)));
                                                }
                                                if let Some(node) = &astraea.node_version {
                                                    ui.label(egui::RichText::new(node).size(10.5).color(Color32::from_rgb(120, 230, 130)));
                                                }
                                            });
                                        })
                                        .response
                                        .on_hover_text(active_info.cwd.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
                                }
                            }

                            if selecting {
                                ui.label(egui::RichText::new("Selection").size(10.0).color(CYAN))
                                    .on_hover_text("Output continues in the background. Press Escape or type to return to live output.");
                            }
                            if exited {
                                ui.label(egui::RichText::new("Exited").size(10.0).color(VIOLET));
                            }
                            // Metis status
                            ui.label(egui::RichText::new("⚡ Metis").size(10.5).color(Color32::from_rgb(100, 230, 150)))
                                .on_hover_text("Metis predictive shell engine active (O(k) prefix trie)");
                        });
                    });
                });
            if header_resp.response.double_clicked() {
                self.actions.push(Action::MaximizeWindow);
            } else if header_resp.response.drag_started() {
                self.actions.push(Action::DragWindow);
            }
            let h_rect = header_resp.response.rect;
            ctx.layer_painter(egui::LayerId::background()).line_segment(
                [Pos2::new(h_rect.left(), h_rect.bottom()), Pos2::new(h_rect.right(), h_rect.bottom())],
                Stroke::new(0.5_f32, Color32::from_rgba_premultiplied(255, 255, 255, 18)),
            );
            if tabs.len() > 1 {
                egui::TopBottomPanel::top("well_tabs")
                    .exact_height(TABSTRIP)
                    .frame(
                        egui::Frame::none()
                            .fill(Color32::from_rgb(11, 15, 23))
                            .inner_margin(egui::Margin::symmetric(10.0, 3.0)),
                    )
                    .show(ctx, |ui| {
                        egui::ScrollArea::horizontal()
                            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                                    for (index, tab) in tabs.iter().enumerate() {
                                        let is_active = index == active_tab;
                                        let name = tab.title.as_deref().unwrap_or("Shell");
                                        let title = format!(
                                            "{name} #{}{}",
                                            tab.id,
                                            tab.exit_status.map_or(String::new(), |code| format!(" · {code}"))
                                        );

                                        let bg_fill = if is_active {
                                            Color32::from_rgb(22, 32, 48)
                                        } else {
                                            Color32::from_rgb(14, 18, 27)
                                        };
                                        let border_stroke = if is_active {
                                            Stroke::new(1.0_f32, CYAN.gamma_multiply(0.85))
                                        } else {
                                            Stroke::new(1.0_f32, Color32::from_rgb(28, 38, 54))
                                        };

                                        egui::Frame::none()
                                            .fill(bg_fill)
                                            .stroke(border_stroke)
                                            .rounding(egui::Rounding { nw: 6.0, ne: 6.0, sw: 2.0, se: 2.0 })
                                            .inner_margin(egui::Margin::symmetric(10.0, 4.0))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    let label_color = if is_active {
                                                        Color32::WHITE
                                                    } else {
                                                        Color32::from_rgb(160, 175, 195)
                                                    };
                                                    let tab_btn = egui::Button::new(
                                                        egui::RichText::new(&title).size(11.5).color(label_color)
                                                    )
                                                    .frame(false);
                                                    if ui.add(tab_btn).clicked() {
                                                        self.actions.push(Action::SwitchTab(index));
                                                    }
                                                    if is_active {
                                                        ui.add_space(3.0);
                                                        let close_btn = egui::Button::new(
                                                            egui::RichText::new("×").size(12.0).color(Color32::from_rgb(255, 120, 140))
                                                        )
                                                        .frame(false);
                                                        if ui.add(close_btn).on_hover_text("Close tab (Cmd+W)").clicked() {
                                                            self.actions.push(Action::CloseTab);
                                                        }
                                                    }
                                                });
                                            });
                                    }
                                });
                            });
                    });
            }
            let rect = ctx.screen_rect();
            let painter = ctx.layer_painter(egui::LayerId::background());
            if tabs.len() > 1 {
                painter.line_segment(
                    [Pos2::new(0.0, HEADER), Pos2::new(rect.width(), HEADER)],
                    Stroke::new(1.0_f32, Color32::from_rgb(20, 27, 39)),
                );
            }
            painter.line_segment(
                [Pos2::new(0.0, self.header_height), Pos2::new(rect.width(), self.header_height)],
                Stroke::new(1.0_f32, Color32::from_rgb(26, 36, 52)),
            );
            if prefs.appearance.geometric_border {
                geometry(
                    &painter,
                    Rect::from_min_max(Pos2::new(0.5, self.header_height + 2.0), rect.max - Vec2::splat(0.5)),
                );
            }
            if !self.settings.open && !self.automation && !self.editor_open {
                if !preedit.is_empty() {
                    if let Some(cursor) = terminal_cursor {
                        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("terminal_preedit")));
                        let fg = prefs.appearance.foreground;
                        let bg = prefs.appearance.background;
                        let galley = painter.layout_no_wrap(preedit.to_owned(), egui::FontId::monospace(prefs.appearance.font_size), Color32::from_rgb(fg[0], fg[1], fg[2]));
                        let size = galley.size();
                        let position = Pos2::new(cursor.left().min((ctx.screen_rect().right() - size.x - 12.0).max(12.0)), cursor.top());
                        let bounds = Rect::from_min_size(position, Vec2::new(size.x.max(4.0), size.y));
                        painter.rect_filled(bounds, 0.0, Color32::from_rgb(bg[0], bg[1], bg[2]));
                        painter.galley(position, galley, Color32::WHITE);
                        painter.line_segment([bounds.left_bottom(), bounds.right_bottom()], Stroke::new(1.0_f32, CYAN));
                    }
                } else if let Some(ghost) = ghost_suggestion {
                    if let Some(cursor) = terminal_cursor {
                        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("terminal_ghost")));
                        let pred_rgb = prefs.appearance.prediction_color;
                        let pred_color = Color32::from_rgb(pred_rgb[0], pred_rgb[1], pred_rgb[2]);
                        let galley = painter.layout_no_wrap(
                            ghost.to_owned(),
                            egui::FontId::monospace(prefs.appearance.font_size),
                            pred_color,
                        );
                        let origin = Pos2::new(cursor.right(), cursor.top());
                        let ghost_rect = Rect::from_min_size(origin, galley.size());
                        // Soft tinted highlight backdrop for predictive suggestion
                        painter.rect_filled(
                            ghost_rect.expand2(Vec2::new(2.0, 1.0)),
                            3.0,
                            Color32::from_rgba_unmultiplied(pred_rgb[0] / 4, pred_rgb[1] / 4, pred_rgb[2] / 4, 45),
                        );
                        painter.galley(origin, galley.clone(), pred_color);

                        // Visual Prediction key hint badge
                        let hint_text = " ⇥ Tab / Alt+→ ";
                        let hint_galley = painter.layout_no_wrap(
                            hint_text.to_owned(),
                            egui::FontId::proportional(prefs.appearance.font_size * 0.72),
                            Color32::from_rgb(210, 245, 255),
                        );
                        let hint_origin = Pos2::new(ghost_rect.right() + 6.0, cursor.top() + (galley.size().y - hint_galley.size().y) * 0.5);
                        let hint_rect = Rect::from_min_size(hint_origin, hint_galley.size());
                        painter.rect_filled(hint_rect.expand2(Vec2::new(3.0, 1.5)), 4.0, Color32::from_rgb(18, 42, 58));
                        painter.rect_stroke(hint_rect.expand2(Vec2::new(3.0, 1.5)), 4.0, Stroke::new(1.0_f32, Color32::from_rgb(60, 180, 220)));
                        painter.galley(hint_origin, hint_galley, Color32::from_rgb(210, 245, 255));
                    }
                }
            }
            if self.settings.open {
                if let Some(seqlock) = viewport_state {
                    let vp = seqlock.read();
                    self.settings.telemetry.seqlock_sequence = seqlock.sequence();
                    self.settings.telemetry.viewport_rows = vp.rows;
                    self.settings.telemetry.viewport_cols = vp.cols;
                }
                self.settings.telemetry.physical_width = size.width;
                self.settings.telemetry.physical_height = size.height;
                self.settings.telemetry.scale_factor = scale as f64;
                self.settings.telemetry.surface_id = tabs.get(active_tab).map(|t| t.id).unwrap_or(0);
                self.settings.telemetry.active_tab_index = active_tab;
                self.settings.telemetry.total_tabs = tabs.len();

                // Settings owns input until dismissed. Keep the terminal visible behind it.
                candidate = self.settings.draw(ctx, prefs, path);
                if let Some(target) = self.settings.ssh_connect_requested.take() {
                    self.actions.push(Action::ConnectSsh(target));
                }
                if let Some((cols, rows)) = self.settings.resize_grid_requested.take() {
                    self.actions.push(Action::ResizeWindowGrid(cols, rows));
                }
            }
            if self.editor_open {
                egui::Window::new("Mneme Composition Editor (Tree-sitter Bash AST)")
                    .open(&mut self.editor_open)
                    .default_width(540.0)
                    .default_height(280.0)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            if self.editor.has_syntax_error() {
                                ui.colored_label(Color32::from_rgb(255, 100, 100), "✗ Syntax Error in Bash AST");
                            } else {
                                ui.colored_label(Color32::from_rgb(100, 245, 97), "✓ Bash AST Valid");
                            }
                            let highlights = self.editor.highlights();
                            ui.label(format!("Tokens: {}", highlights.len()));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Send to Shell ↵").clicked() {
                                    let text = self.editor.get_text();
                                    if !text.is_empty() {
                                        self.actions.push(Action::SendInput(format!("{text}\n")));
                                    }
                                }
                                if ui.button("⚡ Run in Metis (In-Process)").on_hover_text("Execute command in-process with zero fork overhead for builtins").clicked() {
                                    let text = self.editor.get_text();
                                    if !text.is_empty() {
                                        self.actions.push(Action::ExecuteMetisInProcess(text));
                                    }
                                }
                                if ui.button("Clear").clicked() {
                                    self.editor.set_text("");
                                    self.editor_buffer.clear();
                                    self.metis_output = None;
                                }
                                if ui.button("Redo").clicked() {
                                    self.editor.redo();
                                    self.editor_buffer = self.editor.get_text();
                                }
                                if ui.button("Undo").clicked() {
                                    self.editor.undo();
                                    self.editor_buffer = self.editor.get_text();
                                }
                            });
                        });
                        if let Some(destr) = self.editor.destructive_warning() {
                            ui.colored_label(Color32::from_rgb(255, 75, 75), destr.to_string());
                        }
                        if let Some(unclosed) = self.editor.unclosed_reason() {
                            ui.colored_label(Color32::from_rgb(255, 180, 50), format!("ℹ️ {unclosed}"));
                        }
                        ui.separator();
                        let highlights = self.editor.highlights();
                        let cap_color = Color32::from_rgb(
                            prefs.appearance.capital_color[0],
                            prefs.appearance.capital_color[1],
                            prefs.appearance.capital_color[2],
                        );
                        let highlight_cap = prefs.appearance.highlight_capitals;
                        let multicolor_cap = prefs.appearance.multicolor_capitals;
                        let syntax_theme = prefs.appearance.syntax_theme.clone();
                        let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                            mneme_syntax_layouter(
                                ui,
                                text,
                                wrap_width,
                                &highlights,
                                cap_color,
                                highlight_cap,
                                multicolor_cap,
                                &syntax_theme,
                            )
                        };
                        let text_edit = egui::TextEdit::multiline(&mut self.editor_buffer)
                            .layouter(&mut layouter)
                            .desired_rows(8)
                            .desired_width(f32::INFINITY);
                        let response = ui.add(text_edit);
                        if response.changed() {
                            self.editor.set_text(&self.editor_buffer);
                        }
                        if response.has_focus() && ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && i.key_pressed(egui::Key::Enter)) {
                            let text = self.editor.get_text();
                            if !text.is_empty() {
                                self.actions.push(Action::SendInput(format!("{text}\n")));
                            }
                        }
                        if let Some(out) = self.metis_output.clone() {
                            ui.separator();
                            let mut clear = false;
                            ui.horizontal(|ui| {
                                ui.colored_label(Color32::from_rgb(120, 220, 160), "Metis Output:");
                                if ui.small_button("Clear Output").clicked() {
                                    clear = true;
                                }
                            });
                            if clear {
                                self.metis_output = None;
                            }
                            let mut out_str = out.as_str();
                            ui.add(
                                egui::TextEdit::multiline(&mut out_str)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_rows(3)
                                    .desired_width(f32::INFINITY),
                            );
                        }
                        if !self.editor.diagnostics().is_empty() {
                            ui.separator();
                            ui.small("Diagnostics:");
                            for diag in self.editor.diagnostics() {
                                ui.colored_label(VIOLET, format!("{:?}: {}", diag.severity, diag.message));
                            }
                        }
                    });
            }
            if self.search_open {
                let mut search_open = self.search_open;
                egui::Window::new("Find in Terminal")
                    .open(&mut search_open)
                    .collapsible(false)
                    .resizable(false)
                    .default_width(320.0)
                    .anchor(egui::Align2::RIGHT_TOP, [-12.0, self.header_height + 8.0])
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            let resp = ui.text_edit_singleline(&mut self.search_query);
                            if resp.changed() {
                                self.search_matches = 0;
                            }
                            if !self.search_query.is_empty() {
                                ui.label(format!("{} matches", self.search_matches));
                            }
                        });
                    });
                self.search_open = search_open;
            }
            if self.automation {
                egui::Window::new("Local automation").open(&mut self.automation).collapsible(false).default_width(440.0).show(ctx, |ui| {
                    if let Some(error) = &rpc.error { ui.colored_label(VIOLET, error); }
                    else { ui.label(format!("Connected clients: {}", rpc.stats.active_connections())); }
                    ui.label("Unix socket · same user only");
                    ui.monospace(rpc.path.display().to_string());
                    if ui.button("Copy socket path").clicked() { ui.output_mut(|o| o.copied_text = rpc.path.display().to_string()); }
                    ui.separator();
                    for method in crate::rpc::METHODS { ui.monospace(*method); }
                    ui.small("Shells receive WELL_RPC_SOCKET and WELL_SURFACE_ID.");
                    ui.small("send_text returns bytes queued to the selected shell, not command completion.");
                });
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        });
        let icon = match output.platform_output.cursor_icon {
            egui::CursorIcon::Text => CursorIcon::Text,
            egui::CursorIcon::PointingHand => CursorIcon::Pointer,
            egui::CursorIcon::ResizeHorizontal => CursorIcon::EwResize,
            egui::CursorIcon::ResizeVertical => CursorIcon::NsResize,
            _ => CursorIcon::Default,
        };
        window.set_cursor(icon);
        if let Some(cursor) = output
            .platform_output
            .ime
            .map(|ime| ime.cursor_rect)
            .or(terminal_cursor)
        {
            window.set_ime_cursor_area(
                winit::dpi::LogicalPosition::new(cursor.min.x as f64, cursor.min.y as f64),
                winit::dpi::LogicalSize::new(cursor.width() as f64, cursor.height() as f64),
            );
        }
        (output, candidate)
    }
    pub fn repaint_after(output: &egui::FullOutput) -> Option<Duration> {
        output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.repaint_delay)
            .filter(|d| *d < Duration::from_secs(3600))
    }
}
fn modifiers(m: ModifiersState) -> egui::Modifiers {
    egui::Modifiers {
        alt: m.alt_key(),
        ctrl: m.control_key(),
        shift: m.shift_key(),
        mac_cmd: cfg!(target_os = "macos") && m.super_key(),
        command: if cfg!(target_os = "macos") {
            m.super_key()
        } else {
            m.control_key()
        },
    }
}
fn egui_key(key: &Key) -> Option<egui::Key> {
    Some(match key {
        Key::Named(NamedKey::ArrowDown) => egui::Key::ArrowDown,
        Key::Named(NamedKey::ArrowUp) => egui::Key::ArrowUp,
        Key::Named(NamedKey::ArrowLeft) => egui::Key::ArrowLeft,
        Key::Named(NamedKey::ArrowRight) => egui::Key::ArrowRight,
        Key::Named(NamedKey::Escape) => egui::Key::Escape,
        Key::Named(NamedKey::Tab) => egui::Key::Tab,
        Key::Named(NamedKey::Backspace) => egui::Key::Backspace,
        Key::Named(NamedKey::Enter) => egui::Key::Enter,
        Key::Named(NamedKey::Delete) => egui::Key::Delete,
        Key::Named(NamedKey::Home) => egui::Key::Home,
        Key::Named(NamedKey::End) => egui::Key::End,
        Key::Named(NamedKey::PageUp) => egui::Key::PageUp,
        Key::Named(NamedKey::PageDown) => egui::Key::PageDown,
        Key::Named(NamedKey::Space) => egui::Key::Space,
        Key::Character(ch) => egui::Key::from_name(&ch.to_ascii_uppercase())?,
        _ => return None,
    })
}

#[allow(clippy::too_many_arguments)]
fn append_with_capital_highlight(
    job: &mut egui::text::LayoutJob,
    text: &str,
    base_color: Color32,
    capital_color: Color32,
    highlight_capitals: bool,
    multicolor_capitals: bool,
    start_byte_offset: usize,
    font_id: egui::FontId,
    bg_color: Color32,
) {
    if !highlight_capitals {
        job.append(
            text,
            0.0,
            egui::TextFormat {
                font_id,
                color: base_color,
                background: bg_color,
                ..Default::default()
            },
        );
        return;
    }

    let mut lower_start = None;

    for (idx, ch) in text.char_indices() {
        if ch.is_uppercase() {
            if let Some(start) = lower_start.take() {
                job.append(
                    &text[start..idx],
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: base_color,
                        background: bg_color,
                        ..Default::default()
                    },
                );
            }
            let color = if multicolor_capitals {
                let rgb = well_config::capital_color_at(
                    ch,
                    start_byte_offset + idx,
                    [capital_color.r(), capital_color.g(), capital_color.b()],
                    true,
                );
                Color32::from_rgb(rgb[0], rgb[1], rgb[2])
            } else {
                capital_color
            };
            job.append(
                &text[idx..idx + ch.len_utf8()],
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color,
                    background: bg_color,
                    ..Default::default()
                },
            );
        } else if lower_start.is_none() {
            lower_start = Some(idx);
        }
    }

    if let Some(start) = lower_start {
        job.append(
            &text[start..],
            0.0,
            egui::TextFormat {
                font_id,
                color: base_color,
                background: bg_color,
                ..Default::default()
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn mneme_syntax_layouter(
    ui: &egui::Ui,
    text: &str,
    wrap_width: f32,
    spans: &[well_editor::HighlightSpan],
    capital_color: Color32,
    highlight_capitals: bool,
    multicolor_capitals: bool,
    syntax_theme: &well_config::SyntaxTheme,
) -> std::sync::Arc<egui::Galley> {
    use well_editor::SyntaxClass;
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());

    let default_color = Color32::from_rgb(208, 216, 225);

    if spans.is_empty() {
        append_with_capital_highlight(
            &mut job,
            text,
            default_color,
            capital_color,
            highlight_capitals,
            multicolor_capitals,
            0,
            font_id,
            Color32::TRANSPARENT,
        );
        return ui.fonts(|f| f.layout_job(job));
    }

    let mut byte_idx = 0;
    for span in spans {
        if span.start_byte > byte_idx && span.start_byte <= text.len() {
            let unstyled = &text[byte_idx..span.start_byte];
            append_with_capital_highlight(
                &mut job,
                unstyled,
                default_color,
                capital_color,
                highlight_capitals,
                multicolor_capitals,
                byte_idx,
                font_id.clone(),
                Color32::TRANSPARENT,
            );
            byte_idx = span.start_byte;
        }

        if span.start_byte < text.len()
            && span.end_byte <= text.len()
            && span.start_byte < span.end_byte
        {
            let chunk = &text[span.start_byte..span.end_byte];
            let color = match span.kind {
                SyntaxClass::Command => Color32::from_rgb(
                    syntax_theme.command[0],
                    syntax_theme.command[1],
                    syntax_theme.command[2],
                ),
                SyntaxClass::Builtin => Color32::from_rgb(
                    syntax_theme.builtin[0],
                    syntax_theme.builtin[1],
                    syntax_theme.builtin[2],
                ),
                SyntaxClass::ValidPath => Color32::from_rgb(
                    syntax_theme.valid_path[0],
                    syntax_theme.valid_path[1],
                    syntax_theme.valid_path[2],
                ),
                SyntaxClass::InvalidPath => Color32::from_rgb(
                    syntax_theme.invalid_path[0],
                    syntax_theme.invalid_path[1],
                    syntax_theme.invalid_path[2],
                ),
                SyntaxClass::Keyword => Color32::from_rgb(
                    syntax_theme.keyword[0],
                    syntax_theme.keyword[1],
                    syntax_theme.keyword[2],
                ),
                SyntaxClass::String => Color32::from_rgb(
                    syntax_theme.string[0],
                    syntax_theme.string[1],
                    syntax_theme.string[2],
                ),
                SyntaxClass::Number => Color32::from_rgb(
                    syntax_theme.number[0],
                    syntax_theme.number[1],
                    syntax_theme.number[2],
                ),
                SyntaxClass::Comment => Color32::from_rgb(
                    syntax_theme.comment[0],
                    syntax_theme.comment[1],
                    syntax_theme.comment[2],
                ),
                SyntaxClass::Parameter => Color32::from_rgb(
                    syntax_theme.parameter[0],
                    syntax_theme.parameter[1],
                    syntax_theme.parameter[2],
                ),
                SyntaxClass::Operator => Color32::from_rgb(
                    syntax_theme.operator[0],
                    syntax_theme.operator[1],
                    syntax_theme.operator[2],
                ),
                SyntaxClass::Delimiter => Color32::from_rgb(
                    syntax_theme.delimiter[0],
                    syntax_theme.delimiter[1],
                    syntax_theme.delimiter[2],
                ),
                SyntaxClass::Variable => Color32::from_rgb(
                    syntax_theme.variable[0],
                    syntax_theme.variable[1],
                    syntax_theme.variable[2],
                ),
                SyntaxClass::InvalidSyntax | SyntaxClass::UnclosedDelimiter => Color32::from_rgb(
                    syntax_theme.error[0],
                    syntax_theme.error[1],
                    syntax_theme.error[2],
                ),
                SyntaxClass::Destructive => Color32::from_rgb(255, 50, 50),
            };

            let bg_color = match span.kind {
                SyntaxClass::Destructive => Color32::from_rgb(80, 15, 15),
                SyntaxClass::InvalidSyntax | SyntaxClass::UnclosedDelimiter => {
                    Color32::from_rgb(60, 20, 20)
                }
                _ => Color32::TRANSPARENT,
            };

            append_with_capital_highlight(
                &mut job,
                chunk,
                color,
                capital_color,
                highlight_capitals,
                multicolor_capitals,
                span.start_byte,
                font_id.clone(),
                bg_color,
            );
            byte_idx = span.end_byte;
        }
    }

    if byte_idx < text.len() {
        append_with_capital_highlight(
            &mut job,
            &text[byte_idx..],
            default_color,
            capital_color,
            highlight_capitals,
            multicolor_capitals,
            byte_idx,
            font_id,
            Color32::TRANSPARENT,
        );
    }

    ui.fonts(|f| f.layout_job(job))
}

fn geometry(p: &egui::Painter, r: Rect) {
    let step = 20.0;
    let band = 11.0;
    let mut i = 0;
    let mut y = r.top();
    while y < r.bottom() - band {
        let end = (y + step).min(r.bottom() - band);
        let color = if i % 2 == 0 { CYAN } else { VIOLET };
        p.add(egui::Shape::closed_line(
            vec![
                Pos2::new(r.left(), y),
                Pos2::new(r.left() + band, (y + end) / 2.0),
                Pos2::new(r.left(), end),
            ],
            Stroke::new(0.75_f32, color),
        ));
        p.add(egui::Shape::closed_line(
            vec![
                Pos2::new(r.right(), y),
                Pos2::new(r.right() - band, (y + end) / 2.0),
                Pos2::new(r.right(), end),
            ],
            Stroke::new(0.75_f32, color),
        ));
        y = end;
        i += 1;
    }
    let mut x = r.left();
    while x < r.right() {
        let end = (x + step).min(r.right());
        p.add(egui::Shape::closed_line(
            vec![
                Pos2::new(x, r.bottom() - band),
                Pos2::new((x + end) / 2.0, r.bottom()),
                Pos2::new(end, r.bottom() - band),
            ],
            Stroke::new(0.75_f32, if i % 2 == 0 { CYAN } else { VIOLET }),
        ));
        x = end;
        i += 1;
    }
    let inner_left = r.left() + band;
    let inner_right = r.right() - band;
    let inner_bottom = r.bottom() - band;
    let inner_top = r.top();

    // Inner guide lines along left, right, and bottom (no top horizontal stroke cutting through workspace)
    p.line_segment(
        [
            Pos2::new(inner_left, inner_top),
            Pos2::new(inner_left, inner_bottom),
        ],
        Stroke::new(0.5_f32, CYAN.gamma_multiply(0.7)),
    );
    p.line_segment(
        [
            Pos2::new(inner_right, inner_top),
            Pos2::new(inner_right, inner_bottom),
        ],
        Stroke::new(0.5_f32, CYAN.gamma_multiply(0.7)),
    );
    p.line_segment(
        [
            Pos2::new(inner_left, inner_bottom),
            Pos2::new(inner_right, inner_bottom),
        ],
        Stroke::new(0.5_f32, CYAN.gamma_multiply(0.7)),
    );
}
