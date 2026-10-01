/// Theia's Prism: Visual Control Center and Configuration Dashboard Component for Well.
/// Built on top of egui for zero-overhead, GPU-accelerated immediate mode GUI layout
/// synchronized natively over WebGPU (wgpu) within Orpheus's composition loop.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

// Simulated imported types representing our custom subsystems
use crate::astraea::PromptState;
use crate::hermes::SeqLock;
use crate::metis::ShellHistoryTrie;
use crate::mneme::TreeSitterValidator;

/// Mock namespaces to represent Well's internal structures
mod mock_modules {
    use super::*;

    #[derive(Clone, Default)]
    pub struct ShellAbbreviation {
        pub keyword: String,
        pub expansion: String,
    }

    #[derive(Clone, Default)]
    pub struct KeybindingRow {
        pub chord: String,
        pub action: String,
        pub conflict: bool,
    }

    #[derive(Clone, Default)]
    pub struct ShaderUniforms {
        pub scanline_frequency: f32,
        pub screen_curvature: f32,
        pub glow_radius: f32,
        pub blur_intensity: f32,
    }
}
use mock_modules::*;

/// State container backing Theia's Prism control panel interface.
pub struct TheiasPrismPanel {
    // Astraea (Starship Prompt Compiler) Visual Handles
    active_prompt_preset: String,
    enable_git_status: Arc<AtomicBool>,
    enable_directory_context: Arc<AtomicBool>,
    enable_toolchain_versions: Arc<AtomicBool>,
    enable_aws_profile: Arc<AtomicBool>,
    scan_timeout_ms: Arc<AtomicU32>,
    command_timeout_ms: Arc<AtomicU32>,
    enable_transience: Arc<AtomicBool>,

    // Metis (Fish Shell Logic Core) Visual Handles
    abbreviations: Arc<RwLock<Vec<ShellAbbreviation>>>,
    new_keyword: String,
    new_expansion: String,
    history_limit: Arc<AtomicU32>,
    keybindings: Vec<KeybindingRow>,

    // Orpheus (WGPU Texture Renderer) Shader Settings
    active_shader: String,
    shader_uniforms: ShaderUniforms,
    wgpu_pipeline_valid: bool,

    // Mneme (B-Tree Rope Editor) Validation State
    ts_validator: TreeSitterValidator,
    config_input_buffer: String,
    ts_ast_valid: bool,
    ts_error_message: Option<String>,

    // Hermes (PTY-Bypass / Socket IPC) Stream Status
    active_socket_clients: u32,
    last_agent_id: String,
    seqlock: Arc<SeqLock>,
}

impl TheiasPrismPanel {
    pub fn new(seqlock: Arc<SeqLock>) -> Self {
        // Mock initial abbreviation dataset
        let initial_abbrevs = vec![
            ShellAbbreviation { keyword: "gco".to_string(), expansion: "git checkout".to_string() },
            ShellAbbreviation { keyword: "lg".to_string(), expansion: "lazygit".to_string() },
            ShellAbbreviation { keyword: "hx".to_string(), expansion: "helix".to_string() },
        ];

        // Mock hotkeys conflict checking
        let initial_bindings = vec![
            KeybindingRow { chord: "F12".to_string(), action: "toggle_config_tab".to_string(), conflict: false },
            KeybindingRow { chord: "Ctrl+Shift+D".to_string(), action: "split_pane_horizontal".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+F".to_string(), action: "toggle_scrollback_search".to_string(), conflict: false },
            KeybindingRow { chord: "Ctrl+F".to_string(), action: "move_cursor_forward".to_string(), conflict: true }, // Potential collision with shell movements
        ];

        Self {
            active_prompt_preset: "Gruvbox Retro".to_string(),
            enable_git_status: Arc::new(AtomicBool::new(true)),
            enable_directory_context: Arc::new(AtomicBool::new(true)),
            enable_toolchain_versions: Arc::new(AtomicBool::new(false)),
            enable_aws_profile: Arc::new(AtomicBool::new(false)),
            scan_timeout_ms: Arc::new(AtomicU32::new(30)),
            command_timeout_ms: Arc::new(AtomicU32::new(500)),
            enable_transience: Arc::new(AtomicBool::new(true)),

            abbreviations: Arc::new(RwLock::new(initial_abbrevs)),
            new_keyword: String::new(),
            new_expansion: String::new(),
            history_limit: Arc::new(AtomicU32::new(260_000)),
            keybindings: initial_bindings,

            active_shader: "CRT curved curvature".to_string(),
            shader_uniforms: ShaderUniforms {
                scanline_frequency: 0.75,
                screen_curvature: 0.12,
                glow_radius: 1.25,
                blur_intensity: 0.40,
            },
            wgpu_pipeline_valid: true,

            ts_validator: TreeSitterValidator::new(),
            config_input_buffer: "# Manual configuration override overrides here\n[character]\nsuccess_symbol = \"[❯](bold green)\"\nerror_symbol = \"[✖](bold red)\"\n".to_string(),
            ts_ast_valid: true,
            ts_error_message: None,

            active_socket_clients: 2,
            last_agent_id: "Claude-Code-acp-95bf".to_string(),
            seqlock,
        }
    }

    fn render_header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(
                egui::RichText::new("🏛️ THEIA'S PRISM - CONTROL CENTER")
                    .color(egui::Color32::from_rgb(0xFE, 0x80, 0x19)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Hermes Active Sockets: {}", self.active_socket_clients));
                    ui.separator();
                    ui.label(format!("Seqlock Generation: #{}", self.seqlock.get_generation()));
                    ui.separator();
                    ui.label("Legend: system / shell / render");
                });
            });
        });
    }

    fn render_prompt_controls(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("🛰️ ASTRAEA PROMPT COMPILER (Starship Engine)", |ui| {
            ui.horizontal(|ui| {
                ui.label("Format Preset:");
                egui::ComboBox::from_id_source("preset_combo")
                    .selected_text(&self.active_prompt_preset)
                    .show_ui(ui, |ui| {
                        for preset in &["Gruvbox Retro", "Tokyo Night", "Pure Minimal", "Catppuccin Glass"] {
                            if ui.selectable_label(&self.active_prompt_preset == preset, *preset).clicked() {
                                self.active_prompt_preset = preset.to_string();
                            }
                        }
                    });
            });

            ui.add_space(6.0);
            ui.label("Enabled Context Modules:");

            let mut git = self.enable_git_status.load(Ordering::Relaxed);
            if ui.checkbox(&mut git, "Git status tracking (branch, staged, additions)").changed() {
                self.enable_git_status.store(git, Ordering::Relaxed);
            }

            let mut dir = self.enable_directory_context.load(Ordering::Relaxed);
            if ui.checkbox(&mut dir, "Smart truncated directory path (relative to repo root)").changed() {
                self.enable_directory_context.store(dir, Ordering::Relaxed);
            }

            let mut tools = self.enable_toolchain_versions.load(Ordering::Relaxed);
            if ui.checkbox(&mut tools, "Incremental compiler version checks (Node.js, Rust, Python)").changed() {
                self.enable_toolchain_versions.store(tools, Ordering::Relaxed);
            }

            let mut aws = self.enable_aws_profile.load(Ordering::Relaxed);
            if ui.checkbox(&mut aws, "Cloud AWS context alerts").changed() {
                self.enable_aws_profile.store(aws, Ordering::Relaxed);
            }

            ui.add_space(8.0);
            ui.label("Latency Timed Thresholds:");

            let mut scan_val = self.scan_timeout_ms.load(Ordering::Relaxed);
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut scan_val, 10..=250).text("Scan timeout (ms)"));
                if scan_val != self.scan_timeout_ms.load(Ordering::Relaxed) {
                    self.scan_timeout_ms.store(scan_val, Ordering::Relaxed);
                }
            });

            let mut cmd_val = self.command_timeout_ms.load(Ordering::Relaxed);
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut cmd_val, 100..=1000).text("Command timeout (ms)"));
                if cmd_val != self.command_timeout_ms.load(Ordering::Relaxed) {
                    self.command_timeout_ms.store(cmd_val, Ordering::Relaxed);
                }
            });

            let mut transience = self.enable_transience.load(Ordering::Relaxed);
            if ui.checkbox(&mut transience, "Strict shell-level transience (pristine scrollback)").changed() {
                self.enable_transience.store(transience, Ordering::Relaxed);
            }
        });
    }

    fn render_shell_controls(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("🐟 METIS INTERACTIVE SHELL (Fish Logic)", |ui| {
            ui.horizontal(|ui| {
                let mut cap_val = self.history_limit.load(Ordering::Relaxed);
                ui.add(egui::Slider::new(&mut cap_val, 10_000..=500_000).text("History SQLite capacity"));
                if cap_val != self.history_limit.load(Ordering::Relaxed) {
                    self.history_limit.store(cap_val, Ordering::Relaxed);
                }
            });

            ui.add_space(6.0);
            ui.label("Shell abbreviations (direct memory sync):");

            let abbrev_handle = self.abbreviations.clone();
            let mut abbrev_lock = abbrev_handle.write().unwrap();

            egui::Grid::new("abbrev_grid").striped(true).show(ui, |ui| {
                ui.label("Abbrev");
                ui.label("Expansion / command string");
                ui.end_row();

                let mut remove_index = None;
                for (idx, abbrev) in abbrev_lock.iter().enumerate() {
                    ui.label(egui::RichText::new(&abbrev.keyword).color(egui::Color32::from_rgb(0xFE, 0x80, 0x19)));
                    ui.label(&abbrev.expansion);
                    if ui.small_button("❌").clicked() {
                        remove_index = Some(idx);
                    }
                    ui.end_row();
                }

                if let Some(idx) = remove_index {
                    abbrev_lock.remove(idx);
                }
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.new_keyword);
                ui.label("👉");
                ui.text_edit_singleline(&mut self.new_expansion);
                if ui.button("Add Abbrev").clicked() && !self.new_keyword.is_empty() {
                    abbrev_lock.push(ShellAbbreviation {
                        keyword: self.new_keyword.clone(),
                        expansion: self.new_expansion.clone(),
                    });
                    self.new_keyword.clear();
                    self.new_expansion.clear();
                }
            });
        });
    }

    fn render_graphics_controls(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("⚡ ORPHEUS GRAPHICS PIPELINE (Shader Configuration)", |ui| {
            ui.horizontal(|ui| {
                ui.label("WGPU custom shader:");
                egui::ComboBox::from_id_source("shader_combo")
                    .selected_text(&self.active_shader)
                    .show_ui(ui, |ui| {
                        for shader in &["CRT curved curvature", "Neon Text-Glow", "ASCII terminal Matrix", "Classic Raw Retro"] {
                            if ui.selectable_label(&self.active_shader == shader, *shader).clicked() {
                                self.active_shader = shader.to_string();
                            }
                        }
                    });
            });

            ui.add_space(6.0);
            ui.label("Active uniform adjustments (translated via Naga):");
            ui.add(egui::Slider::new(&mut self.shader_uniforms.scanline_frequency, 0.0..=1.5).text("Scanline freq"));
            ui.add(egui::Slider::new(&mut self.shader_uniforms.screen_curvature, 0.0..=0.30).text("Screen curvature"));
            ui.add(egui::Slider::new(&mut self.shader_uniforms.glow_radius, 0.0..=4.0).text("Text-glow radius"));
            ui.add(egui::Slider::new(&mut self.shader_uniforms.blur_intensity, 0.0..=1.0).text("Backdrop opacity"));
        });
    }

    fn render_preview_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("👁️ LIVE PREVIEW");
        ui.add_space(4.0);

        let git_branch_str = if self.enable_git_status.load(Ordering::Relaxed) { "on  main [!] " } else { "" };
        let dir_str = if self.enable_directory_context.load(Ordering::Relaxed) { "~/crates/well-config " } else { "" };
        let duration_str = "took 4ms ";
        let final_char = "❯ ";

        ui.group(|ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("~").color(egui::Color32::from_rgb(0xB8, 0xBB, 0x26)));
                ui.label(egui::RichText::new(dir_str).color(egui::Color32::from_rgb(0xFA, 0xBD, 0x2F)));
                ui.label(egui::RichText::new(git_branch_str).color(egui::Color32::from_rgb(0x8E, 0xC0, 0x7C)));
                ui.label(egui::RichText::new(duration_str).color(egui::Color32::from_rgb(0xDF, 0x3F, 0x3F)));
                ui.label(egui::RichText::new(final_char).color(egui::Color32::from_rgb(0xB8, 0xBB, 0x26)));
            });
            ui.add_space(4.0);
        });
    }

    fn render_hotkeys_panel(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("⌨️ UNIFIED HOTKEY MATRIX", |ui| {
            egui::Grid::new("hotkeys_grid").striped(true).show(ui, |ui| {
                ui.label("Trigger chord");
                ui.label("Action");
                ui.label("Collision check");
                ui.end_row();

                for hotkey in &self.keybindings {
                    ui.label(&hotkey.chord);
                    ui.label(&hotkey.action);
                    if hotkey.conflict {
                        ui.label(egui::RichText::new("⚠️ Shell collision").color(egui::Color32::from_rgb(0xFB, 0x49, 0x34)));
                    } else {
                        ui.label(egui::RichText::new("✓ Valid").color(egui::Color32::from_rgb(0xB8, 0xBB, 0x26)));
                    }
                    ui.end_row();
                }
            });
        });
    }

    fn render_validation_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("✍️ MNEME MANUAL CONFIG OVERRIDES");
        ui.add_space(4.0);

        let text_edit_response = ui.add(
            egui::TextEdit::multiline(&mut self.config_input_buffer)
                .font(egui::TextStyle::Monospace)
                .desired_rows(6)
                .desired_width(f32::INFINITY),
        );

        if text_edit_response.changed() {
            match self.ts_validator.validate_toml_syntax(&self.config_input_buffer) {
                Ok(_) => {
                    self.ts_ast_valid = true;
                    self.ts_error_message = None;
                }
                Err(err) => {
                    self.ts_ast_valid = false;
                    self.ts_error_message = Some(err.to_string());
                }
            }
        }

        if self.ts_ast_valid {
            ui.colored_label(
                egui::Color32::from_rgb(0xB8, 0xBB, 0x26),
                "✓ Tree-Sitter validation: AST syntax trees valid.",
            );
        } else {
            ui.group(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(0xFB, 0x49, 0x34),
                    "✖ Tree-Sitter error: unclosed bracket or malformed TOML structure detected.",
                );
                if let Some(msg) = &self.ts_error_message {
                    ui.small_label(msg);
                }
            });
        }

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.add_enabled(self.ts_ast_valid, egui::Button::new("Apply Config & Update State")).clicked() {
                let new_ver = self.seqlock.write_payload(&self.config_input_buffer);
                println!("[*] Synced new state config payload via Hermes. Sync Generation: {}", new_ver);
            }

            if ui.button("Load Host defaults").clicked() {
                self.config_input_buffer = "# Manual configuration override overrides here\n[character]\nsuccess_symbol = \"[❯](bold green)\"\nerror_symbol = \"[✖](bold red)\"\n".to_string();
                self.ts_ast_valid = true;
                self.ts_error_message = None;
            }
        });
    }

    /// Primary render interface called from Well's egui-WGPU draw thread on F12 tab activation.
    pub fn draw(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ctx.style_mut(|style| {
                style.visuals.window_rounding = 6.0.into();
                style.visuals.override_text_color = Some(egui::Color32::from_rgb(0xEB, 0xDB, 0xB2));
                style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(0x1D, 0x20, 0x21);
                style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(0x28, 0x28, 0x28);
                style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(0x3C, 0x38, 0x36);
                style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(0x50, 0x49, 0x45);
            });

            self.render_header(ui);
            ui.separator();

            ui.columns(2, |columns| {
                self.render_prompt_controls(&mut columns[0]);
                self.render_shell_controls(&mut columns[0]);
                self.render_graphics_controls(&mut columns[0]);

                self.render_preview_panel(&mut columns[1]);
                self.render_hotkeys_panel(&mut columns[1]);
                self.render_validation_panel(&mut columns[1]);
            });
        });
    }
}
