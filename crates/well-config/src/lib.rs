#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WellConfig {
    pub theme: Theme,
    pub font_size: f32,
    pub scrollback_lines: usize,
    pub shell_integration: bool,
    pub confirm_destructive_commands: bool,
}

impl Default for WellConfig {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            font_size: 14.0,
            scrollback_lines: 10_000,
            shell_integration: true,
            confirm_destructive_commands: true,
        }
    }
}

/// Render the configuration panel and return whether any setting changed.
pub fn show_dashboard(context: &egui::Context, config: &mut WellConfig) -> bool {
    let mut changed = false;

    egui::CentralPanel::default().show(context, |ui| {
        ui.heading("Well settings");
        ui.separator();

        ui.label("Appearance");
        egui::ComboBox::from_label("Theme")
            .selected_text(config.theme.label())
            .show_ui(ui, |ui| {
                for theme in [Theme::System, Theme::Light, Theme::Dark] {
                    changed |= ui
                        .selectable_value(&mut config.theme, theme, theme.label())
                        .changed();
                }
            });
        changed |= ui
            .add(egui::Slider::new(&mut config.font_size, 10.0..=28.0).text("Font size"))
            .changed();

        ui.add_space(8.0);
        ui.label("Terminal");
        changed |= ui
            .add(
                egui::Slider::new(&mut config.scrollback_lines, 1_000..=100_000)
                    .text("Scrollback lines"),
            )
            .changed();
        changed |= ui
            .checkbox(&mut config.shell_integration, "Enable shell integration")
            .changed();

        ui.add_space(8.0);
        ui.label("Safety");
        changed |= ui
            .checkbox(
                &mut config.confirm_destructive_commands,
                "Confirm destructive commands",
            )
            .changed();
    });

    changed
}

#[cfg(test)]
mod tests {
    use super::{show_dashboard, Theme, WellConfig};

    #[test]
    fn defaults_are_conservative_and_render_without_changes() {
        let mut config = WellConfig::default();
        assert_eq!(config.theme, Theme::System);
        assert_eq!(config.scrollback_lines, 10_000);
        assert!(config.shell_integration);
        assert!(config.confirm_destructive_commands);

        let context = egui::Context::default();
        let mut changed = false;
        let _ = context.run(Default::default(), |context| {
            changed = show_dashboard(context, &mut config);
        });
        assert!(!changed);
    }
}
