//! well-config: Theia's Prism Immediate-Mode Configuration Dashboard
//!
//! Subsystems:
//! - TheiasPrismPanel: Comprehensive immediate-mode settings GUI built on egui for
//!   GPU-accelerated terminal configuration synchronized natively over Hermes Seqlock.
//! - Hyprlang: Native configuration parser and emitter for Hyprlang-syntax (.hl) configs.

pub mod hyprlang;
pub use hyprlang::{HyprlangDocument, HyprlangEmitter, HyprlangParser};

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use egui::{pos2, Color32, Context, Rect, RichText, ScrollArea, Slider, Stroke, Ui};
use serde::{Deserialize, Serialize};
use well_ipc::{HermesChannel, TheiaConfigPayload};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShellAbbreviation {
    pub keyword: String,
    pub expansion: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeybindingRow {
    pub chord: String,
    pub action: String,
    pub conflict: bool,
}

pub const TOP_BAR_HEIGHT: f32 = 36.0;
pub const FRAME_MARGIN: f32 = 16.0;

#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowControlAction {
    None,
    Close,
    Minimize,
    ToggleFullscreen,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentEvent {
    pub event_type: String,
    pub payload: serde_json::Value,
    #[serde(skip)]
    pub timestamp: Option<Instant>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentConfig {
    pub theia: TheiaConfigPayload,
    pub shell_path: String,
    pub active_font_name: String,
    pub abbreviations: Vec<ShellAbbreviation>,
    pub keybindings: Vec<KeybindingRow>,
    pub hf_token: Option<String>,
}

pub struct TheiasPrismPanel {
    pub channel: Arc<HermesChannel<TheiaConfigPayload>>,
    pub config: TheiaConfigPayload,
    pub shell_path: String,
    pub active_font_name: String,
    pub abbreviations: Vec<ShellAbbreviation>,
    pub new_keyword: String,
    pub new_expansion: String,
    pub keybindings: Vec<KeybindingRow>,
    pub new_chord: String,
    pub new_action: String,
    pub active_tab: usize,
    pub is_open: bool,
    pub status_notification: Option<(String, Instant)>,
    pub pending_window_action: WindowControlAction,
    // Undo/Redo stacks for config snapshots
    pub undo_stack: Vec<TheiaConfigPayload>,
    pub redo_stack: Vec<TheiaConfigPayload>,
    // Current profile name (if any)
    pub current_profile: Option<String>,
    // Pythia LLM Translation HUD state
    pub show_pythia_hud: bool,
    pub pythia_query: String,
    pub pythia_result: Option<well_llm::TranslationResult>,
    pub pythia_mode: u8, // 0 = Natural Language -> Shell, 1 = Error Diagnosis
    pub pythia_image_dim: (u32, u32),
    pub pending_pty_injection: Option<String>,
    // URI of the last displayed image — we call ctx.forget_image() on it
    // before loading a new one to bypass egui's texture cache.
    pub last_image_uri: Option<String>,
    // Hugging Face token (optional, overrides env var if set)
    pub hf_token: String,
    // AI Agent Lifecycle Events for BlockList
    pub agent_events: Vec<AgentEvent>,
}

impl TheiasPrismPanel {
    pub fn new(channel: Arc<HermesChannel<TheiaConfigPayload>>) -> Self {
        let initial_config = channel.read_state();
        let initial_abbrevs = vec![
            ShellAbbreviation { keyword: "gco".to_string(), expansion: "git checkout".to_string() },
            ShellAbbreviation { keyword: "lg".to_string(), expansion: "lazygit".to_string() },
            ShellAbbreviation { keyword: "hx".to_string(), expansion: "helix".to_string() },
            ShellAbbreviation { keyword: "gp".to_string(), expansion: "git push".to_string() },
            ShellAbbreviation { keyword: "ll".to_string(), expansion: "ls -la".to_string() },
        ];

        let initial_bindings = vec![
            KeybindingRow { chord: "Cmd+,".to_string(), action: "Toggle Settings Menu".to_string(), conflict: false },
            KeybindingRow { chord: "F12".to_string(), action: "Toggle Settings Menu".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+I / Ctrl+I".to_string(), action: "Toggle Pythia AI Translator".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+Shift+P / Cmd+P".to_string(), action: "Open Command Palette".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+A / Ctrl+Shift+A".to_string(), action: "Select All Terminal Text".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+D".to_string(), action: "Split Pane Horizontal".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+Shift+D".to_string(), action: "Split Pane Vertical".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+F".to_string(), action: "Find in Scrollback".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+K".to_string(), action: "Clear Buffer".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+=".to_string(), action: "Increase Font Size".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+-".to_string(), action: "Decrease Font Size".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+0".to_string(), action: "Reset Font Size".to_string(), conflict: false },
        ];

        let shell_path = initial_config.shell_path.clone();

        let active_font_name = if initial_config.font_name == "System Monospace" {
            "JetBrainsMono Nerd Font (Active)".to_string()
        } else {
            "OpenDyslexic Nerd Font (Active)".to_string()
        };

        let mut panel = Self {
            channel,
            config: initial_config,
            shell_path,
            active_font_name,
            abbreviations: initial_abbrevs,
            new_keyword: String::new(),
            new_expansion: String::new(),
            keybindings: initial_bindings,
            new_chord: String::new(),
            new_action: String::new(),
            active_tab: 0,
            is_open: false, // Closed by default on launch for clean workspace
            status_notification: None,
            pending_window_action: WindowControlAction::None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_profile: None,
            show_pythia_hud: false,
            pythia_query: String::new(),
            pythia_result: None,
            pythia_mode: 0,
            pythia_image_dim: (1024, 1024),
            pending_pty_injection: None,
            last_image_uri: None,
            // Initialize HF token from environment variable if present
            hf_token: std::env::var("HF_TOKEN").unwrap_or_default(),
            agent_events: Vec::new(),
        };

        // Attempt to load saved config from disk if available
        panel.try_load_from_disk();
        panel
    }

    pub fn set_notification(&mut self, text: impl Into<String>) {
        self.status_notification = Some((text.into(), Instant::now()));
    }

    pub fn config_path() -> Option<PathBuf> {
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok()?;
        Some(PathBuf::from(home).join(".config").join("well").join("config.json"))
    }

    pub fn hyprlang_path() -> Option<PathBuf> {
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok()?;
        Some(PathBuf::from(home).join(".config").join("well").join("well.hl"))
    }

    pub fn save_to_disk(&mut self) -> Result<(), String> {
        let path = Self::config_path().ok_or("Could not determine HOME directory")?;
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let persistent = PersistentConfig {
            theia: self.config.clone(),
            shell_path: self.shell_path.clone(),
            active_font_name: self.active_font_name.clone(),
            abbreviations: self.abbreviations.clone(),
            keybindings: self.keybindings.clone(),
            hf_token: Some(self.hf_token.clone()),
        };

        let json = serde_json::to_string_pretty(&persistent)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        std::fs::write(&path, json)
            .map_err(|e| format!("Failed to write to {}: {}", path.display(), e))?;

        self.channel.sync_state(self.config.clone());
        self.status_notification = Some((format!("Saved to {}", path.display()), Instant::now()));
        Ok(())
    }

    pub fn try_load_from_disk(&mut self) {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                if let Ok(data) = std::fs::read_to_string(&path) {
                    if let Ok(persistent) = serde_json::from_str::<PersistentConfig>(&data) {
                        self.config = persistent.theia;
                        self.shell_path = persistent.shell_path;
                        self.active_font_name = persistent.active_font_name;
                        self.abbreviations = persistent.abbreviations;
                        self.keybindings = persistent.keybindings;
                        // Load HF token if persisted, otherwise fall back to env var (already set in new())
                        self.hf_token = persistent.hf_token.unwrap_or_else(|| std::env::var("HF_TOKEN").unwrap_or_default());
                        self.channel.sync_state(self.config.clone());
                    }
                }
            }
        }
    }

    pub fn reset_to_defaults(&mut self) {
        self.config = TheiaConfigPayload::default();
        self.active_font_name = "OpenDyslexic Nerd Font (Active)".to_string();
        self.channel.sync_state(self.config.clone());
        self.status_notification = Some(("Reset to factory defaults".to_string(), Instant::now()));
        // Clear undo/redo history on reset
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.current_profile = None;
    }

    pub fn render_window(&mut self, ctx: &Context) {
        // 1. Configure dark glassmorphic styling on egui context
        let mut visuals = egui::Visuals::dark();
        visuals.window_fill = Color32::from_rgba_premultiplied(13, 16, 24, 242);
        visuals.window_stroke = Stroke::new(1.0f32, Color32::from_rgba_premultiplied(56, 189, 248, 80));
        visuals.window_rounding = egui::Rounding::same(14.0);
        visuals.window_shadow = egui::epaint::Shadow {
            offset: egui::vec2(0.0, 8.0),
            blur: 24.0,
            spread: 0.0,
            color: Color32::from_black_alpha(220),
        };
        visuals.panel_fill = Color32::from_rgba_premultiplied(18, 22, 32, 245);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgba_premultiplied(22, 28, 40, 190);
        visuals.widgets.noninteractive.rounding = egui::Rounding::same(6.0);
        visuals.widgets.inactive.bg_fill = Color32::from_rgba_premultiplied(26, 33, 48, 200);
        visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
        visuals.widgets.hovered.bg_fill = Color32::from_rgba_premultiplied(38, 48, 70, 255);
        visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0f32, Color32::from_rgb(57, 255, 20));
        visuals.widgets.active.bg_fill = Color32::from_rgba_premultiplied(48, 62, 88, 255);
        visuals.widgets.active.rounding = egui::Rounding::same(6.0);
        visuals.selection.bg_fill = Color32::from_rgba_premultiplied(57, 255, 20, 50);
        visuals.selection.stroke = Stroke::new(1.0f32, Color32::from_rgb(57, 255, 20));
        ctx.set_visuals(visuals);

        self.render_agent_sidebar(ctx);

        let mut toggle_clicked = false;
        let screen_rect = ctx.screen_rect();
        let screen_w = screen_rect.width();
        let _screen_h = screen_rect.height();
        let top_bar_h = TOP_BAR_HEIGHT;
        let margin = FRAME_MARGIN;

        // 1. Draw Oroboros scale frame around the terminal viewport
        let bg_painter = ctx.layer_painter(egui::LayerId::background());
        draw_oroboros_scale_frame(&bg_painter, screen_rect, top_bar_h, margin);

        // 2. Defined Full-Width Top Bar
        let top_bar_rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(screen_w, top_bar_h));

        // Dark obsidian header fill with subtle top rounding
        bg_painter.rect_filled(
            top_bar_rect,
            egui::Rounding { nw: 10.0, ne: 10.0, sw: 0.0, se: 0.0 },
            Color32::from_rgba_premultiplied(12, 16, 24, 252),
        );

        // Defined bottom divider line separating top bar from terminal
        bg_painter.line_segment(
            [pos2(0.0, top_bar_h), pos2(screen_w, top_bar_h)],
            Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(255, 255, 255, 18)),
        );

        // Central glowing cyber accent seam (electric violet blending into neon cyan)
        let cx = screen_w * 0.5;
        let seam_half_w = (screen_w * 0.16).clamp(50.0, 140.0);
        bg_painter.line_segment(
            [pos2(cx - seam_half_w, top_bar_h), pos2(cx, top_bar_h)],
            Stroke::new(1.5_f32, Color32::from_rgba_premultiplied(168, 85, 247, 190)),
        );
        bg_painter.line_segment(
            [pos2(cx, top_bar_h), pos2(cx + seam_half_w, top_bar_h)],
            Stroke::new(1.5_f32, Color32::from_rgba_premultiplied(56, 189, 248, 190)),
        );

        // Top bar interactive UI layer: Single unified area across top
        egui::Area::new(egui::Id::new("top_bar_unified_area"))
            .fixed_pos(pos2(12.0, 5.0))
            .show(ctx, |ui| {
                ui.set_width(screen_w - 24.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
                    ui.label(RichText::new("⟳").size(15.0).color(Color32::from_rgb(192, 132, 252)).strong());
                    ui.label(RichText::new("WELL").size(13.0).color(Color32::from_rgb(240, 245, 255)).strong());
                    egui::Frame::none()
                        .fill(Color32::from_rgba_premultiplied(22, 28, 42, 180))
                        .stroke(Stroke::new(0.8_f32, Color32::from_rgba_premultiplied(56, 189, 248, 80)))
                        .rounding(egui::Rounding::same(4.0))
                        .inner_margin(egui::Margin::symmetric(5.0, 1.5))
                        .show(ui, |ui| {
                            ui.label(RichText::new("SHELL").size(9.5).color(Color32::from_rgb(140, 165, 195)).strong());
                        });

                    let ai_btn = egui::Button::new(RichText::new("✦ Pythia AI").size(10.0).color(Color32::from_rgb(192, 132, 252)).strong())
                        .fill(Color32::from_rgba_premultiplied(28, 18, 44, 200))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(168, 85, 247, 120)))
                        .rounding(egui::Rounding::same(4.0));
                    if ui.add(ai_btn).on_hover_text("Open Pythia Shell Copilot (Cmd+I)").clicked() {
                        self.show_pythia_hud = !self.show_pythia_hud;
                    }

                    let img_btn = egui::Button::new(
                        RichText::new("🖼 Image Gen")
                            .size(10.5)
                            .color(Color32::from_rgb(56, 189, 248))
                            .strong(),
                    )
                    .fill(Color32::from_rgba_premultiplied(16, 36, 60, 230))
                    .stroke(Stroke::new(1.2_f32, Color32::from_rgb(56, 189, 248)))
                    .rounding(egui::Rounding::same(6.0));
                    if ui.add(img_btn).on_hover_text("Open Cybernetic Image Studio (Cmd+I)").clicked() {
                        self.show_pythia_hud = true;
                        if self.pythia_query.trim().is_empty() {
                            self.pythia_query = "/image synthwave neon grid wireframe sunset".to_string();
                        }
                    }

                    // Right-aligned controls
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (btn_text, text_color, border_color) = if self.is_open {
                            ("✕ Close (Esc)", Color32::from_rgb(255, 100, 130), Color32::from_rgba_premultiplied(255, 80, 120, 130))
                        } else {
                            ("⚙ Settings", Color32::from_rgb(180, 205, 230), Color32::from_rgba_premultiplied(56, 189, 248, 80))
                        };
                        let btn = egui::Button::new(RichText::new(btn_text).size(11.0).strong().color(text_color))
                            .fill(Color32::from_rgba_premultiplied(16, 21, 32, 220))
                            .stroke(Stroke::new(1.0f32, border_color))
                            .rounding(egui::Rounding::same(8.0));
                        if ui.add(btn).clicked() {
                            toggle_clicked = true;
                        }

                        ui.label(RichText::new("UTF-8").size(9.5).color(Color32::from_rgb(100, 120, 150)));
                        ui.label(RichText::new("●").size(8.0).color(Color32::from_rgb(57, 255, 20)));

                        // Window Control Capsule (─, □, ✕)
                        egui::Frame::none()
                            .fill(Color32::from_rgba_premultiplied(14, 18, 28, 180))
                            .stroke(Stroke::new(0.8_f32, Color32::from_rgba_premultiplied(255, 255, 255, 18)))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::symmetric(3.0, 1.5))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
                                    // Close Button
                                    let (rect_close, resp_close) = ui.allocate_exact_size(egui::vec2(20.0, 16.0), egui::Sense::click());
                                    if resp_close.hovered() {
                                        ui.painter().rect_filled(rect_close, egui::Rounding::same(4.0), Color32::from_rgba_premultiplied(239, 68, 68, 35));
                                    }
                                    ui.painter().text(rect_close.center(), egui::Align2::CENTER_CENTER, "✕", egui::FontId::monospace(9.5), if resp_close.hovered() { Color32::from_rgb(248, 113, 113) } else { Color32::from_rgba_premultiplied(135, 150, 175, 170) });
                                    if resp_close.on_hover_text("Close Well (Cmd+Q)").clicked() {
                                        self.pending_window_action = WindowControlAction::Close;
                                    }

                                    // Maximize Button
                                    let (rect_fs, resp_fs) = ui.allocate_exact_size(egui::vec2(20.0, 16.0), egui::Sense::click());
                                    if resp_fs.hovered() {
                                        ui.painter().rect_filled(rect_fs, egui::Rounding::same(4.0), Color32::from_rgba_premultiplied(168, 85, 247, 28));
                                    }
                                    ui.painter().text(rect_fs.center(), egui::Align2::CENTER_CENTER, "□", egui::FontId::monospace(10.0), if resp_fs.hovered() { Color32::from_rgb(192, 132, 252) } else { Color32::from_rgba_premultiplied(135, 150, 175, 170) });
                                    if resp_fs.on_hover_text("Toggle Fullscreen (Ctrl+Cmd+F)").clicked() {
                                        self.pending_window_action = WindowControlAction::ToggleFullscreen;
                                    }

                                    // Minimize Button
                                    let (rect_min, resp_min) = ui.allocate_exact_size(egui::vec2(20.0, 16.0), egui::Sense::click());
                                    if resp_min.hovered() {
                                        ui.painter().rect_filled(rect_min, egui::Rounding::same(4.0), Color32::from_rgba_premultiplied(56, 189, 248, 28));
                                    }
                                    ui.painter().text(rect_min.center(), egui::Align2::CENTER_CENTER, "─", egui::FontId::monospace(9.5), if resp_min.hovered() { Color32::from_rgb(56, 189, 248) } else { Color32::from_rgba_premultiplied(135, 150, 175, 170) });
                                    if resp_min.on_hover_text("Minimize (Cmd+M)").clicked() {
                                        self.pending_window_action = WindowControlAction::Minimize;
                                    }
                                });
                            });
                    });
                });
            });

        if toggle_clicked {
            self.is_open = !self.is_open;
        }

        if !self.is_open {
            self.render_pythia_hud(ctx);
            return;
        }

        let mut is_open = self.is_open;
        let mut close_clicked = false;
        let mut apply_clicked = false;
        let mut save_clicked = false;
        let mut reset_clicked = false;
        let mut undo_clicked = false;
        let mut redo_clicked = false;

        egui::Window::new(
            RichText::new("WELL // CONTROL CENTER")
                .color(Color32::from_rgb(34, 180, 50))
                .strong()
                .size(13.5),
        )
        .id(egui::Id::new("theia_control_center_window"))
        .open(&mut is_open)
        .default_width(760.0)
        .default_height(580.0)
        .min_width(620.0)
        .min_height(440.0)
        .resizable(true)
        .movable(true)
        .collapsible(false)
        .default_pos(egui::pos2(100.0, 50.0))
        .show(ctx, |ui| {
            // Sleek Two-Column Category Rail Navigation (No Tabs)
            let sidebar_w = 170.0;
            let available_h = ui.available_height() - 48.0;

            ui.horizontal(|ui| {
                // Left Column: Category Sidebar Rail
                ui.allocate_ui_with_layout(
                    egui::vec2(sidebar_w, available_h),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        let categories: [(usize, &str, Color32); 12] = [
                            (0, "🎨  Appearance", Color32::from_rgb(57, 255, 20)),
                            (1, "🔤  Typography", Color32::from_rgb(56, 189, 248)),
                            (2, "⌨  Shortcuts", Color32::from_rgb(250, 204, 21)),
                            (3, "🐚  Metis Shell", Color32::from_rgb(255, 0, 127)),
                            (4, "📺  CRT Shaders", Color32::from_rgb(168, 85, 247)),
                            (5, "📑  Profiles & HL", Color32::from_rgb(0, 240, 255)),
                            (6, "⚡  Engine Core", Color32::from_rgb(57, 255, 20)),
                            (7, "📖  Codex & Docs", Color32::from_rgb(148, 163, 184)),
                            (8, "📐  Architecture", Color32::from_rgb(14, 165, 233)),
                            (9, "⏱  Milestones", Color32::from_rgb(244, 63, 94)),
                            (10, "✦  Pythia AI", Color32::from_rgb(192, 132, 252)),
                            (11, "🖼  Image Studio", Color32::from_rgb(56, 189, 248)),
                        ];

                        ScrollArea::vertical()
                            .id_salt("theia_category_sidebar")
                            .auto_shrink([false, false])
                            .max_width(sidebar_w)
                            .show(ui, |ui| {
                                for (idx, label, accent) in categories {
                                    let is_active = self.active_tab == idx;
                                    let (fill, stroke, text_color) = if is_active {
                                        (
                                            Color32::from_rgba_premultiplied(accent.r() / 6, accent.g() / 6, accent.b() / 6, 230),
                                            Stroke::new(1.2f32, accent),
                                            accent,
                                        )
                                    } else {
                                        (
                                            Color32::from_rgba_premultiplied(16, 22, 34, 150),
                                            Stroke::new(1.0f32, Color32::from_rgba_premultiplied(255, 255, 255, 15)),
                                            Color32::from_rgb(148, 163, 184),
                                        )
                                    };

                                    let btn = egui::Button::new(
                                        RichText::new(label)
                                            .size(11.5)
                                            .strong()
                                            .color(text_color)
                                    )
                                    .fill(fill)
                                    .stroke(stroke)
                                    .rounding(egui::Rounding::same(6.0));

                                    if ui.add_sized([sidebar_w - 8.0, 30.0], btn).clicked() {
                                        self.active_tab = idx;
                                    }
                                    ui.add_space(2.0);
                                }
                            });
                    },
                );

                ui.separator();

                // Right Column: Category Content View
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), available_h),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ScrollArea::vertical()
                            .id_salt("theia_category_content_area")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                match self.active_tab {
                                    0 => self.render_appearance_tab(ui),
                                    1 => self.render_typography_tab(ui),
                                    2 => self.render_shortcuts_tab(ui),
                                    3 => self.render_shell_tab(ui),
                                    4 => self.render_shaders_tab(ui),
                                    5 => self.render_profiles_tab(ui),
                                    6 => self.render_script_tab(ui),
                                    7 => self.render_help_tab(ui),
                                    8 => self.render_outline_tab(ui),
                                    9 => self.render_timeline_tab(ui),
                                    10 => self.render_pythia_tab(ui),
                                    11 => self.render_image_studio_tab(ui),
                                    _ => self.render_appearance_tab(ui),
                                }
                            });
                    },
                );
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            // Bottom Action & Telemetry Bar
            ui.horizontal(|ui| {
                let apply_btn = egui::Button::new(RichText::new("⚡ Apply Live").color(Color32::from_rgb(57, 255, 20)).strong())
                    .fill(Color32::from_rgba_premultiplied(20, 50, 30, 220))
                    .stroke(Stroke::new(1.0f32, Color32::from_rgb(57, 255, 20)))
                    .rounding(egui::Rounding::same(6.0));
                if ui.add(apply_btn).clicked() {
                    apply_clicked = true;
                }

                let save_btn = egui::Button::new(RichText::new("💾 Save Config").color(Color32::from_rgb(56, 189, 248)))
                    .fill(Color32::from_rgba_premultiplied(20, 40, 60, 220))
                    .stroke(Stroke::new(1.0f32, Color32::from_rgb(56, 189, 248)))
                    .rounding(egui::Rounding::same(6.0));
                if ui.add(save_btn).clicked() {
                    save_clicked = true;
                }

                let can_undo = !self.undo_stack.is_empty();
                let can_redo = !self.redo_stack.is_empty();
                ui.add_enabled_ui(can_undo, |ui| {
                    if ui.button("↶ Undo").clicked() {
                        undo_clicked = true;
                    }
                });
                ui.add_enabled_ui(can_redo, |ui| {
                    if ui.button("↷ Redo").clicked() {
                        redo_clicked = true;
                    }
                });

                let reset_btn = egui::Button::new(RichText::new("↺ Reset").color(Color32::from_rgb(255, 100, 130)))
                    .fill(Color32::from_rgba_premultiplied(40, 20, 30, 200))
                    .stroke(Stroke::new(1.0f32, Color32::from_rgba_premultiplied(255, 100, 130, 120)))
                    .rounding(egui::Rounding::same(6.0));
                if ui.add(reset_btn).clicked() {
                    reset_clicked = true;
                }

                let studio_btn = egui::Button::new(RichText::new("🖼 Image Studio").color(Color32::from_rgb(56, 189, 248)).strong())
                    .fill(Color32::from_rgba_premultiplied(16, 32, 54, 220))
                    .stroke(Stroke::new(1.0f32, Color32::from_rgba_premultiplied(56, 189, 248, 160)))
                    .rounding(egui::Rounding::same(6.0));
                if ui.add(studio_btn).on_hover_text("Jump to Cybernetic Image Studio Tab").clicked() {
                    self.active_tab = 11;
                }

                let close_btn = egui::Button::new(RichText::new("✕ Close (Esc)").color(Color32::from_rgb(255, 120, 140)).strong())
                    .fill(Color32::from_rgba_premultiplied(45, 20, 30, 220))
                    .stroke(Stroke::new(1.0f32, Color32::from_rgba_premultiplied(255, 120, 140, 150)))
                    .rounding(egui::Rounding::same(6.0));
                if ui.add(close_btn).clicked() {
                    close_clicked = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((msg, created)) = &self.status_notification {
                        if created.elapsed().as_secs() < 4 {
                            ui.label(RichText::new(format!("● {}", msg)).color(Color32::from_rgb(57, 255, 20)).size(11.5));
                        } else {
                            ui.label(RichText::new("● Hermes Seqlock Active").color(Color32::from_rgb(100, 120, 140)).size(11.5));
                        }
                    } else {
                        let seq = self.channel.sequence();
                        ui.label(RichText::new(format!("● Hermes Seqlock Active (Seq #{})", seq)).color(Color32::from_rgb(57, 255, 20)).size(11.5));
                    }
                });
            });
        });

        if undo_clicked {
            if let Some(prev) = self.undo_stack.pop() {
                self.redo_stack.push(self.config.clone());
                self.config = prev;
                self.channel.sync_state(self.config.clone());
            }
        }
        if redo_clicked {
            if let Some(next) = self.redo_stack.pop() {
                self.undo_stack.push(self.config.clone());
                self.config = next;
                self.channel.sync_state(self.config.clone());
            }
        }
        if apply_clicked {
            self.undo_stack.push(self.config.clone());
            self.redo_stack.clear();
            self.channel.sync_state(self.config.clone());
            self.status_notification = Some(("Live synced over Hermes Seqlock".to_string(), Instant::now()));
        }
        if save_clicked {
            let _ = self.save_to_disk();
        }
        if reset_clicked {
            self.reset_to_defaults();
        }

        if close_clicked || !is_open {
            self.is_open = false;
        } else {
            self.is_open = true;
        }

        self.render_pythia_hud(ctx);
    }

    fn render_appearance_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Color Palette & Window Appearance").color(Color32::from_rgb(57, 255, 20)));
        ui.add_space(4.0);

        ui.label(RichText::new("Select Active Color Theme:").strong());
        let mut new_theme = None;
        ui.horizontal(|ui| {
            let themes = [
                (0, "Cyber-Neon (Active)", Color32::from_rgb(57, 255, 20)),
                (1, "Tokyo Night", Color32::from_rgb(168, 85, 247)),
                (2, "Matrix Green", Color32::from_rgb(0, 255, 102)),
                (3, "Synthwave '84", Color32::from_rgb(255, 0, 127)),
            ];

            for (id, name, color) in themes {
                let is_selected = self.config.theme_id == id;
                if ui.selectable_label(is_selected, RichText::new(name).color(if is_selected { color } else { Color32::GRAY })).clicked() {
                    new_theme = Some(id);
                }
            }
        });
        if let Some(id) = new_theme {
            self.config.theme_id = id;
            self.channel.sync_state(self.config.clone());
        }

        ui.add_space(8.0);
        ui.label(RichText::new("Theme Swatch Preview (Cyber-Neon):").weak());
        ui.horizontal(|ui| {
            let swatches = [
                ("Neon Green", Color32::from_rgb(57, 255, 20)),
                ("Light Blue", Color32::from_rgb(56, 189, 248)),
                ("Magenta", Color32::from_rgb(255, 0, 127)),
                ("Purple", Color32::from_rgb(168, 85, 247)),
                ("Grey", Color32::from_rgb(136, 146, 154)),
                ("Black", Color32::from_rgb(5, 5, 8)),
            ];
            for (label, color) in swatches {
                ui.vertical(|ui| {
                    let (rect, _response) = ui.allocate_exact_size(egui::vec2(24.0, 16.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 3.0, color);
                    ui.label(RichText::new(label).size(10.0));
                });
            }
        });

        ui.separator();
        ui.label(RichText::new("Window Transparency & Blur:").strong());
        if ui.add(Slider::new(&mut self.config.background_opacity, 0.2..=1.0).text("Background Opacity")).changed() {
            self.channel.sync_state(self.config.clone());
        }
        if ui.add(Slider::new(&mut self.config.glass_blur_radius, 0.0..=50.0).text("Glass Blur Radius (px)")).changed() {
            self.channel.sync_state(self.config.clone());
        }

        ui.separator();
        ui.label(RichText::new("Cursor Styling:").strong());
        let mut new_cursor = None;
        ui.horizontal(|ui| {
            let cursors = [(0, "Block (█)"), (1, "Beam (|)"), (2, "Underline (_) ")];
            for (style, name) in cursors {
                if ui.selectable_label(self.config.cursor_style == style, name).clicked() {
                    new_cursor = Some(style);
                }
            }
        });
        if let Some(style) = new_cursor {
            self.config.cursor_style = style;
            self.channel.sync_state(self.config.clone());
        }
        if ui.checkbox(&mut self.config.cursor_blink, "Enable Cursor Blinking").changed() {
            self.channel.sync_state(self.config.clone());
        }
    }

    fn render_typography_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Typography & Font Sizing").color(Color32::from_rgb(56, 189, 248)));
        ui.add_space(4.0);

        ui.label(RichText::new("Active Monospace Font:").strong());
        let mut selected_font = None;
        ui.horizontal(|ui| {
            let fonts = ["OpenDyslexic Nerd Font (Active)", "JetBrains Mono", "Fira Code", "System Monospace"];
            for f in fonts {
                if ui.selectable_label(self.active_font_name == f, f).clicked() {
                    selected_font = Some(f.to_string());
                }
            }
        });
        if let Some(f) = selected_font {
            self.active_font_name = f.clone();
            self.config.font_name = f;
            self.channel.sync_state(self.config.clone());
        }

        ui.add_space(8.0);
        ui.label(RichText::new("Dynamic Font Sizing:").strong());
        if ui.add(Slider::new(&mut self.config.font_size, 10.0..=32.0).text("Font Size (pt)")).changed() {
            self.channel.sync_state(self.config.clone());
        }
        if ui.add(Slider::new(&mut self.config.line_height, 1.0..=2.0).text("Line Height Multiplier")).changed() {
            self.channel.sync_state(self.config.clone());
        }

        ui.separator();
        ui.label(RichText::new("Typography Features:").strong());
        if ui.checkbox(&mut self.config.enable_kitty_keyboard, "Kitty Keyboard Protocol (Precise Modifiers)").changed() {
            self.channel.sync_state(self.config.clone());
        }
    }

    fn render_shortcuts_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Command & Keybinding Mapping").color(Color32::from_rgb(250, 204, 21)));
        ui.label("Configure terminal key shortcuts, mouse actions, and clipboard bindings.");
        ui.add_space(8.0);

        // Header Row
        ui.horizontal(|ui| {
            ui.add_sized([160.0, 20.0], egui::Label::new(RichText::new("Shortcut Chord").strong().color(Color32::from_rgb(56, 189, 248))));
            ui.add_sized([220.0, 20.0], egui::Label::new(RichText::new("Target Action").strong().color(Color32::from_rgb(57, 255, 20))));
            ui.label(RichText::new("Delete").strong().color(Color32::from_rgb(148, 163, 184)));
        });
        ui.separator();

        egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
            let mut to_delete = None;
            for (idx, kb) in self.keybindings.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_sized([160.0, 22.0], egui::TextEdit::singleline(&mut kb.chord));
                    ui.add_sized([220.0, 22.0], egui::TextEdit::singleline(&mut kb.action));
                    if ui.button("❌").clicked() {
                        to_delete = Some(idx);
                    }
                });
            }
            if let Some(idx) = to_delete {
                self.keybindings.remove(idx);
            }
        });

        ui.add_space(6.0);
        ui.separator();
        ui.label(RichText::new("Add Custom Shortcut:").strong().color(Color32::from_rgb(250, 204, 21)));
        ui.horizontal(|ui| {
            ui.add_sized([160.0, 22.0], egui::TextEdit::singleline(&mut self.new_chord).hint_text("e.g. Cmd+Shift+P"));
            ui.add_sized([220.0, 22.0], egui::TextEdit::singleline(&mut self.new_action).hint_text("e.g. Command Palette"));
            if ui.button("➕ Add Shortcut").clicked() && !self.new_chord.is_empty() {
                self.keybindings.push(KeybindingRow {
                    chord: self.new_chord.clone(),
                    action: self.new_action.clone(),
                    conflict: false,
                });
                self.new_chord.clear();
                self.new_action.clear();
            }
        });
    }

    fn render_shell_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Metis Shell & Telemetry Config").color(Color32::from_rgb(255, 0, 127)));
        ui.add_space(4.0);

        ui.label("Executable Shell Binary:");
        if ui.text_edit_singleline(&mut self.shell_path).changed() {
            self.config.shell_path = self.shell_path.clone();
            self.channel.sync_state(self.config.clone());
        }

        ui.separator();
        ui.label(RichText::new("Starship Prompt Integration & Timeouts:").strong());
        if ui.checkbox(&mut self.config.enable_transient_prompt, "Enable Starship Transience (Instant Prompt Refresh)").changed() {
            self.channel.sync_state(self.config.clone());
        }

        if ui.add(Slider::new(&mut self.config.scan_timeout_ms, 10..=100).text("Directory Scan Timeout (ms)")).changed() {
            self.channel.sync_state(self.config.clone());
        }
        if ui.add(Slider::new(&mut self.config.command_timeout_ms, 100..=2000).text("Command Execution Timeout (ms)")).changed() {
            self.channel.sync_state(self.config.clone());
        }
        if ui.add(Slider::new(&mut self.config.scrollback_limit, 10_000..=500_000).text("Scrollback Buffer Depth")).changed() {
            self.channel.sync_state(self.config.clone());
        }

        ui.separator();
        ui.label(RichText::new("🛰️ Astraea Starship Presets:").strong().color(Color32::from_rgb(0, 240, 255)));
        ui.label("Select and instantly apply a Starship prompt preset to ~/.config/starship.toml:");

        let presets = ["Jetpack", "Tokyo Night", "Pure Minimal", "Gruvbox Rainbow", "Pastel Powerline"];
        
        ui.horizontal(|ui| {
            ui.label("Active Preset:");
            egui::ComboBox::from_id_salt("starship_preset_combobox")
                .selected_text(RichText::new(&self.config.prompt_preset).strong().color(Color32::from_rgb(57, 255, 20)))
                .show_ui(ui, |ui| {
                    for p in &presets {
                        if ui.selectable_label(self.config.prompt_preset == *p, *p).clicked() {
                            self.config.prompt_preset = p.to_string();
                            self.channel.sync_state(self.config.clone());
                        }
                    }
                });
        });

        // Preset Highlights Card
        if self.config.prompt_preset == "Jetpack" {
            ui.group(|ui| {
                ui.label(RichText::new("🚀 Jetpack Preset (Recommended)").color(Color32::from_rgb(250, 204, 21)).strong());
                ui.label("• Ultra high-speed asynchronous prompt with detailed Git metrics (adds/deletes).");
                ui.label("• Command execution timer (◄ 12ms), context symbols, and responsive battery/time status.");
                ui.label("• Custom character states: [◎] success, [○] error, [■] Vim mode.");
            });
        }

        ui.horizontal(|ui| {
            let apply_preset_btn = egui::Button::new(
                RichText::new(format!("🚀 Apply '{}' Preset to Starship", self.config.prompt_preset))
                    .color(Color32::from_rgb(57, 255, 20))
                    .strong()
            )
            .fill(Color32::from_rgba_premultiplied(20, 50, 30, 220))
            .stroke(Stroke::new(1.0f32, Color32::from_rgb(57, 255, 20)))
            .rounding(egui::Rounding::same(6.0));

            if ui.add(apply_preset_btn).clicked() {
                match apply_starship_preset(&self.config.prompt_preset) {
                    Ok(msg) => {
                        self.set_notification(msg);
                        self.channel.sync_state(self.config.clone());
                    }
                    Err(err) => {
                        self.set_notification(format!("Error: {}", err));
                    }
                }
            }
        });

        ui.separator();
        ui.label(RichText::new("Fish Abbreviation Expansion:").strong());
        let mut del_abbrev = None;
        for (idx, ab) in self.abbreviations.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut ab.keyword);
                ui.text_edit_singleline(&mut ab.expansion);
                if ui.button("❌").clicked() {
                    del_abbrev = Some(idx);
                }
            });
        }
        if let Some(idx) = del_abbrev {
            self.abbreviations.remove(idx);
        }

        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.new_keyword);
            ui.text_edit_singleline(&mut self.new_expansion);
            if ui.button("➕ Add Abbreviation").clicked() && !self.new_keyword.is_empty() {
                self.abbreviations.push(ShellAbbreviation {
                    keyword: self.new_keyword.clone(),
                    expansion: self.new_expansion.clone(),
                });
                self.new_keyword.clear();
                self.new_expansion.clear();
            }
        });
    }

    fn render_shaders_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Orpheus CRT Shaders & Phosphor Effects").color(Color32::from_rgb(168, 85, 247)));
        ui.add_space(4.0);

        if ui.add(Slider::new(&mut self.config.screen_curvature, 0.0..=0.5).text("CRT Barrel Curvature")).changed() {
            self.channel.sync_state(self.config.clone());
        }
        if ui.add(Slider::new(&mut self.config.scanline_frequency, 0.0..=2.0).text("Scanline Frequency")).changed() {
            self.channel.sync_state(self.config.clone());
        }
        if ui.add(Slider::new(&mut self.config.glow_radius, 0.0..=3.0).text("Phosphor Glow Radius")).changed() {
            self.channel.sync_state(self.config.clone());
        }
    }

    fn render_profiles_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Configuration Profiles & Hyprlang/Hermes").color(Color32::from_rgb(0, 240, 255)));
        ui.add_space(4.0);

        // Show profile selector
        let profiles = self.list_profile_names();
        ui.horizontal(|ui| {
            ui.label("Current Profile:");
            egui::ComboBox::from_label("Profile")
                .selected_text(self.current_profile.clone().unwrap_or_else(|| "<none>".to_string()))
                .show_ui(ui, |ui| {
                    for name in &profiles {
                        ui.selectable_value(&mut self.current_profile, Some(name.clone()), name);
                    }
                });
        });

        ui.add_space(8.0);
        ui.label(RichText::new("Hermes IPC Telemetry:").strong());
        ui.label(format!("Active Theme: {}", self.config.theme_id));
        ui.label(format!("Font Size: {} pt", self.config.font_size));
        ui.label(format!("Scrollback Buffer: {} lines", self.config.scrollback_limit));
        ui.label(format!("Background Opacity: {:.2}", self.config.background_opacity));

        ui.separator();
        let mut save_profile_req = false;
        let mut load_profile_req = false;
        let mut import_hyprlang_req = false;
        let mut export_hyprlang_req = false;

        ui.horizontal(|ui| {
            if ui.button("Save Profile").clicked() {
                save_profile_req = true;
            }
            if ui.button("Load Profile").clicked() {
                load_profile_req = true;
            }
            if ui.button("Import Hyprlang (.hl)").clicked() {
                import_hyprlang_req = true;
            }
            if ui.button("Export Hyprlang (.hl)").clicked() {
                export_hyprlang_req = true;
            }
            if ui.button("Reset Defaults").clicked() {
                self.reset_to_defaults();
            }
        });

        if save_profile_req {
            if let Some(name) = self.current_profile.clone() {
                let _ = self.save_profile(&name);
            }
        }
        if load_profile_req {
            if let Some(name) = self.current_profile.clone() {
                let _ = self.load_profile(&name);
            }
        }
        if import_hyprlang_req {
            if let Some(path) = Self::hyprlang_path() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(doc) = HyprlangParser::parse(&content) {
                        self.config = HyprlangParser::to_theia_config(&doc);
                        self.keybindings = HyprlangParser::to_keybinding_rows(&doc);
                        self.channel.sync_state(self.config.clone());
                        self.status_notification = Some(("Imported from Hyprlang (.hl)".into(), Instant::now()));
                    }
                }
            }
        }
        if export_hyprlang_req {
            let hl_str = HyprlangEmitter::emit(&self.config, &self.keybindings);
            if let Some(path) = Self::hyprlang_path() {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::write(&path, hl_str).is_ok() {
                    self.status_notification = Some((format!("Exported to {}", path.display()), Instant::now()));
                }
            }
        }
    }

    fn render_script_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Automations & HyperShell Execution").color(Color32::from_rgb(57, 255, 20)));
        ui.add_space(6.0);
        ui.label(RichText::new("HyperShell Async Engine").strong());
        ui.label("Integrated non-blocking command execution pipeline with sub-millisecond task dispatch.");
        ui.add_space(8.0);
        ui.label(RichText::new("Quick Automation Triggers:").strong());
        ui.horizontal(|ui| {
            if ui.button("Run Benchmark Suite (Hyperfine)").clicked() {
                let _ = std::process::Command::new("./well-benchmarks.sh").spawn();
                self.status_notification = Some(("Launched Hyperfine benchmark".into(), Instant::now()));
            }
            if ui.button("Launch Fish Transience Check").clicked() {
                let _ = std::process::Command::new("/opt/homebrew/bin/fish").arg("-c").arg("echo 'Fish active'").spawn();
            }
        });
    }

    fn render_help_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Well-Shell Codex & Architecture Documentation").color(Color32::from_rgb(0, 240, 255)));
        ui.add_space(4.0);

        ui.label(RichText::new("Overview").strong().color(Color32::from_rgb(57, 255, 20)));
        ui.label("Well-Shell (Phrear) is a native, sub-millisecond hardware-accelerated GPU terminal environment written in Rust with direct Metal/WebGPU instancing, zero-lock Hermes IPC, and native Fish 4.x + Starship prompt integration.");
        ui.add_space(8.0);

        ui.label(RichText::new("Keyboard Shortcuts:").strong().color(Color32::from_rgb(250, 204, 21)));
        egui::Grid::new("shortcuts_grid")
            .striped(true)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Shortcut").strong());
                ui.label(RichText::new("Action").strong());
                ui.label(RichText::new("Subsystem").strong());
                ui.end_row();

                ui.label("Cmd+, / Ctrl+,");
                ui.label("Toggle Theia Control Center");
                ui.label("Theia's Prism");
                ui.end_row();

                ui.label("Esc");
                ui.label("Dismiss / Close Control Center");
                ui.label("Theia's Prism");
                ui.end_row();

                ui.label("F1 / F12");
                ui.label("Alternate Control Center Toggle");
                ui.label("Theia's Prism");
                ui.end_row();

                ui.label("Cmd+Shift+P / Cmd+P");
                ui.label("Open Universal Command Palette");
                ui.label("Atlas Host");
                ui.end_row();

                ui.label("Cmd+F / Ctrl+F");
                ui.label("Find & Search in Scrollback");
                ui.label("Atlas Host");
                ui.end_row();

                ui.label("Cmd+I / Ctrl+I");
                ui.label("Launch Pythia AI Shell Assistant");
                ui.label("Pythia Copilot");
                ui.end_row();

                ui.label("Cmd+A / Ctrl+Shift+A");
                ui.label("Select All Visible Cells");
                ui.label("Atlas Host");
                ui.end_row();

                ui.label("Cmd + D");
                ui.label("Split Pane Horizontal");
                ui.label("Metis Shell");
                ui.end_row();

                ui.label("Cmd + Shift + D");
                ui.label("Split Pane Vertical");
                ui.label("Metis Shell");
                ui.end_row();

                ui.label("Cmd + K");
                ui.label("Clear Terminal Buffer");
                ui.label("Orpheus Screen");
                ui.end_row();

                ui.label("Shift + PgUp / Cmd + PgUp");
                ui.label("Scroll Viewport Up (Page)");
                ui.label("Orpheus Screen");
                ui.end_row();

                ui.label("Shift + PgDn / Cmd + PgDn");
                ui.label("Scroll Viewport Down (Page)");
                ui.label("Orpheus Screen");
                ui.end_row();

                ui.label("Cmd + Home / Shift + Home");
                ui.label("Scroll to Top of History");
                ui.label("Orpheus Screen");
                ui.end_row();

                ui.label("Cmd + End / Shift + End");
                ui.label("Scroll to Bottom (Active Prompt)");
                ui.label("Orpheus Screen");
                ui.end_row();

                ui.label("Cmd+= / Cmd+-");
                ui.label("Increase / Decrease Font Size");
                ui.label("Orpheus Atlas");
                ui.end_row();

                ui.label("F5");
                ui.label("Run Diagnostics / Embedded Demo");
                ui.label("Metis Engine");
                ui.end_row();
            });

        ui.add_space(10.0);
        ui.label(RichText::new("Subsystem Pantheon:").strong().color(Color32::from_rgb(56, 189, 248)));
        ui.label("• ATLAS: Host platform windowing & winit event loop pacer (Metal / WebGPU).");
        ui.label("• ORPHEUS: Single-pass GPU cell matrix renderer with fontdue glyph cache & CRT shaders.");
        ui.label("• METIS: Interactive PTY supervisor with sub-0.1ms DA1/DA2 responder for Fish 4.x.");
        ui.label("• HYPERSHELL: Asynchronous multi-stage task pipeline and background job execution engine.");
        ui.label("• HERMES: Zero-lock atomic Seqlock (AtomicU64) for lock-free state synchronization.");
        ui.label("• HYPRLANG: Declarative Wayland-style block configuration parser and emitter (.hl).");
        ui.label("• ASTRAEA: Sub-100µs atomic prompt vector compiler bypassing process fork overhead.");
        ui.label("• MNEME: Inline multi-line composition editor powered by an O(log n) B-tree rope buffer.");

        ui.add_space(8.0);
        ui.label(RichText::new("Configuration & Profiles:").strong().color(Color32::from_rgb(168, 85, 247)));
        ui.label("• Profiles are stored under ~/.config/well/profiles/<name>.json.");
        ui.label("• Use the 'Profiles & Hyprlang' tab to switch profiles, export .hl, or import .hl files.");
        ui.label("• All settings changes update live over Hermes Seqlock without requiring an app restart.");
    }

    fn render_outline_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("📐 Well Architecture & Subsystem Specification").color(Color32::from_rgb(56, 189, 248)).size(18.0));
        ui.label(RichText::new("Unified, sub-millisecond, memory-safe terminal, shell, prompt, and editor workspace running at the physical performance ceiling of modern hardware.").italics().color(Color32::from_rgb(148, 163, 184)));
        ui.add_space(8.0);

        // Core Architectural Tenets Card
        egui::Frame::none()
            .fill(Color32::from_rgba_premultiplied(15, 23, 42, 200))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(56, 189, 248, 100)))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("⚡ Core Mandates:").strong().color(Color32::from_rgb(56, 189, 248)));
                    ui.label(RichText::new("Sub-2.5ms latency").strong().color(Color32::from_rgb(57, 255, 20)));
                    ui.label("•");
                    ui.label(RichText::new("< 30MB base RSS").strong().color(Color32::from_rgb(57, 255, 20)));
                    ui.label("•");
                    ui.label(RichText::new("4ms I/O Batching (250 FPS)").strong().color(Color32::from_rgb(57, 255, 20)));
                    ui.label("•");
                    ui.label(RichText::new("Zero Subprocess Forks").strong().color(Color32::from_rgb(57, 255, 20)));
                });
            });

        ui.add_space(10.0);
        ui.label(RichText::new("Subsystem Pantheon:").strong().size(15.0).color(Color32::from_rgb(255, 255, 255)));
        ui.add_space(4.0);

        let subsystems = [
            ("ATLAS", "Host Windowing & OS Loop", Color32::from_rgb(0, 240, 255), "winit event loop pacer, high-precision frame timers, raw modifier bitmask capture, split-pane multiplexing (Cmd+D / Cmd+Shift+D), and native Cocoa/Wayland window chrome integration."),
            ("ORPHEUS", "GPU Text Shaper & Pipeline", Color32::from_rgb(56, 189, 248), "Single-pass instanced drawIndexed calls, layered dynamic texture glyph array (monochrome ASCII, bold/italic, CJK, RGBA emojis), branchless WGSL fragment shading, dynamic CRT curvature & scanline passes, and DEC 2026 150ms timeout lock recovery."),
            ("METIS", "Shell Logic & History Engine", Color32::from_rgb(255, 0, 127), "Direct in-process shell execution, O(k) prefix-trie indexing up to 260,000 commands from history, sub-50µs ghost completions with Right Arrow/Ctrl+F full accept and Alt+Right Arrow token traversal."),
            ("MNEME", "Inline Composition Buffer", Color32::from_rgb(250, 204, 21), "In-place prompt expansion for unclosed multi-line commands (if, for, unclosed quotes) or Alt+E/Alt+V, backed by an O(log n) B-tree rope buffer (ropey), background incremental Tree-Sitter AST syntax validator (green/red diagnostics), and embedded LSP client."),
            ("ASTRAEA", "State-Vector Prompt Compiler", Color32::from_rgb(168, 85, 247), "Zero-fork background state vector daemon tracking SCM (Git branch, dirty files, ahead/behind, stash count), cloud environment, and battery without subprocess spawns. Compiles prompts in < 100µs with strict prompt transience collapsing past prompts to ❯."),
            ("HERMES", "PTY-Bypass Protocol & Seqlock IPC", Color32::from_rgb(57, 255, 20), "Lock-free atomic sequence lock (AtomicU64) ring queues mapped directly between shell logic and GPU renderer, zero mutex contention, and strongly typed binary IPC schemas."),
            ("CADUCEUS", "In-Process hyper RPC Daemon", Color32::from_rgb(244, 63, 94), "Local-only async HTTP/WebSocket server over Unix Domain Sockets (~/.well/well.sock) or Windows Named Pipes, enforced by kernel peer credential verification (SO_PEERCRED at 0600), exposing workspace.*, surface.*, and ai.* orchestration hooks."),
            ("THEIA'S PRISM", "Visual GPU Control Center", Color32::from_rgb(0, 240, 255), "Immediate-mode egui control center overlay (Cmd+, / F12), real-time telemetry HUD, Hyprlang/TOML two-way config synchronization, and live Tree-Sitter syntax validation gates preventing corrupt config persistence."),
        ];

        for (name, subtitle, color, desc) in subsystems {
            egui::Frame::none()
                .fill(Color32::from_rgba_premultiplied(18, 24, 38, 180))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(color.r() / 3, color.g() / 3, color.b() / 3, 160)))
                .rounding(egui::Rounding::same(6.0))
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("◈ {}", name)).strong().color(color).size(13.0));
                        ui.label(RichText::new(format!("— {}", subtitle)).color(Color32::from_rgb(203, 213, 225)).size(12.0));
                    });
                    ui.add_space(2.0);
                    ui.label(RichText::new(desc).color(Color32::from_rgb(148, 163, 184)).size(11.5));
                });
            ui.add_space(4.0);
        }

        ui.add_space(6.0);
        ui.label(RichText::new("Strict Architectural Boundaries:").strong().size(13.0).color(Color32::from_rgb(250, 204, 21)));
        ui.label("1. Never spawn subprocesses in hot paths (prompts, suggestions, keystrokes).");
        ui.label("2. Every viewport redraw must occur in a single instanced GPU draw call.");
        ui.label("3. Memory footprint must stay strictly below 30MB baseline RSS.");
        ui.label("4. Redraw frequencies are capped at 250 FPS via 4ms batch epochs to prevent log-flood GPU saturation.");
    }

    fn render_timeline_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("⏱️ Well 12-Week Production Build Roadmap").color(Color32::from_rgb(244, 63, 94)).size(18.0));
        ui.label(RichText::new("Strategic 6-phase engineering timeline from GPU & terminal core maturity to production cross-platform distribution.").italics().color(Color32::from_rgb(148, 163, 184)));
        ui.add_space(8.0);

        let phases = [
            (
                "Phase 1: GPU Engine & Windowing Maturity",
                "Weeks 1–2",
                "90% Complete",
                Color32::from_rgb(57, 255, 20),
                "Split-pane multiplexing (Cmd+D / Cmd+Shift+D), CRT post-processing shader pass, 4ms I/O event coalescing, DEC 2026 150ms timeout safety gate, full Kitty Keyboard Protocol compliance.",
                "Target: 250 FPS ceiling under log flood; sub-millisecond redraws.",
            ),
            (
                "Phase 2: In-Process Shell Core & Predictive Engine",
                "Weeks 3–4",
                "60% Complete",
                Color32::from_rgb(56, 189, 248),
                "Metis prefix-trie history indexer (260,000 commands), sub-50µs ghost completion suggestions, path-token traversal (Alt+Right Arrow), in-namespace command execution.",
                "Target: < 50 µs ghost completion lookup time.",
            ),
            (
                "Phase 3: Astraea State-Vector Prompt Engine",
                "Weeks 5–6",
                "50% Complete",
                Color32::from_rgb(168, 85, 247),
                "File-watcher event-driven state vector (Git SCM, cloud, battery), zero-fork prompt compiler, strict prompt transience collapsing old prompts to ❯.",
                "Target: < 100 µs prompt compile time (verified by unit tests).",
            ),
            (
                "Phase 4: Mneme Inline Editor & Safety AST Validator",
                "Weeks 7–8",
                "40% Complete",
                Color32::from_rgb(250, 204, 21),
                "Dynamic in-place multi-line expansion on unclosed blocks or Alt+E/Alt+V, ropey B-tree buffer, live Tree-Sitter AST syntax validator (green valid / red error), embedded LSP diagnostic channel.",
                "Target: Live syntax validation with zero typing buffer lag.",
            ),
            (
                "Phase 5: Hermes Shared-Memory IPC & Caduceus RPC",
                "Weeks 9–10",
                "50% Complete",
                Color32::from_rgb(0, 240, 255),
                "Lock-free Seqlock ring buffers, local Unix domain socket daemon (~/.well/well.sock) with SO_PEERCRED validation, agent orchestration endpoints (workspace.*, surface.*, ai.*).",
                "Target: Sub-millisecond RPC roundtrip and zero mutex contention.",
            ),
            (
                "Phase 6: Hardening, Packaging & Distribution",
                "Weeks 11–12",
                "25% Complete",
                Color32::from_rgb(244, 63, 94),
                "Fat LTO release profile verification (<30MB RSS, <2.5ms latency), cross-platform packaging (macOS Universal .dmg, Linux AppImage/deb/rpm, Windows MSI/zip), vttest compliance suite.",
                "Target: Zero-flicker 144Hz+ rendering, full terminal compatibility.",
            ),
        ];

        for (title, horizon, progress, color, deliverables, metric) in phases {
            egui::Frame::none()
                .fill(Color32::from_rgba_premultiplied(18, 24, 38, 180))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(color.r() / 3, color.g() / 3, color.b() / 3, 160)))
                .rounding(egui::Rounding::same(6.0))
                .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(title).strong().color(color).size(13.5));
                        ui.label(RichText::new(format!("— {}", horizon)).color(Color32::from_rgb(203, 213, 225)).size(12.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(progress).strong().color(color).size(11.5));
                        });
                    });
                    ui.add_space(3.0);
                    ui.label(RichText::new(deliverables).color(Color32::from_rgb(148, 163, 184)).size(11.5));
                    ui.add_space(2.0);
                    ui.label(RichText::new(metric).italics().color(Color32::from_rgb(250, 204, 21)).size(11.0));
                });
            ui.add_space(4.0);
        }

        ui.add_space(6.0);
        ui.label(RichText::new("Milestone Summary Schedule:").strong().size(13.0).color(Color32::from_rgb(56, 189, 248)));
        egui::Grid::new("timeline_milestone_grid")
            .striped(true)
            .spacing([14.0, 6.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Milestone").strong().color(Color32::from_rgb(56, 189, 248)));
                ui.label(RichText::new("Horizon").strong().color(Color32::from_rgb(56, 189, 248)));
                ui.label(RichText::new("Primary Focus").strong().color(Color32::from_rgb(56, 189, 248)));
                ui.label(RichText::new("Success Verification").strong().color(Color32::from_rgb(56, 189, 248)));
                ui.end_row();

                let milestones = [
                    ("M1", "Week 2", "Multiplexing & Shaders", "250 FPS ceiling under log flood"),
                    ("M2", "Week 4", "Predictive Shell Engine", "< 50 µs ghost completion lookup"),
                    ("M3", "Week 6", "State-Vector Prompts", "< 100 µs compile time, transience"),
                    ("M4", "Week 8", "Mneme Inline Editor", "Live AST validation, zero buffer lag"),
                    ("M5", "Week 10", "Hermes & Caduceus IPC", "Sub-millisecond IPC roundtrip"),
                    ("M6", "Week 12", "Production Release", "< 30 MB RSS, < 2.5 ms latency"),
                ];

                for (m, horizon, focus, verify) in milestones {
                    ui.label(RichText::new(m).strong().color(Color32::from_rgb(255, 255, 255)));
                    ui.label(RichText::new(horizon).color(Color32::from_rgb(203, 213, 225)));
                    ui.label(RichText::new(focus).color(Color32::from_rgb(203, 213, 225)));
                    ui.label(RichText::new(verify).color(Color32::from_rgb(57, 255, 20)));
                    ui.end_row();
                }
            });
    }

    fn render_pythia_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("✦ Pythia LLM Translation & Diagnostic Layer").color(Color32::from_rgb(192, 132, 252)));
        ui.label(RichText::new("Configure high-speed Natural Language to Shell synthesis and error diagnosis.").color(Color32::from_rgb(148, 163, 184)));
        ui.add_space(8.0);

        ui.label(RichText::new("Primary AI Engine:").strong());
        let providers = [
            ("Gemma Abliterated (Hugging Face)", "Uncensored Gemma 2 without corporate refusal filters"),
            ("Google Gemini 2.0", "Fast, high-reasoning multimodal cloud intelligence"),
            ("Ollama (Local)", "100% private local inference (localhost:11434)"),
            ("Offline Semantic Rules", "Zero-latency offline regex pattern compiler (<1ms)"),
        ];
        for (p, desc) in providers {
            let selected = self.config.pythia_provider == p;
            ui.horizontal(|ui| {
                if ui.selectable_label(selected, p).clicked() {
                    self.config.pythia_provider = p.to_string();
                }
                ui.label(RichText::new(format!("— {}", desc)).size(11.0).color(Color32::from_rgb(120, 140, 165)));
            });
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(RichText::new("1. Hugging Face Gemma Abliterated Configuration").strong().color(Color32::from_rgb(250, 204, 21)));
        ui.label(RichText::new("Uncensored Gemma models have refusal vectors abliterated so they execute technical root admin, security, and networking commands without corporate lecture.").size(11.0).color(Color32::from_rgb(160, 180, 205)));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Model ID:");
            ui.text_edit_singleline(&mut self.config.hf_model);
            if ui.button("Gemma 2 9B (Default)").clicked() {
                self.config.hf_model = "failspy/gemma-2-9b-it-abliterated".to_string();
            }
            if ui.button("Gemma 2 2B (Lightweight)").clicked() {
                self.config.hf_model = "mlabonne/gemma-2-2b-it-abliterated".to_string();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Hugging Face Token (HF_TOKEN):");
            ui.add(egui::TextEdit::singleline(&mut self.config.hf_token).password(true));
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(RichText::new("2. Google Gemini 2.0 Flash Configuration").strong().color(Color32::from_rgb(56, 189, 248)));
        ui.horizontal(|ui| {
            ui.label("Gemini API Key:");
            ui.add(egui::TextEdit::singleline(&mut self.config.gemini_api_key).password(true));
        });
        ui.horizontal(|ui| {
            ui.label("Model ID:");
            ui.text_edit_singleline(&mut self.config.gemini_model);
            if ui.button("gemini-2.0-flash").clicked() {
                self.config.gemini_model = "gemini-2.0-flash".to_string();
            }
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(RichText::new("3. Local Ollama Configuration").strong().color(Color32::from_rgb(57, 255, 20)));
        ui.horizontal(|ui| {
            ui.label("Ollama Endpoint:");
            ui.text_edit_singleline(&mut self.config.ollama_url);
        });
        ui.horizontal(|ui| {
            ui.label("Model Name:");
            ui.text_edit_singleline(&mut self.config.ollama_model);
            if ui.button("qwen2.5-coder").clicked() {
                self.config.ollama_model = "qwen2.5-coder".to_string();
            }
            if ui.button("llama3.2").clicked() {
                self.config.ollama_model = "llama3.2".to_string();
            }
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(RichText::new("4. Cybernetic Image & Graphic Generation Studio").strong().color(Color32::from_rgb(192, 132, 252)));
        ui.label(RichText::new("Generate high-resolution PNG terminal graphics, background textures, and icons using Gemini Omni Flash or Hugging Face Stable Diffusion.").size(11.0).color(Color32::from_rgb(160, 180, 205)));
        ui.add_space(6.0);

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let cache_dir = std::path::Path::new(&home).join(".well").join("generated");
        ui.horizontal(|ui| {
            ui.label(RichText::new("Storage Cache:").size(11.0).color(Color32::from_rgb(120, 140, 165)));
            ui.label(RichText::new(cache_dir.to_string_lossy()).size(11.0).monospace().color(Color32::from_rgb(56, 189, 248)));
            if ui.button("📁 Open Folder").clicked() {
                let _ = std::process::Command::new("open").arg(&cache_dir).spawn();
            }
        });
        ui.add_space(6.0);

        ui.label(RichText::new("One-Click Graphic Presets:").size(11.5).strong().color(Color32::from_rgb(200, 215, 235)));
        ui.horizontal_wrapped(|ui| {
            let presets = [
                ("🖼 Synthwave Grid", "synthwave neon grid wireframe sunset", 1024, 1024),
                ("🌆 Cyber City", "cyberpunk matrix rain city skyline", 1024, 1024),
                ("👾 Terminal Icon", "retro phosphor green terminal icon", 512, 512),
                ("🌌 Deep Space", "deep space cosmic nebula stars", 1024, 1024),
                ("⚡ Quantum Flux", "quantum computing golden glowing circuit board", 1024, 1024),
            ];
            for (label, prompt, w, h) in presets {
                let btn = egui::Button::new(RichText::new(label).size(11.0).strong().color(Color32::from_rgb(192, 132, 252)))
                    .fill(Color32::from_rgba_premultiplied(32, 20, 48, 220))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(168, 85, 247, 120)))
                    .rounding(egui::Rounding::same(6.0));
                if ui.add(btn).clicked() {
                    self.pythia_query = format!("/image {}", prompt);
                    self.trigger_image_generation(prompt, w, h);
                }
            }
        });

        if let Some(ref res) = self.pythia_result {
            if let Some(ref path) = res.generated_image_path {
                ui.add_space(8.0);
                egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(12, 16, 28, 240))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(168, 85, 247)))
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("🖼 Active Artifact:").strong().color(Color32::from_rgb(192, 132, 252)).size(12.0));
                            ui.label(RichText::new(path).monospace().color(Color32::from_rgb(148, 163, 184)).size(11.0));
                        });
                        ui.horizontal(|ui| {
                            if ui.button("🔍 Open Image").clicked() {
                                let _ = std::process::Command::new("open").arg(path).spawn();
                            }
                            if ui.button("📁 Reveal in Finder").clicked() {
                                let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
                            }
                            if ui.button("⚡ Inject Path to Shell").clicked() {
                                self.pending_pty_injection = Some(format!("{}\n", path));
                            }
                        });
                    });
            }
        }
    }

    pub fn trigger_image_generation(&mut self, prompt: &str, width: u32, height: u32) {
        // Clear last URI so that the render function will call forget_image() on
        // it before attempting to load the new file, bypassing egui's stale cache.
        self.last_image_uri = None;
        let provider = match self.config.pythia_provider.as_str() {
            "Gemma Abliterated (Hugging Face)" => well_llm::LlmProvider::HuggingFaceGemmaAbliterated,
            "Google Gemini 2.0" => well_llm::LlmProvider::Gemini,
            "Ollama (Local)" => well_llm::LlmProvider::Ollama,
            _ => well_llm::LlmProvider::OfflineRules,
        };

        let req = well_llm::TranslationRequest {
            query: String::new(),
            shell: self.config.shell_path.clone(),
            cwd: std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            provider,
            gemini_api_key: if self.config.gemini_api_key.trim().is_empty() { None } else { Some(self.config.gemini_api_key.trim().to_string()) },
            gemini_model: Some(self.config.gemini_model.clone()),
            ollama_url: Some(self.config.ollama_url.clone()),
            ollama_model: Some(self.config.ollama_model.clone()),
            hf_token: if self.config.hf_token.trim().is_empty() { None } else { Some(self.config.hf_token.trim().to_string()) },
            hf_model: Some(self.config.hf_model.clone()),
            image_prompt: Some(prompt.to_string()),
            image_width: Some(width),
            image_height: Some(height),
        };

        self.pythia_result = Some(well_llm::PythiaTranslator::translate(&req));
    }

    fn render_image_studio_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("🖼 Cybernetic Image & Graphic Studio").color(Color32::from_rgb(56, 189, 248)).size(18.0));
        ui.label(RichText::new("Generate PNG terminal graphics, background textures, and icons directly from prompt vectors.").color(Color32::from_rgb(148, 163, 184)));
        ui.add_space(8.0);

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let cache_dir = std::path::Path::new(&home).join(".well").join("generated");
        let _ = std::fs::create_dir_all(&cache_dir);

        // Studio Info Box
        egui::Frame::none()
            .fill(Color32::from_rgba_premultiplied(12, 18, 30, 220))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(56, 189, 248, 100)))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("📁 Storage Cache:").strong().color(Color32::from_rgb(120, 140, 165)));
                    ui.label(RichText::new(cache_dir.to_string_lossy()).monospace().color(Color32::from_rgb(56, 189, 248)));
                    if ui.button("📁 Open Folder in Finder").clicked() {
                        let _ = std::process::Command::new("open").arg(&cache_dir).spawn();
                    }
                });
            });

        ui.add_space(10.0);
        ui.label(RichText::new("⚡ One-Click Generation Presets:").strong().size(13.0).color(Color32::from_rgb(200, 215, 235)));
        ui.add_space(4.0);

        let presets = [
            ("🖼 Synthwave Neon Grid", "synthwave neon grid wireframe sunset", 1024, 1024, Color32::from_rgb(255, 0, 128)),
            ("🌆 Cyberpunk Matrix City", "cyberpunk matrix rain city skyline", 1024, 1024, Color32::from_rgb(57, 255, 20)),
            ("👾 Retro Terminal Icon", "retro phosphor green terminal icon", 512, 512, Color32::from_rgb(56, 189, 248)),
            ("🌌 Deep Space Nebula", "deep space cosmic nebula stars", 1024, 1024, Color32::from_rgb(192, 132, 252)),
            ("⚡ Quantum Golden Circuit", "quantum computing golden glowing circuit board", 1024, 1024, Color32::from_rgb(250, 204, 21)),
        ];

        for (label, prompt, w, h, accent) in presets {
            ui.horizontal(|ui| {
                let btn = egui::Button::new(RichText::new(label).strong().color(accent).size(12.0))
                    .fill(Color32::from_rgba_premultiplied(accent.r() / 8, accent.g() / 8, accent.b() / 8, 220))
                    .stroke(Stroke::new(1.0_f32, accent))
                    .rounding(egui::Rounding::same(6.0));
                if ui.add_sized([240.0, 30.0], btn).clicked() {
                    self.pythia_query = format!("/image {}", prompt);
                    self.trigger_image_generation(prompt, w, h);
                }
                ui.label(RichText::new(format!("{}x{} • \"{}\"", w, h, prompt)).color(Color32::from_rgb(140, 155, 175)).size(11.0));
            });
            ui.add_space(3.0);
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(RichText::new("Custom Image Generator:").strong().size(13.0).color(Color32::from_rgb(56, 189, 248)));
        ui.add_space(4.0);

        let mut do_generate = false;
        ui.horizontal(|ui| {
            ui.label("Prompt:");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.pythia_query)
                    .hint_text("e.g. 'synthwave sunset wireframe grid' or '/image ...'")
                    .desired_width(ui.available_width() - 180.0)
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                do_generate = true;
            }

            let gen_btn = egui::Button::new(RichText::new("⚡ Generate").strong().color(Color32::from_rgb(56, 189, 248)))
                .fill(Color32::from_rgba_premultiplied(16, 32, 54, 220))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                .rounding(egui::Rounding::same(6.0));
            if ui.add_sized([100.0, 26.0], gen_btn).clicked() {
                do_generate = true;
            }
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Dimensions:").size(11.0).color(Color32::from_rgb(120, 140, 165)));
            let dims = [((512, 512), "512 x 512"), ((1024, 1024), "1024 x 1024")];
            for (d, name) in dims {
                let sel = self.pythia_image_dim == d;
                let btn = egui::Button::new(RichText::new(name).size(10.5).color(if sel { Color32::BLACK } else { Color32::from_rgb(56, 189, 248) }))
                    .fill(if sel { Color32::from_rgb(56, 189, 248) } else { Color32::from_rgba_premultiplied(16, 28, 44, 180) })
                    .rounding(egui::Rounding::same(4.0));
                if ui.add(btn).clicked() {
                    self.pythia_image_dim = d;
                }
            }
        });

        if do_generate {
            let prompt = self.pythia_query.trim_start_matches("/image ").trim_start_matches("/img ").trim().to_string();
            if !prompt.is_empty() {
                self.trigger_image_generation(&prompt, self.pythia_image_dim.0, self.pythia_image_dim.1);
            }
        }

        if let Some(ref res) = self.pythia_result {
            if let Some(ref img_path) = res.generated_image_path {
                ui.add_space(12.0);
                egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(12, 16, 28, 240))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(168, 85, 247)))
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("🖼 Active Artifact:").strong().color(Color32::from_rgb(192, 132, 252)).size(13.0));
                            ui.label(RichText::new(img_path).monospace().color(Color32::from_rgb(148, 163, 184)).size(11.5));
                        });
                        ui.add_space(6.0);
                        // In-window visual image preview via egui texture
                        // Forget any previously cached texture for this path to ensure
                        // fresh file reads after each generation.
                        let img_uri = format!("file://{}", img_path);
                        if self.last_image_uri.as_deref() != Some(img_path.as_str()) {
                            if let Some(ref old_uri) = self.last_image_uri {
                                ui.ctx().forget_image(old_uri);
                            }
                            // Also forget the current URI to force a fresh load
                            ui.ctx().forget_image(&img_uri);
                            self.last_image_uri = Some(img_path.clone());
                        }
                        ui.add(
                            egui::Image::new(&img_uri)
                                .max_width(320.0)
                                .max_height(320.0)
                                .rounding(egui::Rounding::same(6.0))
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            let open_btn = egui::Button::new(RichText::new("🔍 Open in Viewer").strong().color(Color32::from_rgb(57, 255, 20)))
                                .fill(Color32::from_rgba_premultiplied(18, 42, 28, 220))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(57, 255, 20)))
                                .rounding(egui::Rounding::same(6.0));
                            if ui.add(open_btn).clicked() {
                                let _ = std::process::Command::new("open").arg(img_path).spawn();
                            }

                            let reveal_btn = egui::Button::new(RichText::new("📁 Reveal in Finder").strong().color(Color32::from_rgb(56, 189, 248)))
                                .fill(Color32::from_rgba_premultiplied(16, 28, 44, 220))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                                .rounding(egui::Rounding::same(6.0));
                            if ui.add(reveal_btn).clicked() {
                                let _ = std::process::Command::new("open").arg("-R").arg(img_path).spawn();
                            }

                            let inject_btn = egui::Button::new(RichText::new("⚡ Inject Path to Shell").strong().color(Color32::from_rgb(250, 204, 21)))
                                .fill(Color32::from_rgba_premultiplied(40, 30, 10, 220))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(250, 204, 21)))
                                .rounding(egui::Rounding::same(6.0));
                            if ui.add(inject_btn).clicked() {
                                self.pending_pty_injection = Some(format!("{}\n", img_path));
                            }
                        });
                    });
            }
        }
    }

    pub fn render_agent_sidebar(&mut self, ctx: &egui::Context) {
        if self.agent_events.is_empty() {
            return;
        }
        
        egui::SidePanel::right("agent_activity_sidebar")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.heading(
                    egui::RichText::new("✦ Agent Activity")
                        .color(egui::Color32::from_rgb(168, 85, 247))
                        .strong()
                );
                ui.add_space(4.0);
                ui.separator();
                
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let events = self.agent_events.clone(); // Clone to avoid borrow checker issues during render
                    for ev in events {
                        ui.add_space(8.0);
                        
                        let event_color = match ev.event_type.as_str() {
                            "ai.session_start" => egui::Color32::from_rgb(57, 255, 20),
                            "ai.tool_use" => egui::Color32::from_rgb(56, 189, 248),
                            "ai.stream_diff" => egui::Color32::from_rgb(250, 204, 21),
                            _ => egui::Color32::from_rgb(200, 200, 200),
                        };
                        
                        ui.label(
                            egui::RichText::new(&ev.event_type)
                                .color(event_color)
                                .strong()
                        );
                        
                        // Render BlockList
                        if let Some(blocks) = ev.payload.get("blocks").and_then(|b| b.as_array()) {
                            for block in blocks {
                                if let Some(content) = block.get("content").and_then(|c| c.as_str()) {
                                    if block.get("type").and_then(|t| t.as_str()) == Some("code") {
                                        ui.group(|ui| {
                                            ui.label(egui::RichText::new(content).monospace().color(egui::Color32::LIGHT_GRAY));
                                            ui.horizontal(|ui| {
                                                if ui.button("📋 Copy").clicked() {
                                                    ui.output_mut(|o| o.copied_text = content.to_string());
                                                }
                                                if ui.button("▶ Run").clicked() {
                                                    // Set pending_pty_injection to inject into terminal
                                                    self.pending_pty_injection = Some(format!("{}\n", content));
                                                }
                                            });
                                        });
                                    } else {
                                        ui.label(content);
                                    }
                                }
                            }
                        } else {
                            // Fallback rendering
                            ui.label(ev.payload.to_string());
                        }
                        ui.separator();
                    }
                });
            });
    }

    pub fn render_pythia_hud(&mut self, ctx: &egui::Context) {
        if !self.show_pythia_hud {
            return;
        }

        let screen_rect = ctx.screen_rect();
        let modal_w = (screen_rect.width() * 0.72).clamp(420.0, 750.0);

        let mut close_modal = false;
        let mut do_translate = false;

        egui::Window::new("✦ Pythia Cybernetic Translation HUD")
            .id(egui::Id::new("pythia_floating_hud_modal"))
            .open(&mut self.show_pythia_hud)
            .collapsible(false)
            .resizable(true)
            .default_width(modal_w)
            .default_pos(pos2((screen_rect.width() - modal_w) * 0.5, 55.0))
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgba_premultiplied(12, 16, 26, 250))
                    .stroke(Stroke::new(1.2_f32, Color32::from_rgba_premultiplied(168, 85, 247, 220)))
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("✦ Pythia Translation Layer").strong().size(14.0).color(Color32::from_rgb(192, 132, 252)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let provider_color = match self.config.pythia_provider.as_str() {
                            "Gemma Abliterated (Hugging Face)" => Color32::from_rgb(250, 204, 21),
                            "Google Gemini 2.0" => Color32::from_rgb(56, 189, 248),
                            "Ollama (Local)" => Color32::from_rgb(57, 255, 20),
                            _ => Color32::from_rgb(168, 85, 247),
                        };
                        ui.label(RichText::new(&self.config.pythia_provider).size(11.0).color(provider_color).strong());
                        ui.label(RichText::new("Engine:").size(11.0).color(Color32::from_rgb(120, 140, 165)));
                    });
                });

                ui.add_space(6.0);

                // Provider quick switcher pills
                ui.horizontal(|ui| {
                    let providers = [
                        ("Gemma Obliterated (HF)", "Gemma Abliterated (Hugging Face)"),
                        ("Gemini 2.0 Flash", "Google Gemini 2.0"),
                        ("Ollama Local", "Ollama (Local)"),
                        ("Offline Rules", "Offline Semantic Rules"),
                    ];
                    for (label, val) in providers {
                        let selected = self.config.pythia_provider == val;
                        let btn = egui::Button::new(RichText::new(label).size(10.0).color(if selected { Color32::BLACK } else { Color32::from_rgb(200, 215, 235) }).strong())
                            .fill(if selected { Color32::from_rgb(192, 132, 252) } else { Color32::from_rgba_premultiplied(22, 28, 44, 200) })
                            .rounding(egui::Rounding::same(4.0));
                        if ui.add(btn).clicked() {
                            self.config.pythia_provider = val.to_string();
                            self.channel.sync_state(self.config.clone());
                        }
                    }
                });

                ui.add_space(8.0);

                // Query input row
                let mut do_generate_image = false;
                ui.horizontal(|ui| {
                    let text_edit = egui::TextEdit::singleline(&mut self.pythia_query)
                        .hint_text("English query (e.g. 'kill port 3000') or '/image <prompt>' for graphics...")
                        .desired_width((ui.available_width() - 310.0).max(120.0));
                    let resp = ui.add(text_edit);
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if self.pythia_query.trim().starts_with("/image ") || self.pythia_query.trim().starts_with("/img ") {
                            do_generate_image = true;
                        } else {
                            do_translate = true;
                        }
                    }

                    if ui.button("✂ Cut").on_hover_text("Cut query text to clipboard").clicked() {
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            let _ = cb.set_text(self.pythia_query.clone());
                        }
                        self.pythia_query.clear();
                    }

                    if ui.button("📋 Copy").on_hover_text("Copy query text to clipboard").clicked() {
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            let _ = cb.set_text(self.pythia_query.clone());
                        }
                    }

                    if ui.button("📋 Paste").on_hover_text("Paste clipboard into query field").clicked() {
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            if let Ok(text) = cb.get_text() {
                                self.pythia_query = text;
                            }
                        }
                    }

                    let translate_btn = egui::Button::new(RichText::new("✦ Translate").strong().color(Color32::from_rgb(57, 255, 20)))
                        .fill(Color32::from_rgba_premultiplied(18, 42, 28, 220))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(57, 255, 20)))
                        .rounding(egui::Rounding::same(6.0));
                    if ui.add(translate_btn).clicked() {
                        do_translate = true;
                    }

                    let image_btn = egui::Button::new(RichText::new("🖼 Gen Image").strong().color(Color32::from_rgb(192, 132, 252)))
                        .fill(Color32::from_rgba_premultiplied(32, 20, 48, 220))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(192, 132, 252)))
                        .rounding(egui::Rounding::same(6.0));
                    if ui.add(image_btn).clicked() {
                        do_generate_image = true;
                    }
                });

                // One-click quick image generation prompt presets
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Quick Prompts:").size(10.5).color(Color32::from_rgb(120, 140, 165)));
                    let quick_prompts = [
                        ("🖼 Synthwave", "synthwave neon grid wireframe sunset"),
                        ("🌆 Cyber City", "cyberpunk matrix rain city skyline"),
                        ("👾 Retro Icon", "retro phosphor green terminal icon"),
                        ("🌌 Deep Space", "deep space cosmic nebula stars"),
                        ("⚡ Quantum", "quantum computing golden glowing circuit board"),
                    ];
                    for (lbl, p) in quick_prompts {
                        let btn = egui::Button::new(RichText::new(lbl).size(10.0).color(Color32::from_rgb(192, 132, 252)))
                            .fill(Color32::from_rgba_premultiplied(28, 18, 44, 180))
                            .stroke(Stroke::new(0.8_f32, Color32::from_rgba_premultiplied(168, 85, 247, 80)))
                            .rounding(egui::Rounding::same(4.0));
                        if ui.add(btn).clicked() {
                            self.pythia_query = format!("/image {}", p);
                            do_generate_image = true;
                        }
                    }

                    ui.label(RichText::new("Resolution:").size(10.5).color(Color32::from_rgb(120, 140, 165)));
                    let dims = [((512, 512), "512x512"), ((1024, 1024), "1024x1024")];
                    for (dim, label) in dims {
                        let sel = self.pythia_image_dim == dim;
                        let btn = egui::Button::new(RichText::new(label).size(10.0).color(if sel { Color32::BLACK } else { Color32::from_rgb(56, 189, 248) }))
                            .fill(if sel { Color32::from_rgb(56, 189, 248) } else { Color32::from_rgba_premultiplied(16, 28, 44, 180) })
                            .rounding(egui::Rounding::same(4.0));
                        if ui.add(btn).clicked() {
                            self.pythia_image_dim = dim;
                        }
                    }
                });

                let trimmed_query = self.pythia_query.trim();
                let is_image_prefix = trimmed_query.starts_with("/image ") || trimmed_query.starts_with("/img ");
                if (do_translate || do_generate_image || is_image_prefix) && !trimmed_query.is_empty() {
                    let provider = match self.config.pythia_provider.as_str() {
                        "Gemma Abliterated (Hugging Face)" => well_llm::LlmProvider::HuggingFaceGemmaAbliterated,
                        "Google Gemini 2.0" => well_llm::LlmProvider::Gemini,
                        "Ollama (Local)" => well_llm::LlmProvider::Ollama,
                        _ => well_llm::LlmProvider::OfflineRules,
                    };

                    let (actual_prompt, is_image_req) = if trimmed_query.starts_with("/image ") {
                        (trimmed_query.trim_start_matches("/image ").trim().to_string(), true)
                    } else if trimmed_query.starts_with("/img ") {
                        (trimmed_query.trim_start_matches("/img ").trim().to_string(), true)
                    } else if do_generate_image {
                        (trimmed_query.to_string(), true)
                    } else {
                        (trimmed_query.to_string(), false)
                    };

                    let req = well_llm::TranslationRequest {
                        query: if is_image_req { String::new() } else { actual_prompt.clone() },
                        shell: self.config.shell_path.clone(),
                        cwd: std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
                        provider,
                        gemini_api_key: if self.config.gemini_api_key.trim().is_empty() { None } else { Some(self.config.gemini_api_key.trim().to_string()) },
                        gemini_model: Some(self.config.gemini_model.clone()),
                        ollama_url: Some(self.config.ollama_url.clone()),
                        ollama_model: Some(self.config.ollama_model.clone()),
                        hf_token: if self.config.hf_token.trim().is_empty() { None } else { Some(self.config.hf_token.trim().to_string()) },
                        hf_model: Some(self.config.hf_model.clone()),
                        image_prompt: if is_image_req { Some(actual_prompt) } else { None },
                        image_width: if is_image_req { Some(self.pythia_image_dim.0) } else { None },
                        image_height: if is_image_req { Some(self.pythia_image_dim.1) } else { None },
                    };

                    self.pythia_result = Some(well_llm::PythiaTranslator::translate(&req));
                }

                if let Some(ref res) = self.pythia_result {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Command preview box (only if command was generated)
                    if !res.command.is_empty() {
                        egui::Frame::none()
                            .fill(Color32::from_rgba_premultiplied(8, 10, 18, 240))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_premultiplied(56, 189, 248, 120)))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("$").strong().color(Color32::from_rgb(168, 85, 247)).monospace());
                                    ui.label(RichText::new(&res.command).strong().color(Color32::from_rgb(56, 189, 248)).monospace().size(13.0));
                                });
                            });
                        ui.add_space(4.0);
                    }

                    ui.label(RichText::new(&res.explanation).color(Color32::from_rgb(160, 180, 205)).size(11.5));

                    // Show generated image preview with copy and open actions
                    if let Some(ref img_path) = res.generated_image_path {
                        ui.add_space(6.0);
                        egui::Frame::none()
                            .fill(Color32::from_rgba_premultiplied(12, 16, 28, 240))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(168, 85, 247)))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::same(8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("🖼 Generated Artifact:").strong().color(Color32::from_rgb(192, 132, 252)).size(12.0));
                                    ui.label(RichText::new(img_path).monospace().color(Color32::from_rgb(148, 163, 184)).size(11.0));
                                });
                                ui.add_space(4.0);
                                // Forget cached texture before loading to ensure fresh PNG read
                                let img_uri = format!("file://{}", img_path);
                                if self.last_image_uri.as_deref() != Some(img_path.as_str()) {
                                    if let Some(ref old_uri) = self.last_image_uri {
                                        ui.ctx().forget_image(old_uri);
                                    }
                                    ui.ctx().forget_image(&img_uri);
                                    self.last_image_uri = Some(img_path.clone());
                                }
                                ui.add(
                                    egui::Image::new(&img_uri)
                                        .max_width(280.0)
                                        .max_height(280.0)
                                        .rounding(egui::Rounding::same(6.0))
                                );
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    if ui.button("🔍 Open Image").clicked() {
                                        let _ = std::process::Command::new("open").arg(img_path).spawn();
                                    }
                                    if ui.button("📁 Reveal in Finder").clicked() {
                                        let _ = std::process::Command::new("open").arg("-R").arg(img_path).spawn();
                                    }
                                    if ui.button("⚡ Insert Path to Prompt").clicked() {
                                        self.pending_pty_injection = Some(format!("{}\n", img_path));
                                        close_modal = true;
                                    }
                                    if ui.button("📁 Cache Folder").clicked() {
                                        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                                        let cache_dir = std::path::Path::new(&home).join(".well").join("generated");
                                        let _ = std::process::Command::new("open").arg(&cache_dir).spawn();
                                    }
                                    if ui.button("↺ Regenerate").clicked() {
                                        do_generate_image = true;
                                    }
                                });
                            });
                    }

                    ui.add_space(6.0);
                    if res.is_destructive {
                        egui::Frame::none()
                            .fill(Color32::from_rgba_premultiplied(48, 12, 12, 220))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(239, 68, 68)))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new("⚠ SAFETY GATE: Potentially destructive disk/file operations detected. Verify thoroughly before execution.").color(Color32::from_rgb(248, 113, 113)).size(10.5));
                            });
                        ui.add_space(4.0);
                    }
                    ui.horizontal(|ui| {
                        if res.is_destructive {
                            ui.label(RichText::new("⚠ DESTRUCTIVE").color(Color32::from_rgb(239, 68, 68)).strong().size(10.5));
                        } else {
                            ui.label(RichText::new("✓ SAFE").color(Color32::from_rgb(57, 255, 20)).strong().size(10.5));
                        }
                        ui.label(RichText::new(format!("Engine: {}", res.provider_used)).color(Color32::from_rgb(140, 160, 185)).size(10.5));
                        ui.label(RichText::new(format!("Confidence: {:.0}%", res.confidence * 100.0)).color(Color32::from_rgb(250, 204, 21)).size(10.5));
                    });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if !res.command.is_empty() {
                            let run_btn = egui::Button::new(RichText::new("⚡ Run in Shell").strong().color(Color32::from_rgb(57, 255, 20)))
                                .fill(Color32::from_rgba_premultiplied(18, 42, 28, 220))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(57, 255, 20)))
                                .rounding(egui::Rounding::same(6.0));
                            if ui.add(run_btn).on_hover_text("Execute directly in current terminal session").clicked() {
                                self.pending_pty_injection = Some(format!("{}\n", res.command));
                                close_modal = true;
                            }

                            let insert_btn = egui::Button::new(RichText::new("✏ Insert into Prompt").strong().color(Color32::from_rgb(56, 189, 248)))
                                .fill(Color32::from_rgba_premultiplied(16, 28, 44, 220))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)))
                                .rounding(egui::Rounding::same(6.0));
                            if ui.add(insert_btn).on_hover_text("Insert command into prompt line for manual editing").clicked() {
                                self.pending_pty_injection = Some(res.command.clone());
                                close_modal = true;
                            }

                            let copy_cmd_btn = egui::Button::new(RichText::new("📋 Copy Command").strong().color(Color32::from_rgb(250, 204, 21)))
                                .fill(Color32::from_rgba_premultiplied(40, 30, 10, 220))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(250, 204, 21)))
                                .rounding(egui::Rounding::same(6.0));
                            if ui.add(copy_cmd_btn).on_hover_text("Copy generated command to OS clipboard").clicked() {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    let _ = cb.set_text(res.command.clone());
                                }
                            }
                        }

                        let copy_exp_btn = egui::Button::new(RichText::new("📄 Copy Explanation").strong().color(Color32::from_rgb(192, 132, 252)))
                            .fill(Color32::from_rgba_premultiplied(32, 20, 48, 220))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(192, 132, 252)))
                            .rounding(egui::Rounding::same(6.0));
                        if ui.add(copy_exp_btn).on_hover_text("Copy full response/explanation to OS clipboard").clicked() {
                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                let _ = cb.set_text(res.explanation.clone());
                            }
                        }

                        if ui.button("✕ Close").clicked() {
                            close_modal = true;
                        }
                    });
                }
            });

        if close_modal {
            self.show_pythia_hud = false;
        }
    }

    // Helper to list profile file names (without extension)
    fn list_profile_names(&self) -> Vec<String> {
        if let Some(mut dir) = Self::config_path().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
            dir.push("profiles");
            if let Ok(entries) = std::fs::read_dir(&dir) {
                let mut names = Vec::new();
                for entry in entries.flatten() {
                    if let Ok(path) = entry.path().canonicalize() {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            names.push(stem.to_string());
                        }
                    }
                }
                return names;
            }
        }
        Vec::new()
    }

    // Save current config as a named profile JSON file
    fn save_profile(&mut self, name: &str) -> Result<(), String> {
        if let Some(mut dir) = Self::config_path().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
            dir.push("profiles");
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join(format!("{}.json", name));
            let json = serde_json::to_string_pretty(&self.config).map_err(|e| e.to_string())?;
            std::fs::write(&path, json).map_err(|e| e.to_string())?;
            self.current_profile = Some(name.to_string());
            self.status_notification = Some((format!("Saved profile {}", name), Instant::now()));
            Ok(())
        } else {
            Err("Unable to determine config directory".into())
        }
    }

    // Load a named profile and apply it
    fn load_profile(&mut self, name: &str) -> Result<(), String> {
        if let Some(mut dir) = Self::config_path().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
            dir.push("profiles");
            let path = dir.join(format!("{}.json", name));
            let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let payload: TheiaConfigPayload = serde_json::from_str(&data).map_err(|e| e.to_string())?;
            self.config = payload;
            self.channel.sync_state(self.config.clone());
            self.current_profile = Some(name.to_string());
            self.status_notification = Some((format!("Loaded profile {}", name), Instant::now()));
            Ok(())
        } else {
            Err("Unable to determine config directory".into())
        }
    }

    pub fn render(&mut self, ctx: &Context) {
        self.render_window(ctx);
    }
}

pub const JETPACK_STARSHIP_PRESET: &str = r#"# version: 1.0.0
"$schema" = 'https://starship.rs/config-schema.json'

add_newline = true
continuation_prompt = "[▸▹ ](dimmed white)"

format = """($nix_shell$container$fill$git_metrics\n)$cmd_duration\
$hostname\
$localip\
$shlvl\
$shell\
$env_var\
$jobs\
$sudo\
$username\
$character"""

right_format = """
$singularity\
$kubernetes\
$directory\
$vcsh\
$fossil_branch\
$git_branch\
$git_commit\
$git_state\
$git_status\
$hg_branch\
$pijul_channel\
$docker_context\
$package\
$c\
$cpp\
$cmake\
$cobol\
$daml\
$dart\
$deno\
$dotnet\
$elixir\
$elm\
$erlang\
$fennel\
$fortran\
$golang\
$guix_shell\
$haskell\
$haxe\
$helm\
$java\
$julia\
$kotlin\
$gradle\
$lua\
$maven\
$nim\
$nodejs\
$bun\
$ocaml\
$opa\
$perl\
$php\
$pulumi\
$purescript\
$python\
$raku\
$rlang\
$red\
$ruby\
$rust\
$scala\
$solidity\
$swift\
$terraform\
$vlang\
$vagrant\
$xmake\
$zig\
$buf\
$conda\
$pixi\
$meson\
$spack\
$memory_usage\
$aws\
$gcloud\
$openstack\
$azure\
$crystal\
$custom\
$status\
$os\
$battery\
$time"""

[fill]
symbol = ' '

[character]
format = "$symbol "
success_symbol = "[◎](bold italic bright-yellow)"
error_symbol = "[○](italic purple)"
vimcmd_symbol = "[■](italic dimmed green)"
vimcmd_replace_one_symbol = "◌"
vimcmd_replace_symbol = "□"
vimcmd_visual_symbol = "▼"

[env_var.VIMSHELL]
format = "[$env_value]($style)"
style = 'green italic'

[sudo]
format = "[$symbol]($style)"
style = "bold italic bright-purple"
symbol = "⋈┈"
disabled = false

[username]
style_user = "bright-yellow bold italic"
style_root = "purple bold italic"
format = "[⭘ $user]($style) "
disabled = false
show_always = false

[directory]
home_symbol = "⌂"
truncation_length = 2
truncation_symbol = "□ "
read_only = " ◈"
use_os_path_sep = true
style = "italic blue"
format = '[$path]($style)[$read_only]($read_only_style)'
repo_root_style = 'bold blue'
repo_root_format = '[$before_root_path]($before_repo_root_style)[$repo_root]($repo_root_style)[$path]($style)[$read_only]($read_only_style) [△](bold bright-blue)'

[cmd_duration]
format = "[◄ $duration ](italic white)"

[jobs]
format = "[$symbol$number]($style) "
style = "white"
symbol = "[▶](blue italic)"

[localip]
ssh_only = true
format = " ◯[$localipv4](bold magenta)"
disabled = false

[time]
disabled = false
format = "[ $time]($style)"
time_format = "%R"
utc_time_offset = "local"
style = "italic dimmed white"

[battery]
format = "[ $percentage $symbol]($style)"
full_symbol = "█"
charging_symbol = "[↑](italic bold green)"
discharging_symbol = "↓"
unknown_symbol = "░"
empty_symbol = "▃"

[[battery.display]]
threshold = 20
style = "italic bold red"

[[battery.display]]
threshold = 60
style = "italic dimmed bright-purple"

[[battery.display]]
threshold = 70
style = "italic dimmed yellow"

[git_branch]
format = " [$branch(:$remote_branch)]($style)"
symbol = "[△](bold italic bright-blue)"
style = "italic bright-blue"
truncation_symbol = "⋯"
truncation_length = 11
ignore_branches = ["main", "master"]
only_attached = true

[git_metrics]
format = '([▴$added]($added_style))([▿$deleted]($deleted_style))'
added_style = 'italic dimmed green'
deleted_style = 'italic dimmed red'
ignore_submodules = true
disabled = false

[git_status]
style = "bold italic bright-blue"
format = "([⎪$ahead_behind$staged$modified$untracked$renamed$deleted$conflicted$stashed⎥]($style))"
conflicted = "[◪◦](italic bright-magenta)"
ahead = "[▴│[${count}](bold white)│](italic green)"
behind = "[▿│[${count}](bold white)│](italic red)"
diverged = "[◇ ▴┤[${ahead_count}](regular white)│▿┤[${behind_count}](regular white)│](italic bright-magenta)"
untracked = "[◌◦](italic bright-yellow)"
stashed = "[◃◈](italic white)"
modified = "[●◦](italic yellow)"
staged = "[▪┤[$count](bold white)│](italic bright-cyan)"
renamed = "[◎◦](italic bright-blue)"
deleted = "[✕](italic red)"

[deno]
format = " [deno](italic) [∫ $version](green bold)"
version_format = "${raw}"

[lua]
format = " [lua](italic) [${symbol}${version}]($style)"
version_format = "${raw}"
symbol = "⨀ "
style = "bold bright-yellow"

[nodejs]
format = " [node](italic) [◫ ($version)](bold bright-green)"
version_format = "${raw}"
detect_files = ["package-lock.json", "yarn.lock"]
detect_folders = ["node_modules"]
detect_extensions = []

[bun]
format = " [bun](italic) [◫ ($version)](bold bright-green)"
version_format = "${raw}"

[python]
format = " [py](italic) [${symbol}${version}]($style)"
symbol = "[⌉](bold bright-blue)⌊ "
version_format = "${raw}"
style = "bold bright-yellow"

[ruby]
format = " [rb](italic) [${symbol}${version}]($style)"
symbol = "◆ "
version_format = "${raw}"
style = "bold red"

[rust]
format = " [rs](italic) [$symbol$version]($style)"
symbol = "⊃ "
version_format = "${raw}"
style = "bold red"

[package]
format = " [pkg](italic dimmed) [$symbol$version]($style)"
version_format = "${raw}"
symbol = "◨ "
style = "dimmed yellow italic bold"

[swift]
format = " [sw](italic) [${symbol}${version}]($style)"
symbol = "◁ "
style = "bold bright-red"
version_format = "${raw}"
"#;

pub fn apply_starship_preset(preset_name: &str) -> Result<String, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|e| e.to_string())?;
    let config_dir = PathBuf::from(home).join(".config");
    let starship_path = config_dir.join("starship.toml");
    let _ = std::fs::create_dir_all(&config_dir);

    match preset_name.to_lowercase().as_str() {
        "jetpack" => {
            // Apply Jetpack preset
            let written_embedded = || -> Result<String, String> {
                std::fs::write(&starship_path, JETPACK_STARSHIP_PRESET)
                    .map_err(|e| format!("Failed to write {}: {}", starship_path.display(), e))?;
                Ok("Activated Jetpack preset in ~/.config/starship.toml".to_string())
            };

            if let Ok(output) = std::process::Command::new("starship").args(["preset", "jetpack"]).output() {
                if output.status.success() && !output.stdout.is_empty() {
                    std::fs::write(&starship_path, &output.stdout)
                        .map_err(|e| format!("Failed to write {}: {}", starship_path.display(), e))?;
                    Ok(format!("Activated official Jetpack preset in {}", starship_path.display()))
                } else {
                    written_embedded()
                }
            } else {
                written_embedded()
            }
        }
        other => {
            let arg_name = match other {
                "tokyo night" => "tokyo-night",
                "pure minimal" => "pure-preset",
                "gruvbox rainbow" => "gruvbox-rainbow",
                "pastel powerline" => "pastel-powerline",
                _ => other,
            };

            if let Ok(output) = std::process::Command::new("starship").args(["preset", arg_name]).output() {
                if output.status.success() && !output.stdout.is_empty() {
                    std::fs::write(&starship_path, &output.stdout)
                        .map_err(|e| format!("Failed to write {}: {}", starship_path.display(), e))?;
                    Ok(format!("Activated '{}' preset in {}", preset_name, starship_path.display()))
                } else {
                    Err(format!("Starship preset '{}' not recognized by CLI", preset_name))
                }
            } else {
                Err("Starship CLI not found to generate preset".to_string())
            }
        }
    }
}

/// Paints the cybernetic Oroboros dragon-scale frame around the terminal viewport,
/// including repeating interlocking chevrons along borders and multi-layered armor brackets at corners.
pub fn draw_oroboros_scale_frame(painter: &egui::Painter, screen_rect: Rect, top_bar_h: f32, margin: f32) {
    let w = screen_rect.width();
    let h = screen_rect.height();
    if w <= 2.0 * margin || h <= top_bar_h + margin {
        return;
    }

    let viewport_rect = Rect::from_min_max(
        pos2(margin, top_bar_h),
        pos2(w - margin, h - margin),
    );

    // 1. Dark outer bezel framing the window edges
    let bezel_fill = Color32::from_rgba_premultiplied(10, 13, 19, 250);
    painter.rect_filled(Rect::from_min_max(pos2(0.0, top_bar_h), pos2(margin, h)), 0.0, bezel_fill);
    painter.rect_filled(Rect::from_min_max(pos2(w - margin, top_bar_h), pos2(w, h)), 0.0, bezel_fill);
    painter.rect_filled(Rect::from_min_max(pos2(0.0, h - margin), pos2(w, h)), 0.0, bezel_fill);

    // 2. Inner hairline border enclosing the terminal viewport
    let inner_border_color = Color32::from_rgba_premultiplied(56, 189, 248, 65); // Cyber cyan hint
    painter.rect_stroke(
        viewport_rect,
        egui::Rounding::same(2.0),
        Stroke::new(1.0_f32, inner_border_color),
    );

    // 3. Repeating Ouroboros scales along Left & Right vertical borders
    let scale_step = 16.0;
    let mut y = top_bar_h + 8.0;
    let mut idx = 0;
    while y + scale_step <= h - margin - 4.0 {
        // Color rhythm: alternate between deep gunmetal, electric purple, and cyber cyan edge glows
        let glow_color = match idx % 4 {
            0 => Color32::from_rgba_premultiplied(168, 85, 247, 180), // Neon Purple
            1 => Color32::from_rgba_premultiplied(56, 189, 248, 160),  // Electric Cyan
            2 => Color32::from_rgba_premultiplied(192, 132, 252, 140), // Light Violet
            _ => Color32::from_rgba_premultiplied(14, 165, 233, 120),  // Deep Cyan
        };

        // Left border scale: Angular chevron plate pointing downwards
        let l_top = pos2(1.0, y);
        let l_mid = pos2(margin - 1.0, y + scale_step * 0.5);
        let l_bot = pos2(1.0, y + scale_step);
        let l_inner = pos2(2.5, y + scale_step * 0.5);

        painter.add(egui::Shape::convex_polygon(
            vec![l_top, l_mid, l_bot, l_inner],
            Color32::from_rgba_premultiplied(18, 24, 36, 220),
            Stroke::new(0.8_f32, glow_color),
        ));

        // Right border scale: Symmetric angular chevron plate
        let r_top = pos2(w - 1.0, y);
        let r_mid = pos2(w - margin + 1.0, y + scale_step * 0.5);
        let r_bot = pos2(w - 1.0, y + scale_step);
        let r_inner = pos2(w - 2.5, y + scale_step * 0.5);

        painter.add(egui::Shape::convex_polygon(
            vec![r_top, r_mid, r_bot, r_inner],
            Color32::from_rgba_premultiplied(18, 24, 36, 220),
            Stroke::new(0.8_f32, glow_color),
        ));

        y += scale_step;
        idx += 1;
    }

    // 4. Repeating Ouroboros scales along Bottom horizontal border
    let mut x = margin + 12.0;
    let b_scale_w = 18.0;
    let mut b_idx = 0;
    while x + b_scale_w <= w - margin - 12.0 {
        let glow_color = match b_idx % 4 {
            0 => Color32::from_rgba_premultiplied(168, 85, 247, 180), // Neon Purple
            1 => Color32::from_rgba_premultiplied(56, 189, 248, 160),  // Electric Cyan
            2 => Color32::from_rgba_premultiplied(192, 132, 252, 140), // Light Violet
            _ => Color32::from_rgba_premultiplied(14, 165, 233, 120),  // Deep Cyan
        };

        let b_left = pos2(x, h - margin + 1.0);
        let b_tip = pos2(x + b_scale_w * 0.5, h - 1.0);
        let b_right = pos2(x + b_scale_w, h - margin + 1.0);
        let b_inner = pos2(x + b_scale_w * 0.5, h - margin + 2.5);

        painter.add(egui::Shape::convex_polygon(
            vec![b_left, b_tip, b_right, b_inner],
            Color32::from_rgba_premultiplied(18, 24, 36, 220),
            Stroke::new(0.8_f32, glow_color),
        ));

        x += b_scale_w;
        b_idx += 1;
    }

    // 5. Four Corner Armor Scale Plates (Dragon Crest Brackets)
    let corner_size = 14.0;
    
    // Bottom-Left Corner Crest: overlapping layered scale chevron
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(0.5, h - corner_size * 1.6),
            pos2(corner_size * 1.6, h - 0.5),
            pos2(0.5, h - 0.5),
        ],
        Color32::from_rgba_premultiplied(22, 28, 44, 255),
        Stroke::new(1.2_f32, Color32::from_rgba_premultiplied(168, 85, 247, 220)),
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(2.0, h - corner_size),
            pos2(corner_size, h - 2.0),
            pos2(2.0, h - 2.0),
        ],
        Color32::from_rgba_premultiplied(32, 40, 60, 255),
        Stroke::new(0.8_f32, Color32::from_rgba_premultiplied(56, 189, 248, 200)),
    ));
    painter.circle_filled(pos2(4.5, h - 4.5), 1.5, Color32::from_rgb(180, 110, 255));

    // Bottom-Right Corner Crest: symmetric layered scale chevron
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(w - 0.5, h - corner_size * 1.6),
            pos2(w - corner_size * 1.6, h - 0.5),
            pos2(w - 0.5, h - 0.5),
        ],
        Color32::from_rgba_premultiplied(22, 28, 44, 255),
        Stroke::new(1.2_f32, Color32::from_rgba_premultiplied(56, 189, 248, 220)),
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(w - 2.0, h - corner_size),
            pos2(w - corner_size, h - 2.0),
            pos2(w - 2.0, h - 2.0),
        ],
        Color32::from_rgba_premultiplied(32, 40, 60, 255),
        Stroke::new(0.8_f32, Color32::from_rgba_premultiplied(168, 85, 247, 200)),
    ));
    painter.circle_filled(pos2(w - 4.5, h - 4.5), 1.5, Color32::from_rgb(56, 189, 248));

    // Top-Left Crest (just below top bar)
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(0.5, top_bar_h + corner_size),
            pos2(corner_size, top_bar_h + 0.5),
            pos2(0.5, top_bar_h + 0.5),
        ],
        Color32::from_rgba_premultiplied(22, 28, 44, 255),
        Stroke::new(1.2_f32, Color32::from_rgba_premultiplied(168, 85, 247, 220)),
    ));
    painter.circle_filled(pos2(4.5, top_bar_h + 4.5), 1.5, Color32::from_rgb(180, 110, 255));

    // Top-Right Crest (just below top bar)
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(w - 0.5, top_bar_h + corner_size),
            pos2(w - corner_size, top_bar_h + 0.5),
            pos2(w - 0.5, top_bar_h + 0.5),
        ],
        Color32::from_rgba_premultiplied(22, 28, 44, 255),
        Stroke::new(1.2_f32, Color32::from_rgba_premultiplied(56, 189, 248, 220)),
    ));
    painter.circle_filled(pos2(w - 4.5, top_bar_h + 4.5), 1.5, Color32::from_rgb(56, 189, 248));
}

