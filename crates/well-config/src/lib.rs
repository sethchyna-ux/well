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
use egui::{Color32, Context, RichText, ScrollArea, Slider, Stroke, Ui};
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistentConfig {
    pub theia: TheiaConfigPayload,
    pub shell_path: String,
    pub active_font_name: String,
    pub abbreviations: Vec<ShellAbbreviation>,
    pub keybindings: Vec<KeybindingRow>,
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
    // Undo/Redo stacks for config snapshots
    pub undo_stack: Vec<TheiaConfigPayload>,
    pub redo_stack: Vec<TheiaConfigPayload>,
    // Current profile name (if any)
    pub current_profile: Option<String>,
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
            KeybindingRow { chord: "Cmd+D".to_string(), action: "Split Pane Horizontal".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+Shift+D".to_string(), action: "Split Pane Vertical".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+F".to_string(), action: "Find in Scrollback".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+K".to_string(), action: "Clear Buffer".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+=".to_string(), action: "Increase Font Size".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+-".to_string(), action: "Decrease Font Size".to_string(), conflict: false },
        ];

        let default_shell = if std::path::Path::new("/opt/homebrew/bin/fish").exists() {
            "/opt/homebrew/bin/fish".to_string()
        } else if std::path::Path::new("/usr/local/bin/fish").exists() {
            "/usr/local/bin/fish".to_string()
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
        };

        let mut panel = Self {
            channel,
            config: initial_config,
            shell_path: default_shell,
            active_font_name: "OpenDyslexic Nerd Font (Active)".to_string(),
            abbreviations: initial_abbrevs,
            new_keyword: String::new(),
            new_expansion: String::new(),
            keybindings: initial_bindings,
            new_chord: String::new(),
            new_action: String::new(),
            active_tab: 0,
            is_open: false, // Closed by default on launch for clean workspace
            status_notification: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_profile: None,
        };

        // Attempt to load saved config from disk if available
        panel.try_load_from_disk();
        panel
    }

    pub fn config_path() -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".config").join("well").join("config.json"))
    }

    pub fn hyprlang_path() -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
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

        let mut toggle_clicked = false;

        // 2. Sleek minimalist HUD pill in top-right corner
        egui::Area::new(egui::Id::new("settings_hud_pill"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 16.0))
            .show(ctx, |ui| {
                let (btn_text, text_color, border_color) = if self.is_open {
                    ("✕ Close Settings (Esc)", Color32::from_rgb(255, 100, 130), Color32::from_rgba_premultiplied(255, 80, 120, 120))
                } else {
                    ("⚙ Settings (Cmd+,)", Color32::from_rgb(180, 200, 220), Color32::from_rgba_premultiplied(56, 189, 248, 70))
                };
                let btn = egui::Button::new(RichText::new(btn_text).size(11.5).strong().color(text_color))
                    .fill(Color32::from_rgba_premultiplied(16, 20, 30, 210))
                    .stroke(Stroke::new(1.0f32, border_color))
                    .rounding(egui::Rounding::same(16.0));
                if ui.add(btn).clicked() {
                    toggle_clicked = true;
                }
            });

        if toggle_clicked {
            self.is_open = !self.is_open;
        }

        if !self.is_open {
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
        .default_width(680.0)
        .default_height(580.0)
        .min_width(520.0)
        .min_height(420.0)
        .resizable(true)
        .movable(true)
        .collapsible(false)
        .default_pos(egui::pos2(120.0, 60.0))
        .show(ctx, |ui| {
            // Modern Segmented Tab Bar Navigation
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                let tabs: [(usize, &str, Color32); 8] = [
                    (0, "Appearance", Color32::from_rgb(57, 255, 20)),
                    (1, "Typography", Color32::from_rgb(56, 189, 248)),
                    (2, "Shortcuts", Color32::from_rgb(250, 204, 21)),
                    (3, "Metis Shell", Color32::from_rgb(255, 0, 127)),
                    (4, "CRT Shaders", Color32::from_rgb(168, 85, 247)),
                    (5, "Profiles & Hyprlang", Color32::from_rgb(0, 240, 255)),
                    (6, "Scripts & Engine", Color32::from_rgb(57, 255, 20)),
                    (7, "Help & Docs", Color32::from_rgb(148, 163, 184)),
                ];

                for (idx, label, accent) in tabs {
                    let is_active = self.active_tab == idx;
                    let (fill, stroke, text_color) = if is_active {
                        (
                            Color32::from_rgba_premultiplied(accent.r() / 5, accent.g() / 5, accent.b() / 5, 200),
                            Stroke::new(1.0f32, accent),
                            accent,
                        )
                    } else {
                        (
                            Color32::from_rgba_premultiplied(22, 28, 40, 160),
                            Stroke::new(1.0f32, Color32::from_rgba_premultiplied(255, 255, 255, 20)),
                            Color32::from_rgb(148, 163, 184),
                        )
                    };

                    let btn = egui::Button::new(RichText::new(label).size(12.0).strong().color(text_color))
                        .fill(fill)
                        .stroke(stroke)
                        .rounding(egui::Rounding::same(8.0));
                    if ui.add(btn).clicked() {
                        self.active_tab = idx;
                    }
                }
            });
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Active Tab Content
            ScrollArea::vertical().show(ui, |ui| {
                match self.active_tab {
                    0 => self.render_appearance_tab(ui),
                    1 => self.render_typography_tab(ui),
                    2 => self.render_shortcuts_tab(ui),
                    3 => self.render_shell_tab(ui),
                    4 => self.render_shaders_tab(ui),
                    5 => self.render_profiles_tab(ui),
                    6 => self.render_script_tab(ui),
                    7 => self.render_help_tab(ui),
                    _ => self.render_appearance_tab(ui),
                }
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
            self.active_font_name = f;
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
        ui.add_space(4.0);

        let mut to_delete = None;
        for (idx, kb) in self.keybindings.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut kb.chord);
                ui.text_edit_singleline(&mut kb.action);
                if ui.button("❌").clicked() {
                    to_delete = Some(idx);
                }
            });
        }
        if let Some(idx) = to_delete {
            self.keybindings.remove(idx);
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.new_chord);
            ui.text_edit_singleline(&mut self.new_action);
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
        ui.text_edit_singleline(&mut self.shell_path);

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
        ui.heading(RichText::new("Well Terminal — Architecture & Shortcuts").color(Color32::from_rgb(0, 240, 255)));
        ui.add_space(6.0);
        ui.label(RichText::new("Global Hotkeys:").strong());
        ui.label("• Cmd+, or F12 / F1: Toggle Theia Control Center");
        ui.label("• Esc: Close Theia Control Center");
        ui.label("• Cmd+D / Cmd+Shift+D: Split Pane Horizontal / Vertical");
        ui.label("• Cmd+K: Clear Terminal Buffer");
        ui.label("• Cmd+= / Cmd+-: Increase / Decrease Font Size");
        ui.add_space(8.0);
        ui.label(RichText::new("Subsystem Telemetry:").strong());
        ui.label("• ATLAS: Host Windowing & Event Loop (winit/Metal)");
        ui.label("• ORPHEUS: GPU Text Shaping & WGPU Cell Matrix");
        ui.label("• METIS: Shell Logic Core & Prefix-Trie History Engine");
        ui.label("• HERMES: Zero-Lock Atomic Seqlock Inter-Process Bus");
        ui.label("• HYPRLANG: Native Wayland/Hyprlang (.hl) Parser & Emitter");
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
