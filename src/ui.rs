//! Small native-window UI layer, with explicit input routing above the terminal.
use crate::workspace::{Action, TabInfo};
use egui::{Color32, Event, Pos2, Rect, Stroke, Vec2};
use std::time::{Duration, Instant};
use well_config::{Preferences, TheiasPrismPanel};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Window};

const HEADER: f32 = 42.0;
const TABSTRIP: f32 = 28.0;
const CYAN: Color32 = Color32::from_rgb(38, 195, 247);
const VIOLET: Color32 = Color32::from_rgb(181, 112, 246);

pub struct Gui {
    pub ctx: egui::Context,
    pub renderer: egui_wgpu::Renderer,
    pub settings: TheiasPrismPanel,
    raw: egui::RawInput,
    pointer: Pos2,
    modifiers: ModifiersState,
    started: Instant,
    pub actions: Vec<Action>,
    automation: bool,
    header_height: f32,
}
impl Gui {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, prefs: &Preferences) -> Self {
        let ctx = egui::Context::default();
        ctx.style_mut(|style| {
            style.visuals = egui::Visuals::dark();
            style.visuals.window_fill = Color32::from_rgb(16, 19, 29);
            style.visuals.panel_fill = Color32::from_rgb(14, 16, 25);
            style.visuals.override_text_color = Some(Color32::from_rgb(190, 203, 219));
            style.visuals.window_stroke = Stroke::new(1.0_f32, CYAN.gamma_multiply(0.65));
            style.visuals.selection.bg_fill = Color32::from_rgb(50, 63, 93);
            style.visuals.selection.stroke = Stroke::new(1.0_f32, CYAN);
            style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, CYAN);
            style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, VIOLET);
            style.visuals.window_rounding = 10.0.into();
            style.visuals.widgets.inactive.rounding = 6.0.into();
        });
        Self {
            ctx,
            renderer: egui_wgpu::Renderer::new(device, format, None, 1),
            settings: TheiasPrismPanel::new(prefs),
            raw: egui::RawInput::default(),
            pointer: Pos2::ZERO,
            modifiers: ModifiersState::empty(),
            started: Instant::now(),
            actions: Vec::new(),
            automation: false,
            header_height: HEADER,
        }
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
        let top = ((HEADER + if tab_count > 1 { TABSTRIP } else { 0.0 }) * scale).round() as u32;
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
        let keyboard = self.settings.open || self.automation;
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
    pub fn frame(
        &mut self,
        window: &Window,
        prefs: &Preferences,
        path: &std::path::Path,
        exited: bool,
        selecting: bool,
        preedit: &str,
        terminal_cursor: Option<Rect>,
        tabs: &[TabInfo],
        active_tab: usize,
        rpc: &crate::rpc::RpcHost,
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
        let raw = self.raw.take();
        let mut candidate = None;
        let output = self.ctx.run(raw, |ctx| {
            egui::TopBottomPanel::top("well_header")
                .exact_height(HEADER)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("⟳").size(25.0).color(VIOLET));
                        ui.label(egui::RichText::new("WELL").size(16.0));
                        egui::Frame::none()
                            .stroke(Stroke::new(1.0_f32, CYAN))
                            .rounding(6.0)
                            .inner_margin(egui::Margin::symmetric(7.0, 3.0))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("SHELL").size(11.0).color(CYAN));
                            });
                        if ui.add_enabled(tabs.len() < crate::workspace::MAX_TABS, egui::Button::new("+").small()).on_hover_text("New tab (Cmd+T / Ctrl+Shift+T)").clicked() { self.actions.push(Action::NewTab); }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            if ui
                                .add(
                                    egui::Button::new("Settings")
                                        .stroke(Stroke::new(1.0_f32, CYAN))
                                        .rounding(10.0),
                                )
                                .clicked()
                            {
                                self.settings.toggle(prefs);
                                self.automation = false;
                            }
                            if ui.small_button("Automation").clicked() { self.automation = !self.automation; self.settings.open = false; }
                            ui.small("UTF-8");
                            if selecting { ui.small("Selection").on_hover_text("Output continues in the background. Press Escape or type to return to live output."); }
                            let (dot_rect, _) =
                                ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                            ui.painter().circle_filled(
                                dot_rect.center(),
                                2.0,
                                if exited {
                                    VIOLET
                                } else {
                                    Color32::from_rgb(100, 245, 97)
                                },
                            );
                            if exited {
                                ui.small("Exited");
                            }
                        });
                    });
                });
            if tabs.len() > 1 {
                egui::TopBottomPanel::top("well_tabs").exact_height(TABSTRIP).show(ctx, |ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| { ui.horizontal(|ui| {
                        for (index, tab) in tabs.iter().enumerate() {
                            let title = format!("Shell {}{}", tab.id, tab.exit_status.map_or(String::new(), |code| format!(" · {code}")));
                            if ui.selectable_label(index == active_tab, title).clicked() { self.actions.push(Action::SwitchTab(index)); }
                            if index == active_tab && ui.small_button("×").on_hover_text("Close this tab and stop its shell").clicked() { self.actions.push(Action::CloseTab); }
                            ui.separator();
                        }
                    }); });
                });
            }
            let rect = ctx.screen_rect();
            let painter = ctx.layer_painter(egui::LayerId::background());
            painter.line_segment(
                [Pos2::new(0.0, self.header_height), Pos2::new(rect.width(), self.header_height)],
                Stroke::new(0.75_f32, Color32::from_gray(180)),
            );
            if prefs.appearance.geometric_border {
                geometry(
                    &painter,
                    Rect::from_min_max(Pos2::new(0.5, self.header_height), rect.max - Vec2::splat(0.5)),
                );
            }
            if !self.settings.open && !self.automation && !preedit.is_empty() {
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
            }
            if self.settings.open {
                // Settings owns input until dismissed. Keep the terminal visible behind it.
                candidate = self.settings.draw(ctx, prefs, path);
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
    p.rect_stroke(
        r.shrink2(Vec2::new(band, 0.0))
            .shrink2(Vec2::new(0.0, band / 2.0))
            .translate(Vec2::new(0.0, -band / 2.0)),
        2.0,
        Stroke::new(0.5_f32, CYAN),
    );
}
