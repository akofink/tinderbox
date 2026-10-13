//! Data-generated live interaction controls and portable rules text import/export.
use eframe::egui;
use tinderbox_sim::World;

#[cfg_attr(target_arch = "wasm32", derive(Default))]
pub struct RulesPanel {
    pub open: bool,
    #[cfg(not(target_arch = "wasm32"))]
    path: String,
    text: String,
    message: String,
    error: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for RulesPanel {
    fn default() -> Self {
        Self {
            open: false,
            #[cfg(not(target_arch = "wasm32"))]
            path: "tinderbox-rules.toml".into(),
            text: String::new(),
            message: String::new(),
            error: false,
        }
    }
}

impl RulesPanel {
    fn report(&mut self, result: Result<(), String>, success: &str) {
        self.error = result.is_err();
        self.message = result.err().unwrap_or_else(|| success.into());
    }

    fn apply_text(&mut self, world: &mut World) {
        let result = world.load_rules(&self.text).map_err(|e| e.to_string());
        self.report(result, "Rules applied. The world keeps running.");
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_file(&mut self, world: &mut World) {
        let result = std::fs::read_to_string(&self.path)
            .map_err(|e| format!("Cannot read rules: {e}"))
            .and_then(|text| world.load_rules(&text).map_err(|e| e.to_string()));
        self.report(result, "Rules loaded. The world keeps running.");
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_file(&mut self, world: &World) {
        let result = world
            .rules()
            .to_toml()
            .map_err(|e| e.to_string())
            .and_then(|text| {
                std::fs::write(&self.path, text).map_err(|e| format!("Cannot save rules: {e}"))
            });
        self.report(result, "Rules saved.");
    }

    pub fn show(&mut self, ctx: &egui::Context, world: &mut World) {
        let mut open = self.open;
        egui::Window::new("Rules")
            .open(&mut open)
            .default_width(470.0)
            .default_height(480.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label("Edits apply immediately. Chance is per neighboring-cell visit.");
                if !self.message.is_empty() {
                    let color = match (self.error, ui.visuals().dark_mode) {
                        (true, true) => egui::Color32::LIGHT_RED,
                        (false, true) => egui::Color32::LIGHT_GREEN,
                        (true, false) => egui::Color32::from_rgb(160, 25, 25),
                        (false, false) => egui::Color32::from_rgb(20, 110, 40),
                    };
                    ui.colored_label(color, &self.message);
                    if self.error {
                        ui.label("Current rules are unchanged.");
                    }
                }
                if ui.button("Reset rules to defaults").clicked() {
                    self.report(
                        world.reset_rules().map_err(|e| e.to_string()),
                        "Default rules restored.",
                    );
                }
                ui.separator();
                for index in 0..world.rules().interactions.len() {
                    let rule = &world.rules().interactions[index];
                    let name = |id: usize| world.rules().elements[id].name.as_str();
                    let label = format!(
                        "{} + {} -> {} + {}",
                        name(rule.when),
                        name(rule.with),
                        name(rule.self_result.unwrap_or(rule.when)),
                        name(rule.result)
                    );
                    let mut enabled = rule.enabled;
                    let mut chance = rule.chance;
                    ui.push_id(index, |ui| {
                        let toggle = ui.checkbox(&mut enabled, label).changed();
                        let tune = ui
                            .indent("chance", |ui| {
                                ui.add(egui::Slider::new(&mut chance, 0.0..=1.0).text("chance"))
                                    .changed()
                            })
                            .inner;
                        if toggle || tune {
                            self.report(
                                world
                                    .set_interaction(index, enabled, chance)
                                    .map_err(|e| e.to_string()),
                                "Interaction updated.",
                            );
                        }
                    });
                }
                ui.separator();
                #[cfg(not(target_arch = "wasm32"))]
                {
                    ui.label("Rules file path (relative to working directory or absolute)");
                    ui.text_edit_singleline(&mut self.path);
                    ui.horizontal(|ui| {
                        if ui.button("Save file").clicked() {
                            self.save_file(world);
                        }
                        if ui.button("Load file").clicked() {
                            self.load_file(world);
                        }
                    });
                }
                ui.collapsing("Rules TOML (copy/paste)", |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Export current rules").clicked() {
                            match world.rules().to_toml() {
                                Ok(text) => {
                                    self.text = text;
                                    self.report(Ok(()), "Rules exported below.");
                                }
                                Err(e) => self.report(Err(e.to_string()), ""),
                            }
                        }
                        if ui.button("Apply TOML").clicked() {
                            self.apply_text(world);
                        }
                    });
                    ui.add(
                        egui::TextEdit::multiline(&mut self.text)
                            .code_editor()
                            .desired_rows(8)
                            .desired_width(f32::INFINITY),
                    );
                });
            });
        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinderbox_sim::{DEFAULT_RULES, Rules};

    fn world() -> World {
        World::new(2, 2, 7, Rules::load(DEFAULT_RULES).unwrap()).unwrap()
    }

    #[test]
    fn invalid_pasted_rules_show_error_and_preserve_live_rules() {
        let mut world = world();
        world.set_interaction(0, false, 0.25).unwrap();
        let before = world.rules().to_toml().unwrap();
        let mut panel = RulesPanel {
            text: "not TOML".into(),
            ..Default::default()
        };
        panel.apply_text(&mut world);
        assert!(panel.error);
        assert!(panel.message.contains("invalid rules TOML"));
        assert_eq!(world.rules().to_toml().unwrap(), before);
        panel.text = DEFAULT_RULES.into();
        panel.apply_text(&mut world);
        assert!(!panel.error);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn invalid_file_shows_error_and_preserves_live_rules() {
        let mut world = world();
        world.set_interaction(0, false, 0.25).unwrap();
        let before = world.rules().to_toml().unwrap();
        // A real, readable TOML file, but not a ruleset. No temporary files needed.
        let mut panel = RulesPanel {
            path: format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR")),
            ..Default::default()
        };
        panel.load_file(&mut world);
        assert!(panel.error);
        assert!(panel.message.contains("invalid rules TOML"));
        assert_eq!(world.rules().to_toml().unwrap(), before);
        panel.path = format!("{}/src", env!("CARGO_MANIFEST_DIR"));
        panel.load_file(&mut world);
        assert!(panel.error);
        assert!(panel.message.contains("Cannot read rules"));
        assert_eq!(world.rules().to_toml().unwrap(), before);
    }
}
