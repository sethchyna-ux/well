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
use egui::{Color32, Context, RichText, ScrollArea, Slider, Ui};
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

        let mut panel = Self {
            channel,
            config: initial_config,
            shell_path: std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string()),
            active_font_name: "OpenDyslexic Nerd Font (Active)".to_string(),
            abbreviations: initial_abbrevs,
            new_keyword: String::new(),
            new_expansion: String::new(),
            keybindings: initial_bindings,
            new_chord: String::new(),
            new_action: String::new(),
            active_tab: 0,
            is_open: true, // Open by default on launch so settings are immediately visible
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
        let mut undo_clicked = false;
        let mut redo_clicked = false;
        let mut toggle_clicked = false;

        egui::Area::new(egui::Id::new("settings_hud_button"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 16.0))
            .show(ctx, |ui| {
                let btn_text = if self.is_open { "✖ Close Settings" } else { "⚙️ Settings (Cmd+,)" };
                if ui.button("↶ Undo").clicked() && !self.undo_stack.is_empty() {
                    undo_clicked = true;
                }
                if ui.button("↷ Redo").clicked() && !self.redo_stack.is_empty() {
                    redo_clicked = true;
                }
                let color = if self.is_open { Color32::from_rgb(255, 0, 127) } else { Color32::from_rgb(57, 255, 20) };
                if ui.button(RichText::new(btn_text).color(color).strong()).clicked() {
                    toggle_clicked = true;
                }
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
        if toggle_clicked {
            self.is_open = !self.is_open;
        }

        if !self.is_open {
            return;
        }

        let mut is_open = self.is_open;
        let mut apply_clicked = false;
        let mut save_clicked = false;
        let mut reset_clicked = false;

        egui::Window::new(RichText::new("🏛️ Theia's Prism — Control Center").color(Color32::from_rgb(57, 255, 20)).strong())
            .open(&mut is_open)
            .default_width(540.0)
            .default_height(580.0)
            .resizable(true)
            .show(ctx, |ui| {
                // Top Tab Bar Navigation
                ui.horizontal(|ui| {
                    let c_green = Color32::from_rgb(57, 255, 20);
                    let c_blue = Color32::from_rgb(56, 189, 248);
                    let c_magenta = Color32::from_rgb(255, 0, 127);
                    let c_purple = Color32::from_rgb(168, 85, 247);
                    let c_amber = Color32::from_rgb(250, 204, 21);
                    let c_cyan = Color32::from_rgb(0, 240, 255);

                    if ui.selectable_label(self.active_tab == 0, RichText::new("🎨 Appearance").color(if self.active_tab == 0 { c_green } else { Color32::GRAY })).clicked() { self.active_tab = 0; }
                    if ui.selectable_label(self.active_tab == 1, RichText::new("🔤 Typography").color(if self.active_tab == 1 { c_blue } else { Color32::GRAY })).clicked() { self.active_tab = 1; }
                    if ui.selectable_label(self.active_tab == 2, RichText::new("⌨️ Shortcuts").color(if self.active_tab == 2 { c_amber } else { Color32::GRAY })).clicked() { self.active_tab = 2; }
                    if ui.selectable_label(self.active_tab == 3, RichText::new("🐚 Metis Shell").color(if self.active_tab == 3 { c_magenta } else { Color32::GRAY })).clicked() { self.active_tab = 3; }
                    if ui.selectable_label(self.active_tab == 4, RichText::new("📺 CRT Shaders").color(if self.active_tab == 4 { c_purple } else { Color32::GRAY })).clicked() { self.active_tab = 4; }
                    if ui.selectable_label(self.active_tab == 5, RichText::new("💾 Profiles").color(if self.active_tab == 5 { c_cyan } else { Color32::GRAY })).clicked() { self.active_tab = 5; }
                    if ui.selectable_label(self.active_tab == 6, RichText::new("📜 Script").color(if self.active_tab == 6 { c_green } else { Color32::GRAY })).clicked() { self.active_tab = 6; }
                    if ui.selectable_label(self.active_tab == 7, RichText::new("▶️ Run").color(if self.active_tab == 7 { c_blue } else { Color32::GRAY })).clicked() { self.active_tab = 7; }
                    if ui.selectable_label(self.active_tab == 8, RichText::new("🚀 Go").color(if self.active_tab == 8 { c_magenta } else { Color32::GRAY })).clicked() { self.active_tab = 8; }
                    if ui.selectable_label(self.active_tab == 9, RichText::new("📁 File").color(if self.active_tab == 9 { c_amber } else { Color32::GRAY })).clicked() { self.active_tab = 9; }
                    if ui.selectable_label(self.active_tab == 10, RichText::new("❓ Help").color(if self.active_tab == 10 { c_cyan } else { Color32::GRAY })).clicked() { self.active_tab = 10; }
                });
                ui.separator();

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
                        7 => self.render_run_tab(ui),
                        8 => self.render_go_tab(ui),
                        9 => self.render_file_tab(ui),
                        10 => self.render_help_tab(ui),
                        _ => self.render_profiles_tab(ui),
                    }
                });

                ui.separator();

                // Bottom Action & Telemetry Bar
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("⚡ Apply Live").color(Color32::from_rgb(57, 255, 20)).strong()).clicked() {
                        apply_clicked = true;
                    }
                    if ui.button(RichText::new("💾 Save Config").color(Color32::from_rgb(56, 189, 248))).clicked() {
                        save_clicked = true;
                    }
                    if ui.button(RichText::new("🔄 Reset").color(Color32::from_rgb(255, 0, 127))).clicked() {
                        reset_clicked = true;
                    }

                    if let Some((msg, created)) = &self.status_notification {
                        if created.elapsed().as_secs() < 4 {
                            ui.label(RichText::new(msg).color(Color32::from_rgb(57, 255, 20)));
                        }
                    } else {
                        ui.label(RichText::new("Lock-free atomic Seqlock active").color(Color32::from_rgb(148, 163, 184)));
                    }
                });
            });

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

        self.is_open = is_open;
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
            if ui.button("💾 Save Profile").clicked() {
                save_profile_req = true;
            }
            if ui.button("📂 Load Profile").clicked() {
                load_profile_req = true;
            }
            if ui.button("📥 Import Hyprlang (.hl)").clicked() {
                import_hyprlang_req = true;
            }
            if ui.button("📤 Export Hyprlang (.hl)").clicked() {
                export_hyprlang_req = true;
            }
            if ui.button("🔄 Reset Defaults").clicked() {
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
        ui.heading(RichText::new("📝 Script Tab").color(Color32::from_rgb(57, 255, 20)));
        ui.label("Placeholder for script management UI.");
    }

    fn render_run_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("▶️ Run Tab").color(Color32::from_rgb(56, 189, 248)));
        ui.label("Placeholder for run commands UI.");
    }

    fn render_go_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("🚀 Go Tab").color(Color32::from_rgb(255, 0, 127)));
        ui.label("Placeholder for Go language integration UI.");
    }

    fn render_file_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("📁 File Tab").color(Color32::from_rgb(250, 204, 21)));
        ui.label("Placeholder for file explorer UI.");
    }

    fn render_help_tab(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("❓ Help Tab").color(Color32::from_rgb(0, 240, 255)));
        ui.label("Placeholder for help and documentation UI.");
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
