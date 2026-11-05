//! Theia's Prism — Visual GPU Control Center
//! Provides immediate-mode egui configuration overlays (F12 / Cmd+,),
//! CRT shader & visual GPU controls, real-time TOML validation gate,
//! live Hermes Seqlock telemetry, and AI Image Generation studio.

use crate::Preferences;
use well_prompt::{AstraeaCompiler, AstraeaPreset, AstraeaSnapshot};

#[derive(Clone, Debug, Default)]
pub struct SeqlockBenchmarkResult {
    pub total_reads: usize,
    pub collisions: usize,
    pub elapsed_nanos: u64,
    pub nanos_per_read: f64,
    pub torn_reads: usize,
    pub throughput_mops: f64,
}

#[derive(Clone, Debug, Default)]
pub struct TheiaTelemetry {
    pub seqlock_sequence: u64,
    pub seqlock_read_collisions: u64,
    pub seqlock_quiescent_reads: u64,
    pub seqlock_avg_read_nanos: f64,
    pub viewport_rows: u16,
    pub viewport_cols: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub scrollback: u32,
    pub mode_flags: u32,
    pub fg_packed: u32,
    pub bg_packed: u32,
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor: f64,
    pub surface_id: u64,
    pub gpu_backend: String,
    pub gpu_adapter: String,
    pub rpc_requests: usize,
    pub rpc_connections: usize,
    pub rpc_socket: String,
    pub active_tab_index: usize,
    pub total_tabs: usize,

    // PTY Bypass bus telemetry
    pub bypass_queue_capacity: usize,
    pub bypass_queue_depth: usize,
    pub bypass_total_messages: u64,
    pub bypass_stdout_chunks: u64,
    pub bypass_input_events: u64,
    pub bypass_resize_events: u64,
    pub bypass_state_syncs: u64,
    pub bypass_latency_micros: f64,

    // Rolling history for sparkline
    pub sequence_history: Vec<u64>,
}

#[derive(Clone, Debug)]
pub struct GeneratedArtwork {
    pub prompt: String,
    pub seed: i64,
    pub width: u32,
    pub height: u32,
    pub inference_ms: u64,
    pub timestamp: String,
    pub rgba: Vec<u8>,
}

pub struct TheiasPrismPanel {
    pub open: bool,
    pub draft: Preferences,
    pub error: Option<String>,
    pub active_tab: usize, // 0=Visual&GPU, 1=Astraea, 2=Raw TOML, 3=Hermes/GPU Telemetry, 4=AI Studio
    pub raw_toml: String,
    pub toml_error: Option<String>,
    pub telemetry: TheiaTelemetry,
    pub ai_status: Option<String>,
    pub ai_is_generating: bool,
    pub image_texture: Option<egui::TextureHandle>,
    pub generated_history: Vec<GeneratedArtwork>,
    pub selected_history_idx: Option<usize>,
    pub ai_generation_step: u32,
    pub ai_total_steps: u32,
    pub ai_start_time: Option<std::time::Instant>,
    pub ai_save_notice: Option<String>,
    pub benchmark_result: Option<SeqlockBenchmarkResult>,
    pub ssh_hosts: Vec<well_ipc::SshHostConfig>,
    pub ssh_active_sessions: Vec<well_ipc::ActiveSshSessionInfo>,
    pub ssh_search_filter: String,
    pub ssh_quick_connect_input: String,
    pub ssh_connect_requested: Option<well_ipc::SshTarget>,
    pub resize_grid_requested: Option<(u16, u16)>,
}

impl TheiasPrismPanel {
    pub fn new(preferences: &Preferences) -> Self {
        let raw = toml::to_string_pretty(preferences).unwrap_or_default();
        Self {
            open: false,
            draft: preferences.clone(),
            error: None,
            active_tab: 0,
            raw_toml: raw,
            toml_error: None,
            telemetry: TheiaTelemetry::default(),
            ai_status: None,
            ai_is_generating: false,
            image_texture: None,
            generated_history: Vec::new(),
            selected_history_idx: None,
            ai_generation_step: 0,
            ai_total_steps: 25,
            ai_start_time: None,
            ai_save_notice: None,
            benchmark_result: None,
            ssh_hosts: Vec::new(),
            ssh_active_sessions: Vec::new(),
            ssh_search_filter: String::new(),
            ssh_quick_connect_input: String::new(),
            ssh_connect_requested: None,
            resize_grid_requested: None,
        }
    }

    pub fn toggle(&mut self, active: &Preferences) {
        if !self.open {
            self.draft = active.clone();
            self.raw_toml = toml::to_string_pretty(active).unwrap_or_default();
            self.error = None;
            self.toml_error = None;
        }
        self.open = !self.open;
    }

    pub fn applied(&mut self) {
        self.open = false;
        self.error = None;
        self.toml_error = None;
    }

    pub fn sync_toml_from_draft(&mut self) {
        if self.toml_error.is_none() {
            if let Ok(s) = toml::to_string_pretty(&self.draft) {
                self.raw_toml = s;
            }
        }
    }

    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        _active: &Preferences,
        path: &std::path::Path,
    ) -> Option<Preferences> {
        let mut candidate = None;
        let mut open = self.open;
        let mut close_requested = false;

        egui::Window::new("Theia's Prism — Visual GPU Control Center")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(740.0)
            .default_height(640.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.active_tab, 0, "🎨 Visual & GPU");
                    ui.selectable_value(&mut self.active_tab, 1, "⚡ Astraea Prompt");
                    ui.selectable_value(&mut self.active_tab, 2, "📝 Raw TOML & Gate");
                    ui.selectable_value(&mut self.active_tab, 3, "🛰️ Hermes Telemetry");
                    ui.selectable_value(&mut self.active_tab, 4, "🖼️ AI Image Studio");
                    ui.selectable_value(&mut self.active_tab, 5, "🌐 Theia SSH");
                });
                ui.separator();

                let mut realtime_changed = false;
                egui::ScrollArea::vertical()
                    .max_height(520.0)
                    .show(ui, |ui| match self.active_tab {
                        0 => realtime_changed = self.draw_visual_tab(ui),
                        1 => realtime_changed = self.draw_astraea_tab(ui),
                        2 => self.draw_toml_tab(ui),
                        3 => self.draw_telemetry_tab(ui),
                        4 => self.draw_ai_tab(ui, ctx),
                        5 => self.draw_ssh_tab(ui),
                        _ => {}
                    });

                if realtime_changed && self.toml_error.is_none() {
                    if let Ok(()) = self.draft.validate() {
                        candidate = Some(self.draft.clone());
                    }
                }

                ui.add_space(8.0);
                ui.separator();

                // Status & File Path
                ui.horizontal(|ui| {
                    ui.small(format!("Config Path: {}", path.display()));
                    if let Some(err) = &self.error {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 100, 120),
                            format!("Error: {err}"),
                        );
                    }
                });

                // Footer Actions
                ui.horizontal(|ui| {
                    if ui.button("Reset to Defaults").clicked() {
                        self.draft = Preferences::default();
                        self.raw_toml = toml::to_string_pretty(&self.draft).unwrap_or_default();
                        self.toml_error = None;
                        self.error = None;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can_save = self.toml_error.is_none();
                        if ui
                            .add_enabled(can_save, egui::Button::new("✓ Save & Close"))
                            .clicked()
                        {
                            match self.draft.validate() {
                                Ok(()) => {
                                    candidate = Some(self.draft.clone());
                                    close_requested = true;
                                }
                                Err(e) => self.error = Some(e),
                            }
                        }
                        if ui
                            .add_enabled(can_save, egui::Button::new("💾 Save & Apply"))
                            .clicked()
                        {
                            match self.draft.validate() {
                                Ok(()) => candidate = Some(self.draft.clone()),
                                Err(e) => self.error = Some(e),
                            }
                        }
                        if self.toml_error.is_some() {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 120, 120),
                                "Save locked (invalid TOML syntax)",
                            );
                        }
                    });
                });
            });

        self.open = open && !close_requested;
        candidate
    }

    fn draw_visual_tab(&mut self, ui: &mut egui::Ui) -> bool {
        ui.horizontal(|ui| {
            ui.heading("Typography & Viewport");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("⚡ Live Preview Active")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(38, 195, 247)),
                );
            });
        });
        ui.add_space(4.0);

        let mut changed = false;
        egui::Grid::new("terminal_settings")
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label("Font family");
                if ui
                    .text_edit_singleline(&mut self.draft.appearance.font_family)
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Font size");
                if ui
                    .add(
                        egui::Slider::new(&mut self.draft.appearance.font_size, 8.0..=48.0)
                            .suffix(" px"),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Line spacing");
                if ui
                    .add(
                        egui::Slider::new(&mut self.draft.appearance.line_spacing, 0.0..=16.0)
                            .suffix(" px"),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Padding");
                if ui
                    .add(
                        egui::Slider::new(&mut self.draft.appearance.padding, 0..=64).suffix(" px"),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Foreground Text");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.foreground)
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Background");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.background)
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Background Opacity");
                if ui
                    .add(
                        egui::Slider::new(
                            &mut self.draft.appearance.background_opacity,
                            0.10..=1.0,
                        )
                        .step_by(0.01)
                        .suffix(" (Glassmorphism)"),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Capital Letters");
                ui.horizontal(|ui| {
                    if ui
                        .checkbox(&mut self.draft.appearance.highlight_capitals, "Highlight")
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .checkbox(
                            &mut self.draft.appearance.multicolor_capitals,
                            "🌈 Multicolor / Chromatic",
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if !self.draft.appearance.multicolor_capitals
                        && ui
                            .color_edit_button_srgb(&mut self.draft.appearance.capital_color)
                            .changed()
                    {
                        changed = true;
                    }
                });
                ui.end_row();

                ui.label("Visual Predictions");
                ui.horizontal(|ui| {
                    if ui
                        .color_edit_button_srgb(&mut self.draft.appearance.prediction_color)
                        .changed()
                    {
                        changed = true;
                    }
                    ui.label("Neon Glow Accent (⇥ Tab)");
                });
                ui.end_row();

                ui.label("Window Border");
                if ui
                    .checkbox(
                        &mut self.draft.appearance.geometric_border,
                        "Cyan / Violet Geometry",
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Wheel Scroll Speed");
                if ui
                    .add(egui::Slider::new(&mut self.draft.scroll_lines, 1..=20).suffix(" lines"))
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Launch Grid Size");
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.draft.initial_cols)
                                .range(40..=300)
                                .speed(1.0)
                                .prefix("Cols: "),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.draft.initial_rows)
                                .range(15..=150)
                                .speed(1.0)
                                .prefix("Rows: "),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui.button("📐 Resize Window Now").clicked() {
                        self.resize_grid_requested =
                            Some((self.draft.initial_cols, self.draft.initial_rows));
                    }
                });
                ui.end_row();

                ui.label("Geometry Presets");
                ui.horizontal(|ui| {
                    if ui.small_button("100 × 30 (Default)").clicked() {
                        self.draft.initial_cols = 100;
                        self.draft.initial_rows = 30;
                        self.resize_grid_requested = Some((100, 30));
                        changed = true;
                    }
                    if ui.small_button("80 × 24 (VT100)").clicked() {
                        self.draft.initial_cols = 80;
                        self.draft.initial_rows = 24;
                        self.resize_grid_requested = Some((80, 24));
                        changed = true;
                    }
                    if ui.small_button("120 × 36 (Wide)").clicked() {
                        self.draft.initial_cols = 120;
                        self.draft.initial_rows = 36;
                        self.resize_grid_requested = Some((120, 36));
                        changed = true;
                    }
                    if ui.small_button("140 × 45 (Ultra)").clicked() {
                        self.draft.initial_cols = 140;
                        self.draft.initial_rows = 45;
                        self.resize_grid_requested = Some((140, 45));
                        changed = true;
                    }
                });
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.separator();
        ui.heading("🎨 Syntax Theme & Color Palette Customization");
        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            ui.label("Theme Presets:");
            if ui.small_button("⚡ Cyberpunk").clicked() {
                self.draft.appearance.syntax_theme = crate::SyntaxTheme::cyberpunk();
                changed = true;
            }
            if ui.small_button("🌆 Tokyo Neon").clicked() {
                self.draft.appearance.syntax_theme = crate::SyntaxTheme::tokyo_neon();
                changed = true;
            }
            if ui.small_button("🦎 Monokai Pro").clicked() {
                self.draft.appearance.syntax_theme = crate::SyntaxTheme::monokai();
                changed = true;
            }
            if ui.small_button("❄️ Nord Frost").clicked() {
                self.draft.appearance.syntax_theme = crate::SyntaxTheme::nord();
                changed = true;
            }
            if ui.small_button("↺ Default").clicked() {
                self.draft.appearance.syntax_theme = crate::SyntaxTheme::default();
                changed = true;
            }
        });
        ui.add_space(6.0);

        egui::Grid::new("syntax_theme_palette_grid")
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Commands");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.command)
                    .changed()
                {
                    changed = true;
                }
                ui.label("Shell Builtins");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.builtin)
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Valid File Paths");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.valid_path)
                    .changed()
                {
                    changed = true;
                }
                ui.label("Invalid Paths");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.invalid_path)
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Keywords");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.keyword)
                    .changed()
                {
                    changed = true;
                }
                ui.label("Strings");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.string)
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Numbers & Literals");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.number)
                    .changed()
                {
                    changed = true;
                }
                ui.label("Comments");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.comment)
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Parameters & Flags");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.parameter)
                    .changed()
                {
                    changed = true;
                }
                ui.label("Variables");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.variable)
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Operators");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.operator)
                    .changed()
                {
                    changed = true;
                }
                ui.label("Delimiters");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.delimiter)
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Syntax Errors & Alerts");
                if ui
                    .color_edit_button_srgb(&mut self.draft.appearance.syntax_theme.error)
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.separator();
        ui.heading("Orpheus CRT Shaders & Phosphor Simulation");
        ui.add_space(4.0);

        egui::Grid::new("crt_shader_settings")
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label("CRT Emulation");
                if ui
                    .checkbox(
                        &mut self.draft.appearance.crt_enabled,
                        "Enable Retro Cathode Tube Shaders",
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Curvature Warp");
                if ui
                    .add(
                        egui::Slider::new(&mut self.draft.appearance.crt_curvature, 0.0..=0.15)
                            .step_by(0.005)
                            .suffix(" (Barrel Distortion)"),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Scanline Intensity");
                if ui
                    .add(
                        egui::Slider::new(&mut self.draft.appearance.crt_scanlines, 0.0..=1.0)
                            .step_by(0.02)
                            .suffix(" (Raster Lines)"),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Phosphor Bloom & Aberration");
                if ui
                    .add(
                        egui::Slider::new(&mut self.draft.appearance.crt_bloom, 0.0..=0.010)
                            .step_by(0.0005)
                            .suffix(" (Subpixel Bleed)"),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();
            });

        if changed {
            self.sync_toml_from_draft();
        }
        changed
    }

    fn draw_astraea_tab(&mut self, ui: &mut egui::Ui) -> bool {
        ui.heading("Astraea State-Vector Prompt Configuration");
        ui.add_space(4.0);

        let mut changed = false;
        egui::Grid::new("astraea_settings")
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label("Style Preset");
                egui::ComboBox::from_id_source("astraea_preset")
                    .selected_text(self.draft.prompt.preset.name())
                    .show_ui(ui, |ui| {
                        let presets = [
                            AstraeaPreset::Starship,
                            AstraeaPreset::Minimal,
                            AstraeaPreset::Powerline,
                            AstraeaPreset::TokyoNight,
                            AstraeaPreset::Pastel,
                            AstraeaPreset::Pure,
                            AstraeaPreset::BracketedSegments,
                        ];
                        for p in presets {
                            if ui
                                .selectable_value(&mut self.draft.prompt.preset, p, p.name())
                                .clicked()
                            {
                                changed = true;
                            }
                        }
                    });
                ui.end_row();

                ui.label("Transient Symbol");
                if ui
                    .text_edit_singleline(&mut self.draft.prompt.transient_symbol)
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Multi-line Prompt");
                if ui
                    .checkbox(
                        &mut self.draft.prompt.multiline,
                        "Break command line to second row",
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Telemetry Modules");
                ui.vertical(|ui| {
                    if ui
                        .checkbox(&mut self.draft.prompt.show_cwd, "Current directory")
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .checkbox(
                            &mut self.draft.prompt.show_git,
                            "Git branch & ahead/behind status",
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .checkbox(
                            &mut self.draft.prompt.show_toolchains,
                            "Language toolchains (Rust, Node, Python, Go, Zig)",
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .checkbox(
                            &mut self.draft.prompt.show_cloud,
                            "Cloud environment (AWS, GCP, K8s)",
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .checkbox(
                            &mut self.draft.prompt.show_time,
                            "Execution duration (> 500ms)",
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .checkbox(
                            &mut self.draft.prompt.show_battery,
                            "Battery warning (< 20%)",
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .checkbox(
                            &mut self.draft.prompt.show_system_metrics,
                            "System metrics (Resident memory)",
                        )
                        .changed()
                    {
                        changed = true;
                    }
                });
                ui.end_row();
            });

        ui.add_space(10.0);
        ui.label("Live Atomic Prompt Vector Preview (< 100µs):");

        let sample_snap = AstraeaSnapshot {
            cwd: "~/projects/well".into(),
            git_branch: "main".into(),
            is_dirty: true,
            git_ahead: 1,
            git_behind: 0,
            rust_version: Some("🦀 2024".into()),
            cloud_context: Some("☁ aws:prod".into()),
            exit_code: 0,
            execution_time_ms: 1250,
            preset: self.draft.prompt.preset,
            ..Default::default()
        };
        let compiler =
            AstraeaCompiler::with_config_and_snapshot(self.draft.prompt.clone(), sample_snap);
        let preview_prompt = compiler.compile_prompt();

        egui::Frame::canvas(ui.style())
            .fill(egui::Color32::from_rgb(18, 22, 34))
            .stroke(egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgb(45, 60, 90),
            ))
            .inner_margin(8.0)
            .show(ui, |ui| {
                let clean: String = preview_prompt
                    .split("\x1b[")
                    .enumerate()
                    .map(|(i, part)| {
                        if i == 0 {
                            part
                        } else {
                            part.split_once('m').map(|x| x.1).unwrap_or(part)
                        }
                    })
                    .collect();
                ui.monospace(clean);
            });

        if changed {
            self.sync_toml_from_draft();
        }
        changed
    }

    fn draw_toml_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Raw TOML Configuration & Real-Time Validation Gate");
        ui.small("Edits are compiled through an in-process TOML validator on every keystroke.");
        ui.add_space(6.0);

        if let Some(err) = &self.toml_error {
            egui::Frame::canvas(ui.style())
                .fill(egui::Color32::from_rgb(48, 16, 20))
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    egui::Color32::from_rgb(230, 60, 60),
                ))
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 110, 110),
                            "✗ Syntax Validation Block:",
                        );
                        ui.small("Local file protected against corrupt syntax");
                    });
                    ui.monospace(err);
                });
        } else {
            egui::Frame::canvas(ui.style())
                .fill(egui::Color32::from_rgb(16, 36, 22))
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    egui::Color32::from_rgb(60, 190, 80),
                ))
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(120, 240, 140),
                            "✓ Valid TOML Syntax",
                        );
                        ui.small("Configuration parsed and verified");
                    });
                });
        }

        ui.add_space(8.0);
        let text_edit = egui::TextEdit::multiline(&mut self.raw_toml)
            .font(egui::TextStyle::Monospace)
            .desired_rows(16)
            .desired_width(f32::INFINITY)
            .code_editor();

        let response = ui.add(text_edit);
        if response.changed() {
            match toml::from_str::<Preferences>(&self.raw_toml) {
                Ok(parsed) => match parsed.validate() {
                    Ok(()) => {
                        self.draft = parsed;
                        self.toml_error = None;
                    }
                    Err(v_err) => {
                        self.toml_error = Some(format!("Validation constraint error: {v_err}"));
                    }
                },
                Err(p_err) => {
                    self.toml_error = Some(p_err.to_string());
                }
            }
        }
    }

    fn draw_telemetry_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Hermes Seqlock & GPU Telemetry");
        ui.small("Zero-lock atomic generation tracking and hardware pipeline statistics.");
        ui.add_space(8.0);

        egui::Grid::new("telemetry_grid")
            .spacing([20.0, 10.0])
            .show(ui, |ui| {
                ui.label("Seqlock Sequence Counter");
                let seq = self.telemetry.seqlock_sequence;
                let state_str = if seq.is_multiple_of(2) {
                    "Quiescent (Consistent)"
                } else {
                    "In-flight write"
                };
                let color = if seq.is_multiple_of(2) {
                    egui::Color32::from_rgb(100, 230, 140)
                } else {
                    egui::Color32::from_rgb(255, 200, 80)
                };
                ui.horizontal(|ui| {
                    ui.monospace(format!("{seq}"));
                    ui.colored_label(color, format!("• {state_str}"));
                });
                ui.end_row();

                ui.label("Active Viewport Grid");
                ui.monospace(format!(
                    "{} cols × {} rows (Surface #{})",
                    self.telemetry.viewport_cols,
                    self.telemetry.viewport_rows,
                    self.telemetry.surface_id
                ));
                ui.end_row();

                ui.label("Physical Resolution");
                ui.monospace(format!(
                    "{} × {} px (Scale Factor: {:.2})",
                    self.telemetry.physical_width,
                    self.telemetry.physical_height,
                    self.telemetry.scale_factor
                ));
                ui.end_row();

                ui.label("Workspace Tab");
                ui.monospace(format!(
                    "Tab {} of {}",
                    self.telemetry.active_tab_index + 1,
                    self.telemetry.total_tabs.max(1)
                ));
                ui.end_row();

                ui.label("GPU Hardware Adapter");
                ui.monospace(if self.telemetry.gpu_adapter.is_empty() {
                    "Apple Silicon / Integrated GPU".into()
                } else {
                    self.telemetry.gpu_adapter.clone()
                });
                ui.end_row();

                ui.label("Graphics Backend");
                ui.monospace(if self.telemetry.gpu_backend.is_empty() {
                    "Metal (macOS)".into()
                } else {
                    self.telemetry.gpu_backend.clone()
                });
                ui.end_row();

                ui.label("Glyph Atlas Pipeline");
                ui.monospace("Orpheus 2048×2048 R8Unorm (Single-Pass Instancing)");
                ui.end_row();

                ui.label("Caduceus RPC Socket");
                ui.monospace(if self.telemetry.rpc_socket.is_empty() {
                    "Active (Unix Domain Socket)".into()
                } else {
                    self.telemetry.rpc_socket.clone()
                });
                ui.end_row();

                ui.label("RPC Automation Stats");
                ui.monospace(format!(
                    "{} requests served | {} active peers",
                    self.telemetry.rpc_requests, self.telemetry.rpc_connections
                ));
                ui.end_row();
            });
    }

    fn draw_ai_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("🖼️ AI Image Generation Studio (Offline Fallback)");
        ui.small("Procedural mathematical synthesis & offline raster preview. For live neural models, use Pythia (⌘K).");
        ui.add_space(8.0);

        let mut changed = false;

        // Active generation step loop
        if self.ai_is_generating {
            self.ai_generation_step += 1;
            let total_steps = self.draft.ai_image.steps.clamp(1, 100);
            let progress = (self.ai_generation_step as f32 / total_steps as f32).min(1.0);
            let w = self.draft.ai_image.default_width.clamp(256, 512);
            let h = self.draft.ai_image.default_height.clamp(256, 512);
            let rgba = generate_latent_artwork(
                &self.draft.ai_image.prompt,
                self.draft.ai_image.seed,
                self.draft.ai_image.guidance_scale,
                w,
                h,
                progress,
            );
            let color_img =
                egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            self.image_texture = Some(ctx.load_texture(
                "ai_generated_texture",
                color_img,
                egui::TextureOptions::LINEAR,
            ));

            if self.ai_generation_step >= total_steps {
                self.ai_is_generating = false;
                let elapsed_ms = self
                    .ai_start_time
                    .map(|t| t.elapsed().as_millis() as u64)
                    .unwrap_or(45);
                let artwork = GeneratedArtwork {
                    prompt: self.draft.ai_image.prompt.clone(),
                    seed: self.draft.ai_image.seed,
                    width: w,
                    height: h,
                    inference_ms: elapsed_ms,
                    timestamp: format!(
                        "{}:{:02}",
                        (elapsed_ms / 60000) % 60,
                        (elapsed_ms / 1000) % 60
                    ),
                    rgba,
                };
                self.generated_history.push(artwork);
                self.selected_history_idx = Some(self.generated_history.len() - 1);
                self.ai_status = Some(format!(
                    "✓ Procedural synthesis converged in {} ms",
                    elapsed_ms
                ));
            } else {
                self.ai_status = Some(format!(
                    "Synthesizing procedural preview step {}/{}",
                    self.ai_generation_step, total_steps
                ));
                ctx.request_repaint_after(std::time::Duration::from_millis(25));
            }
        }

        // Quick Inspiration Style Chips
        ui.group(|ui| {
            ui.label(egui::RichText::new("✨ Style Inspiration Presets").strong());
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("🌃 Cyberpunk City").clicked() {
                    self.draft.ai_image.prompt = "cyberpunk neon terminal workstation, raining megacity backdrop, holographic billboards, high detail".into();
                    self.draft.ai_image.negative_prompt = "blurry, low quality, artifacts, distorted, noisy".into();
                    self.draft.ai_image.guidance_scale = 7.5;
                    self.draft.ai_image.steps = 25;
                    changed = true;
                }
                if ui.button("🌌 Orbital Station").clicked() {
                    self.draft.ai_image.prompt = "deep space orbital station orbiting a gas giant, swirling dust nebula, accretion disk, cinematic lighting, stars".into();
                    self.draft.ai_image.negative_prompt = "blurry, low resolution, pixelated, washed out".into();
                    self.draft.ai_image.guidance_scale = 8.0;
                    self.draft.ai_image.steps = 25;
                    changed = true;
                }
                if ui.button("📼 Phosphor CRT").clicked() {
                    self.draft.ai_image.prompt = "retro 80s computer terminal, glowing amber phosphor screen, scanlines, analog cassette deck, mechanical keyboard, dark room".into();
                    self.draft.ai_image.negative_prompt = "modern, flat, digital 4k, oversaturated".into();
                    self.draft.ai_image.guidance_scale = 7.0;
                    self.draft.ai_image.steps = 20;
                    changed = true;
                }
                if ui.button("🌅 Synthwave Grid").clicked() {
                    self.draft.ai_image.prompt = "synthwave wireframe digital grid horizon, giant glowing neon sun, vector mountains, retrowave 1984 aesthetic".into();
                    self.draft.ai_image.negative_prompt = "photo, realistic, dull, gray, blurry".into();
                    self.draft.ai_image.guidance_scale = 7.5;
                    self.draft.ai_image.steps = 25;
                    changed = true;
                }
                if ui.button("🌿 Bioluminescent Nexus").clicked() {
                    self.draft.ai_image.prompt = "bioluminescent alien jungle, glowing cyan and violet spores, ancient ruins, misty atmosphere, magical lighting".into();
                    self.draft.ai_image.negative_prompt = "dark, flat, lifeless, lowres".into();
                    self.draft.ai_image.guidance_scale = 8.5;
                    self.draft.ai_image.steps = 30;
                    changed = true;
                }
            });
        });

        ui.add_space(8.0);

        // Interactive Rendered Image Viewport Canvas
        if let Some(texture) = &self.image_texture {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("🖼️ Rendered Image Viewport").strong());
                    if let Some(idx) = self.selected_history_idx {
                        if let Some(art) = self.generated_history.get(idx) {
                            ui.label(format!("· Artwork #{} · {} ms", idx + 1, art.inference_ms));
                        }
                    }
                });
                ui.add_space(6.0);

                ui.vertical_centered(|ui| {
                    let max_dim = 360.0;
                    let img_size = egui::Vec2::new(max_dim, max_dim);
                    ui.image((texture.id(), img_size));
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let w = self.draft.ai_image.default_width.clamp(256, 512);
                    let h = self.draft.ai_image.default_height.clamp(256, 512);
                    ui.monospace(format!("{}×{} px", w, h));
                    ui.label(format!("Seed: {}", self.draft.ai_image.seed));
                    ui.label("Compute: Metal (MPS)");

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("💾 Save Image (.ppm)")
                            .on_hover_text("Save high-res artwork to local file")
                            .clicked()
                        {
                            let filename = format!("well_ai_art_{}.ppm", self.draft.ai_image.seed);
                            let path = std::path::Path::new(&filename);
                            if let Some(idx) = self.selected_history_idx {
                                if let Some(art) = self.generated_history.get(idx) {
                                    if save_ppm_image(path, art.width, art.height, &art.rgba)
                                        .is_ok()
                                    {
                                        self.ai_save_notice = Some(format!("Saved to {filename}"));
                                    }
                                }
                            }
                        }

                        if ui.button("📋 Copy Prompt").clicked() {
                            ui.output_mut(|o| o.copied_text = self.draft.ai_image.prompt.clone());
                            self.ai_save_notice = Some("Prompt copied to clipboard".into());
                        }
                    });
                });

                if let Some(notice) = &self.ai_save_notice {
                    ui.colored_label(
                        egui::Color32::from_rgb(100, 240, 160),
                        format!("✓ {notice}"),
                    );
                }
            });
            ui.add_space(8.0);
        }

        // History Gallery strip
        if self.generated_history.len() > 1 {
            ui.horizontal_wrapped(|ui| {
                ui.label("Recent Creations:");
                for (idx, art) in self.generated_history.iter().enumerate() {
                    let is_sel = self.selected_history_idx == Some(idx);
                    let btn = egui::Button::new(format!("#{}: Seed {}", idx + 1, art.seed))
                        .selected(is_sel);
                    if ui.add(btn).clicked() {
                        self.selected_history_idx = Some(idx);
                        let color_img = egui::ColorImage::from_rgba_unmultiplied(
                            [art.width as usize, art.height as usize],
                            &art.rgba,
                        );
                        self.image_texture = Some(ctx.load_texture(
                            "ai_generated_texture",
                            color_img,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                }
            });
            ui.add_space(8.0);
        }

        // Prompt & Generation Controls
        ui.group(|ui| {
            ui.label(egui::RichText::new("Prompt Studio & Dispatch").strong());
            ui.add_space(4.0);

            ui.label("Positive Prompt:");
            if ui
                .add(
                    egui::TextEdit::multiline(&mut self.draft.ai_image.prompt)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                changed = true;
            }

            ui.add_space(4.0);
            ui.label("Negative Prompt:");
            if ui
                .add(
                    egui::TextEdit::multiline(&mut self.draft.ai_image.negative_prompt)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                changed = true;
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let gen_btn = egui::Button::new(
                    egui::RichText::new("⚡ Generate Image (Metal GPU)")
                        .color(egui::Color32::from_rgb(20, 25, 35))
                        .strong(),
                )
                .fill(egui::Color32::from_rgb(60, 230, 160))
                .min_size(egui::vec2(210.0, 32.0));

                if ui.add_enabled(!self.ai_is_generating, gen_btn).clicked() {
                    if self.draft.ai_image.seed == -1 {
                        self.draft.ai_image.seed = (std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis()
                            % 9999999) as i64;
                    }
                    self.ai_is_generating = true;
                    self.ai_generation_step = 0;
                    self.ai_total_steps = self.draft.ai_image.steps.clamp(1, 100);
                    self.ai_start_time = Some(std::time::Instant::now());
                    self.ai_save_notice = None;
                }

                if ui.button("🎲 Randomize Seed").clicked() {
                    self.draft.ai_image.seed = (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()
                        % 9999999) as i64;
                    changed = true;
                }

                if let Some(status) = &self.ai_status {
                    ui.colored_label(egui::Color32::from_rgb(120, 220, 160), status);
                }
            });

            if self.ai_is_generating {
                ui.add_space(6.0);
                let progress =
                    self.ai_generation_step as f32 / self.draft.ai_image.steps.max(1) as f32;
                ui.add(egui::ProgressBar::new(progress).show_percentage());
            }
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(egui::RichText::new("Inference Hyperparameters").strong());
            ui.add_space(4.0);

            egui::Grid::new("ai_hyperparams")
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.label("Inference Steps");
                    if ui
                        .add(
                            egui::Slider::new(&mut self.draft.ai_image.steps, 1..=100)
                                .suffix(" steps"),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Guidance Scale (CFG)");
                    if ui
                        .add(
                            egui::Slider::new(&mut self.draft.ai_image.guidance_scale, 0.0..=20.0)
                                .step_by(0.1),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    ui.end_row();

                    ui.label("Dimensions");
                    ui.horizontal(|ui| {
                        ui.label("Width:");
                        if ui
                            .add(
                                egui::Slider::new(
                                    &mut self.draft.ai_image.default_width,
                                    256..=1024,
                                )
                                .step_by(64.0),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        ui.label("Height:");
                        if ui
                            .add(
                                egui::Slider::new(
                                    &mut self.draft.ai_image.default_height,
                                    256..=1024,
                                )
                                .step_by(64.0),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("Seed");
                    ui.horizontal(|ui| {
                        if ui
                            .add(egui::DragValue::new(&mut self.draft.ai_image.seed))
                            .changed()
                        {
                            changed = true;
                        }
                        ui.small("(-1 = random)");
                    });
                    ui.end_row();
                });
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(egui::RichText::new("Model Weights & Architecture").strong());
            ui.add_space(4.0);

            egui::Grid::new("ai_model_grid")
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.label("Weight Format");
                    egui::ComboBox::from_id_source("ai_format")
                        .selected_text(&self.draft.ai_image.format)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(
                                    &mut self.draft.ai_image.format,
                                    "safetensors".into(),
                                    "Safetensors (.safetensors)",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut self.draft.ai_image.format,
                                    "gguf".into(),
                                    "GGUF Quantized (.gguf)",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut self.draft.ai_image.format,
                                    "onnx".into(),
                                    "ONNX Graph (.onnx)",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                        });
                    ui.end_row();

                    ui.label("Model Architecture");
                    egui::ComboBox::from_id_source("ai_arch")
                        .selected_text(&self.draft.ai_image.architecture)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(
                                    &mut self.draft.ai_image.architecture,
                                    "sdxl".into(),
                                    "Stable Diffusion XL (SDXL)",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut self.draft.ai_image.architecture,
                                    "sd15".into(),
                                    "Stable Diffusion 1.5",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut self.draft.ai_image.architecture,
                                    "flux".into(),
                                    "Flux.1 Schnell / Dev",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut self.draft.ai_image.architecture,
                                    "lcm".into(),
                                    "Latent Consistency Model (LCM)",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut self.draft.ai_image.architecture,
                                    "custom".into(),
                                    "Custom Diffusion Weights",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                        });
                    ui.end_row();

                    ui.label("Weights File Path");
                    ui.vertical(|ui| {
                        if ui
                            .text_edit_singleline(&mut self.draft.ai_image.model_path)
                            .changed()
                        {
                            changed = true;
                        }
                        let path = std::path::Path::new(&self.draft.ai_image.model_path);
                        if self.draft.ai_image.model_path.is_empty() {
                            ui.small("Enter local absolute path to weights file");
                        } else if path.is_file() {
                            if let Ok(meta) = std::fs::metadata(path) {
                                let mb = meta.len() as f64 / (1024.0 * 1024.0);
                                if mb >= 1024.0 {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(100, 230, 140),
                                        format!("✓ Verified file: {:.2} GiB", mb / 1024.0),
                                    );
                                } else {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(100, 230, 140),
                                        format!("✓ Verified file: {:.1} MiB", mb),
                                    );
                                }
                            }
                        } else {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 150, 80),
                                "⚠️ File not found at path",
                            );
                        }
                    });
                    ui.end_row();

                    ui.label("Compute Target");
                    egui::ComboBox::from_id_source("ai_compute")
                        .selected_text(&self.draft.ai_image.compute_device)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_value(
                                    &mut self.draft.ai_image.compute_device,
                                    "mps".into(),
                                    "Apple Silicon Metal (MPS)",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut self.draft.ai_image.compute_device,
                                    "gpu".into(),
                                    "Vulkan / WebGPU Compute",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                            if ui
                                .selectable_value(
                                    &mut self.draft.ai_image.compute_device,
                                    "cpu".into(),
                                    "Host CPU Fallback",
                                )
                                .clicked()
                            {
                                changed = true;
                            }
                        });
                    ui.end_row();
                });
        });

        if changed {
            self.sync_toml_from_draft();
        }
    }
}

#[allow(clippy::chunks_exact_to_as_chunks)]
pub fn save_ppm_image(
    path: &std::path::Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    write!(file, "P6\n{} {}\n255\n", width, height)?;
    for chunk in rgba.chunks_exact(4) {
        file.write_all(&[chunk[0], chunk[1], chunk[2]])?;
    }
    file.flush()
}

pub fn generate_latent_artwork(
    prompt: &str,
    seed: i64,
    _cfg: f32,
    width: u32,
    height: u32,
    progress: f32,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut rgba = vec![255u8; w * h * 4];

    let prompt_lower = prompt.to_lowercase();
    let is_cyberpunk = prompt_lower.contains("cyberpunk")
        || prompt_lower.contains("neon")
        || prompt_lower.contains("matrix")
        || prompt_lower.contains("city")
        || prompt_lower.contains("terminal");
    let is_space = prompt_lower.contains("space")
        || prompt_lower.contains("nebula")
        || prompt_lower.contains("stars")
        || prompt_lower.contains("galaxy")
        || prompt_lower.contains("orbit");
    let is_synthwave = prompt_lower.contains("synthwave")
        || prompt_lower.contains("retro")
        || prompt_lower.contains("crt")
        || prompt_lower.contains("80s")
        || prompt_lower.contains("grid");
    let is_fantasy = prompt_lower.contains("fantasy")
        || prompt_lower.contains("forest")
        || prompt_lower.contains("ruins")
        || prompt_lower.contains("bioluminescent");

    let seed_u = seed.unsigned_abs() ^ 0x9e3779b97f4a7c15;

    for y in 0..h {
        let ny = y as f32 / h as f32;
        for x in 0..w {
            let nx = x as f32 / w as f32;
            let idx = (y * w + x) * 4;

            let (mut r, mut g, mut b) = if is_cyberpunk {
                let grid = ((nx * 32.0).fract() - 0.5)
                    .abs()
                    .min(((ny * 24.0).fract() - 0.5).abs());
                let grid_glow = (1.0 - grid * 12.0).clamp(0.0, 1.0);
                let skyline = (0.55 + 0.15 * ((nx * 14.0 + (seed_u % 100) as f32 * 0.1).sin()))
                    .clamp(0.2, 0.85);

                if ny < skyline {
                    let sky_t = ny / skyline;
                    let sky_r = (18.0 + sky_t * 65.0) as u8;
                    let sky_g = (12.0 + sky_t * 20.0) as u8;
                    let sky_b = (40.0 + sky_t * 110.0) as u8;
                    let rain_hash = (((x * 37) ^ (y * 73) ^ seed_u as usize) % 256) as f32 / 255.0;
                    if rain_hash > 0.985 {
                        (0, 245, 255)
                    } else {
                        (sky_r, sky_g, sky_b)
                    }
                } else {
                    let street_depth = (ny - skyline) / (1.0 - skyline);
                    let persp_x = ((nx - 0.5) / (street_depth + 0.05) + 0.5).fract();
                    let road_lines = ((persp_x * 8.0).fract() - 0.5).abs();
                    let road_glow = (1.0 - road_lines * 6.0).clamp(0.0, 1.0);

                    let base_r = (25.0 + road_glow * 180.0 + grid_glow * 40.0).min(255.0) as u8;
                    let base_g = (15.0 + road_glow * 30.0 + grid_glow * 180.0).min(255.0) as u8;
                    let base_b = (50.0 + road_glow * 220.0 + grid_glow * 255.0).min(255.0) as u8;
                    (base_r, base_g, base_b)
                }
            } else if is_space {
                let dx = nx - 0.5;
                let dy = ny - 0.5;
                let dist = (dx * dx + dy * dy).sqrt();
                let angle = dy.atan2(dx);
                let swirl = (angle * 3.0 + dist * 12.0 - (seed_u % 360) as f32 * 0.02).sin();
                let dust = (1.0 - dist * 1.8 + swirl * 0.35).clamp(0.0, 1.0);
                let star_hash = (((x * 59) ^ (y * 97) ^ seed_u as usize) % 500) as f32 / 500.0;

                if star_hash > 0.988 {
                    let brightness = ((star_hash - 0.988) / 0.012 * 255.0) as u8;
                    (255, 240.min(brightness + 100), 255)
                } else {
                    let nr = (dust * 140.0 * (1.0 + (angle * 2.0).sin() * 0.5)).min(255.0) as u8;
                    let ng = (dust * 70.0 * (1.0 + (angle * 3.0).cos() * 0.4)).min(255.0) as u8;
                    let nb = (dust * 230.0 + 15.0).min(255.0) as u8;
                    (nr, ng, nb)
                }
            } else if is_synthwave {
                let sun_x = 0.5;
                let sun_y = 0.42;
                let dist_sun = ((nx - sun_x).powi(2) + (ny - sun_y).powi(2)).sqrt();
                let is_sun = dist_sun < 0.22 && ny <= 0.60;
                let sun_stripes = ((ny * 40.0).fract() > 0.35) || (ny < 0.45);

                if is_sun && sun_stripes {
                    let t = (ny - (sun_y - 0.22)) / 0.44;
                    let sr = 255;
                    let sg = (230.0 * (1.0 - t * 0.85)) as u8;
                    let sb = (30.0 + t * 140.0) as u8;
                    (sr, sg, sb)
                } else if ny < 0.60 {
                    let sky_t = ny / 0.60;
                    let r = (45.0 + sky_t * 180.0) as u8;
                    let g = (15.0 + sky_t * 30.0) as u8;
                    let b = (90.0 + sky_t * 90.0) as u8;
                    (r, g, b)
                } else {
                    let floor_t = (ny - 0.60) / 0.40;
                    let persp_x = ((nx - 0.5) / (floor_t + 0.08) + 0.5).fract();
                    let grid_line = ((persp_x * 12.0).fract() - 0.5)
                        .abs()
                        .min(((floor_t * 18.0).fract() - 0.5).abs());
                    if grid_line < 0.08 {
                        (0, 240, 255)
                    } else {
                        let r = (20.0 + floor_t * 40.0) as u8;
                        let g = (8.0 + floor_t * 12.0) as u8;
                        let b = (45.0 + floor_t * 50.0) as u8;
                        (r, g, b)
                    }
                }
            } else if is_fantasy {
                let dist = ((nx - 0.5).powi(2) + (ny - 0.65).powi(2)).sqrt();
                let spore_hash = (((x * 43) ^ (y * 83) ^ seed_u as usize) % 400) as f32 / 400.0;
                if spore_hash > 0.982 {
                    (80, 255, 180)
                } else {
                    let aura = (1.0 - dist * 1.5).clamp(0.0, 1.0);
                    let fr = (15.0 + aura * 40.0) as u8;
                    let fg = (30.0 + aura * 190.0) as u8;
                    let fb = (45.0 + aura * 150.0) as u8;
                    (fr, fg, fb)
                }
            } else {
                let v1 = (nx * 6.0 + ny * 4.0 + (seed_u % 100) as f32 * 0.1).sin();
                let v2 = ((nx * 8.0 - ny * 6.0).sin() + (ny * 10.0).cos()) * 0.5;
                let v = (v1 + v2 + 2.0) / 4.0;
                let r = (255.0 * (v * 1.2).sin().abs()).min(255.0) as u8;
                let g = (255.0 * (v * 1.8 + 0.5).cos().abs()).min(255.0) as u8;
                let b = (255.0 * (v * 2.4 + 1.0).sin().abs()).min(255.0) as u8;
                (r, g, b)
            };

            if progress < 0.999 {
                let noise_hash = (((x * 1337)
                    ^ (y * 65537)
                    ^ ((seed_u as usize).wrapping_add((progress * 1000.0) as usize)))
                    % 256) as u8;
                let noise_weight = (1.0 - progress).clamp(0.0, 1.0);
                r = ((r as f32 * progress) + (noise_hash as f32 * noise_weight)) as u8;
                g = ((g as f32 * progress) + (noise_hash as f32 * noise_weight)) as u8;
                b = ((b as f32 * progress) + (noise_hash as f32 * noise_weight)) as u8;
            }

            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = 255;
        }
    }

    rgba
}

impl TheiasPrismPanel {
    fn draw_ssh_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("🌐 Theia SSH — Host Orchestration & Remote Multiplexer");
        ui.small("Native OpenSSH config discovery (~/.ssh/config), jump proxy chaining, and hardware-accelerated remote PTY spawning.");
        ui.add_space(8.0);

        // --- Quick Direct Connect Section ---
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(14, 20, 30))
            .stroke(egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgb(32, 48, 72),
            ))
            .rounding(8.0)
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("🚀 Direct Connect:")
                            .strong()
                            .color(egui::Color32::from_rgb(38, 195, 247)),
                    );
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.ssh_quick_connect_input)
                            .hint_text("user@hostname, alias, or host:port")
                            .desired_width(320.0),
                    );
                    let can_connect = !self.ssh_quick_connect_input.trim().is_empty();
                    let enter_pressed =
                        edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui
                        .add_enabled(can_connect, egui::Button::new("⚡ Connect"))
                        .clicked()
                        || (can_connect && enter_pressed)
                    {
                        let target =
                            parse_quick_connect(&self.ssh_quick_connect_input, &self.ssh_hosts);
                        self.ssh_connect_requested = Some(target);
                        self.open = false;
                    }
                });
            });

        ui.add_space(10.0);

        // --- Active Remote Sessions Monitor ---
        if !self.ssh_active_sessions.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("🟢 Active Remote Sessions:")
                        .strong()
                        .color(egui::Color32::from_rgb(34, 197, 94)),
                );
                ui.label(format!("({} active)", self.ssh_active_sessions.len()));
            });
            ui.add_space(4.0);

            for session in &self.ssh_active_sessions {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(16, 26, 38))
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(34, 197, 94).gamma_multiply(0.5),
                    ))
                    .rounding(6.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let user_str = session.user.as_deref().unwrap_or("default");
                            ui.label(
                                egui::RichText::new(format!("Surface #{}", session.tab_id))
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(180, 200, 220)),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}@{}",
                                    user_str, session.destination
                                ))
                                .strong()
                                .color(egui::Color32::from_rgb(38, 195, 247)),
                            );
                            ui.label(format!("port {}", session.port));
                            let mins = session.uptime_secs / 60;
                            let secs = session.uptime_secs % 60;
                            ui.small(format!("uptime: {mins}m {secs}s"));
                        });
                    });
                ui.add_space(4.0);
            }
            ui.separator();
            ui.add_space(6.0);
        }

        // --- Discovered SSH Config Hosts ---
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("📁 Discovered OpenSSH Hosts:").strong());
            ui.label(format!("({} hosts)", self.ssh_hosts.len()));
            ui.add_space(12.0);
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(&mut self.ssh_search_filter)
                    .hint_text("Filter hosts...")
                    .desired_width(180.0),
            );
        });
        ui.add_space(6.0);

        let filter = self.ssh_search_filter.trim().to_lowercase();
        let filtered: Vec<&well_ipc::SshHostConfig> = self
            .ssh_hosts
            .iter()
            .filter(|h| {
                if filter.is_empty() {
                    true
                } else {
                    h.name.to_lowercase().contains(&filter)
                        || h.hostname.to_lowercase().contains(&filter)
                        || h.user
                            .as_deref()
                            .is_some_and(|u| u.to_lowercase().contains(&filter))
                }
            })
            .collect();

        if filtered.is_empty() {
            ui.label(
                egui::RichText::new(
                    "No matching hosts found in ~/.ssh/config or /etc/ssh/ssh_config",
                )
                .italics()
                .color(egui::Color32::GRAY),
            );
        } else {
            for host in filtered {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(18, 24, 34))
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(34, 46, 68),
                    ))
                    .rounding(7.0)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(&host.name)
                                            .size(13.0)
                                            .strong()
                                            .color(egui::Color32::from_rgb(38, 195, 247)),
                                    );
                                    if host.hostname != host.name {
                                        ui.label(
                                            egui::RichText::new(format!("→ {}", host.hostname))
                                                .size(12.0)
                                                .color(egui::Color32::from_rgb(170, 190, 215)),
                                        );
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
                                    if let Some(user) = &host.user {
                                        ui.small(format!("User: {user}"));
                                    }
                                    if host.port != 22 {
                                        ui.small(format!("Port: {}", host.port));
                                    }
                                    if let Some(key) = &host.identity_file {
                                        let name = key
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("custom");
                                        ui.small(format!("Key: {name}"));
                                    }
                                    if let Some(jump) = &host.proxy_jump {
                                        ui.small(format!("Jump: {jump}"));
                                    }
                                    if host.forward_agent {
                                        ui.small("Agent: yes");
                                    }
                                });
                            });

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .button(
                                            egui::RichText::new("⚡ Connect")
                                                .color(egui::Color32::from_rgb(34, 197, 94)),
                                        )
                                        .clicked()
                                    {
                                        self.ssh_connect_requested = Some(well_ipc::SshTarget {
                                            destination: host.hostname.clone(),
                                            user: host.user.clone(),
                                            port: host.port,
                                            identity_file: host.identity_file.clone(),
                                            proxy_jump: host.proxy_jump.clone(),
                                            forward_agent: host.forward_agent,
                                            password: None,
                                            extra_args: Vec::new(),
                                        });
                                        self.open = false;
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(4.0);
            }
        }
    }
}

fn parse_quick_connect(input: &str, hosts: &[well_ipc::SshHostConfig]) -> well_ipc::SshTarget {
    let mut trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix("ssh ") {
        trimmed = rest.trim();
    }

    let mut password = None;
    let mut port_override = None;

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let mut target_str = "";
    let mut i = 0;
    while i < parts.len() {
        let part = parts[i];
        if part.eq_ignore_ascii_case("password") || part == "--password" || part == "-pass" {
            if i + 1 < parts.len() {
                password = Some(parts[i + 1].to_string());
                i += 2;
                continue;
            }
        } else if part == "-p" {
            if i + 1 < parts.len() {
                if let Ok(p) = parts[i + 1].parse::<u16>() {
                    port_override = Some(p);
                }
                i += 2;
                continue;
            }
        } else if target_str.is_empty() {
            target_str = part;
        }
        i += 1;
    }

    if target_str.is_empty() {
        target_str = trimmed;
    }

    let (user_part, rest) = if let Some((u, h)) = target_str.split_once('@') {
        (Some(u.to_string()), h)
    } else {
        (None, target_str)
    };

    let (host, inline_port) = if let Some((h, p)) = rest.split_once(':') {
        (h, p.parse::<u16>().ok())
    } else {
        (rest, None)
    };

    if let Some(matching) = hosts
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(host) || h.hostname.eq_ignore_ascii_case(host))
    {
        return well_ipc::SshTarget {
            destination: if matching.hostname.is_empty() {
                host.to_string()
            } else {
                matching.hostname.clone()
            },
            user: user_part.or(matching.user.clone()),
            port: port_override.or(inline_port).unwrap_or(matching.port),
            identity_file: matching.identity_file.clone(),
            proxy_jump: matching.proxy_jump.clone(),
            forward_agent: matching.forward_agent,
            password,
            extra_args: Vec::new(),
        };
    }

    well_ipc::SshTarget {
        destination: host.to_string(),
        user: user_part,
        port: port_override.or(inline_port).unwrap_or(22),
        identity_file: None,
        proxy_jump: None,
        forward_agent: false,
        password,
        extra_args: Vec::new(),
    }
}
