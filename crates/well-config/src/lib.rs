//! well-config: Theia's Prism Immediate-Mode Configuration Dashboard
//!
//! Subsystems:
//! - TheiasPrismPanel: Immediate-mode GUI built on egui for zero-overhead,
//!   GPU-accelerated settings layout synchronized natively over Hermes Seqlock.

use std::sync::Arc;
use egui::{Color32, Context, RichText, ScrollArea, Slider, Ui};
use well_ipc::{HermesChannel, TheiaConfigPayload};

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

#[derive(Clone)]
pub struct ShaderUniforms {
    pub scanline_frequency: f32,
    pub screen_curvature: f32,
    pub glow_radius: f32,
    pub blur_intensity: f32,
}

impl Default for ShaderUniforms {
    fn default() -> Self {
        Self {
            scanline_frequency: 0.75,
            screen_curvature: 0.12,
            glow_radius: 1.25,
            blur_intensity: 0.40,
        }
    }
}

pub struct TheiasPrismPanel {
    pub channel: Arc<HermesChannel<TheiaConfigPayload>>,
    pub config: TheiaConfigPayload,
    pub abbreviations: Vec<ShellAbbreviation>,
    pub new_keyword: String,
    pub new_expansion: String,
    pub keybindings: Vec<KeybindingRow>,
    pub active_shader: String,
    pub shader_uniforms: ShaderUniforms,
    pub active_tab: usize,
}

impl TheiasPrismPanel {
    pub fn new(channel: Arc<HermesChannel<TheiaConfigPayload>>) -> Self {
        let initial_config = channel.read_state();
        let initial_abbrevs = vec![
            ShellAbbreviation { keyword: "gco".to_string(), expansion: "git checkout".to_string() },
            ShellAbbreviation { keyword: "lg".to_string(), expansion: "lazygit".to_string() },
            ShellAbbreviation { keyword: "hx".to_string(), expansion: "helix".to_string() },
        ];

        let initial_bindings = vec![
            KeybindingRow { chord: "F12".to_string(), action: "toggle_config_tab".to_string(), conflict: false },
            KeybindingRow { chord: "Ctrl+Shift+D".to_string(), action: "split_pane_horizontal".to_string(), conflict: false },
            KeybindingRow { chord: "Cmd+F".to_string(), action: "toggle_scrollback_search".to_string(), conflict: false },
        ];

        Self {
            channel,
            config: initial_config,
            abbreviations: initial_abbrevs,
            new_keyword: String::new(),
            new_expansion: String::new(),
            keybindings: initial_bindings,
            active_shader: "CRT curved curvature".to_string(),
            shader_uniforms: ShaderUniforms::default(),
            active_tab: 0,
        }
    }

    pub fn render(&mut self, ctx: &Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(RichText::new("🏛️ Theia's Prism — Control Center").color(Color32::from_rgb(0, 240, 255)).strong());
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.selectable_label(self.active_tab == 0, "Astraea (Prompt)").clicked() { self.active_tab = 0; }
                if ui.selectable_label(self.active_tab == 1, "Metis (Shell)").clicked() { self.active_tab = 1; }
                if ui.selectable_label(self.active_tab == 2, "Orpheus (Shaders)").clicked() { self.active_tab = 2; }
                if ui.selectable_label(self.active_tab == 3, "Hermes (IPC)").clicked() { self.active_tab = 3; }
            });
            ui.separator();

            ScrollArea::vertical().show(ui, |ui| {
                match self.active_tab {
                    0 => self.render_astraea_tab(ui),
                    1 => self.render_metis_tab(ui),
                    2 => self.render_orpheus_tab(ui),
                    _ => self.render_hermes_tab(ui),
                }
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(RichText::new("⚡ Apply & Sync to Hermes").color(Color32::BLACK)).clicked() {
                    self.channel.sync_state(self.config);
                }
                ui.label(RichText::new("Lock-free atomic Seqlock active").weak());
            });
        });
    }

    fn render_astraea_tab(&mut self, ui: &mut Ui) {
        ui.heading("Astraea Prompt Configuration");
        ui.checkbox(&mut self.config.enable_transient_prompt, "Enable Transient Prompt (❯ on return)");
        ui.checkbox(&mut self.config.enable_kitty_keyboard, "Enable Kitty Keyboard Protocol Bitmasks");
        ui.add(Slider::new(&mut self.config.scan_timeout_ms, 5..=100).text("Git/SCM Scan Timeout (ms)"));
        ui.add(Slider::new(&mut self.config.background_opacity, 0.5..=1.0).text("Background Opacity"));
        ui.add(Slider::new(&mut self.config.glass_blur_radius, 0.0..=50.0).text("Glass Blur Radius (px)"));
    }

    fn render_metis_tab(&mut self, ui: &mut Ui) {
        ui.heading("Metis Shell & Prefix Trie");
        ui.add(Slider::new(&mut self.config.scrollback_limit, 10_000..=500_000).text("History Limit"));

        ui.add_space(8.0);
        ui.label(RichText::new("Active Abbreviations:").strong());
        for abbrev in &self.abbreviations {
            ui.label(format!("• {} ➔ {}", abbrev.keyword, abbrev.expansion));
        }

        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.new_keyword);
            ui.text_edit_singleline(&mut self.new_expansion);
            if ui.button("Add").clicked() && !self.new_keyword.is_empty() {
                self.abbreviations.push(ShellAbbreviation {
                    keyword: self.new_keyword.clone(),
                    expansion: self.new_expansion.clone(),
                });
                self.new_keyword.clear();
                self.new_expansion.clear();
            }
        });
    }

    fn render_orpheus_tab(&mut self, ui: &mut Ui) {
        ui.heading("Orpheus GPU Text Renderer & CRT Shaders");
        ui.add(Slider::new(&mut self.shader_uniforms.screen_curvature, 0.0..=0.5).text("CRT Barrel Curvature"));
        ui.add(Slider::new(&mut self.shader_uniforms.scanline_frequency, 0.0..=2.0).text("Scanline Frequency"));
        ui.add(Slider::new(&mut self.shader_uniforms.glow_radius, 0.0..=3.0).text("Phosphor Glow Radius"));
    }

    fn render_hermes_tab(&mut self, ui: &mut Ui) {
        ui.heading("Hermes Seqlock State Vector");
        ui.label(format!("Current Theme ID: {}", self.config.theme_id));
        ui.label(format!("Command Timeout: {}ms", self.config.command_timeout_ms));
        ui.label(format!("Scrollback: {} lines", self.config.scrollback_limit));
    }
}
