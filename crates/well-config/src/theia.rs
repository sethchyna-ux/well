//! Settings editor. The application commits the returned candidate after preparing its renderer.
use crate::Preferences;

pub struct TheiasPrismPanel {
    pub open: bool,
    draft: Preferences,
    pub error: Option<String>,
}
impl TheiasPrismPanel {
    pub fn new(preferences: &Preferences) -> Self {
        Self {
            open: false,
            draft: preferences.clone(),
            error: None,
        }
    }
    pub fn toggle(&mut self, active: &Preferences) {
        if !self.open {
            self.draft = active.clone();
            self.error = None;
        }
        self.open = !self.open;
    }
    pub fn applied(&mut self) {
        self.open = false;
        self.error = None;
    }
    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        active: &Preferences,
        path: &std::path::Path,
    ) -> Option<Preferences> {
        let mut candidate = None;
        let mut open = self.open;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Terminal appearance");
                ui.add_space(8.0);
                egui::Grid::new("terminal_settings")
                    .spacing([16.0, 12.0])
                    .show(ui, |ui| {
                        ui.label("Font family");
                        ui.text_edit_singleline(&mut self.draft.appearance.font_family);
                        ui.end_row();
                        ui.label("Font size");
                        ui.add(
                            egui::Slider::new(&mut self.draft.appearance.font_size, 8.0..=48.0)
                                .suffix(" px"),
                        );
                        ui.end_row();
                        ui.label("Line spacing");
                        ui.add(
                            egui::Slider::new(&mut self.draft.appearance.line_spacing, 0.0..=16.0)
                                .suffix(" px"),
                        );
                        ui.end_row();
                        ui.label("Padding");
                        ui.add(
                            egui::Slider::new(&mut self.draft.appearance.padding, 0..=64)
                                .suffix(" px"),
                        );
                        ui.end_row();
                        ui.label("Text");
                        ui.color_edit_button_srgb(&mut self.draft.appearance.foreground);
                        ui.end_row();
                        ui.label("Background");
                        ui.color_edit_button_srgb(&mut self.draft.appearance.background);
                        ui.end_row();
                        ui.label("Border");
                        ui.checkbox(
                            &mut self.draft.appearance.geometric_border,
                            "Cyan / violet geometry",
                        );
                        ui.end_row();
                        ui.label("Wheel speed");
                        ui.add(
                            egui::Slider::new(&mut self.draft.scroll_lines, 1..=20)
                                .suffix(" lines"),
                        );
                        ui.end_row();
                    });
                ui.add_space(12.0);
                ui.small(format!("Saved to {}", path.display()));
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::from_rgb(255, 126, 151), error);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Reset to defaults").clicked() {
                        self.draft = Preferences::default();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(self.draft != *active, egui::Button::new("Save & apply"))
                            .clicked()
                        {
                            match self.draft.validate() {
                                Ok(()) => candidate = Some(self.draft.clone()),
                                Err(e) => self.error = Some(e),
                            }
                        }
                    });
                });
            });
        self.open = open;
        candidate
    }
}
