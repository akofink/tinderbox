//! Data-generated live interaction controls and portable rules text import/export.
use crate::labels::{self, GROUPS};
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
            .default_height(520.0)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 36.0))
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label("Edits apply immediately. Base chance is per neighbor visit; wind can boost it.");
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
                #[cfg(target_arch = "wasm32")]
                browser_files(ui, world);
                ui.separator();
                ui.label(format!("{} interactions", world.rules().interactions.len()));
                let grouped = grouped_interactions(world);
                egui::ScrollArea::vertical()
                    .id_salt("interactions")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .max_height(360.0)
                    .show(ui, |ui| {
                        for (group, indices) in grouped {
                            if indices.is_empty() {
                                continue;
                            }
                            egui::CollapsingHeader::new(format!("{group} ({})", indices.len()))
                                .default_open(group == "Fire")
                                .show(ui, |ui| {
                                    for index in indices {
                                        show_interaction(self, ui, world, index);
                                    }
                                });
                        }
                    });
            });
        self.open = open;
    }
}

fn grouped_interactions(world: &World) -> Vec<(&'static str, Vec<usize>)> {
    GROUPS
        .into_iter()
        .map(|group| {
            let indices = (0..world.rules().interactions.len())
                .filter(|index| {
                    let when = world.rules().interactions[*index].when;
                    labels::group(&world.rules().elements[when].name) == group
                })
                .collect();
            (group, indices)
        })
        .collect()
}

fn interaction_label(world: &World, index: usize) -> String {
    let rule = &world.rules().interactions[index];
    let name = |id: usize| labels::display_name(&world.rules().elements[id].name);
    let when = name(rule.when);
    let with = name(rule.with);
    let result = name(rule.result);
    match rule.self_result {
        Some(id) if id != rule.when => {
            format!("{when} becomes {}; {with} becomes {result}", name(id))
        }
        _ => format!("{with} becomes {result}"),
    }
}

fn show_interaction(panel: &mut RulesPanel, ui: &mut egui::Ui, world: &mut World, index: usize) {
    let rule = &world.rules().interactions[index];
    let label = interaction_label(world, index);
    let mut enabled = rule.enabled;
    let mut chance = rule.chance;
    let downwind_bonus = rule.downwind_bonus;
    ui.push_id(index, |ui| {
        let toggle = ui.checkbox(&mut enabled, label).changed();
        let tune = ui
            .indent("chance", |ui| {
                let changed = ui
                    .add(egui::Slider::new(&mut chance, 0.0..=1.0).text("base chance"))
                    .changed();
                if downwind_bonus > 0.0 {
                    ui.small(format!("Downwind bonus: up to +{downwind_bonus:.2}"));
                }
                changed
            })
            .inner;
        if toggle || tune {
            panel.report(
                world
                    .set_interaction(index, enabled, chance)
                    .map_err(|e| e.to_string()),
                "Interaction updated.",
            );
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn browser_files(ui: &mut egui::Ui, world: &World) {
    use crate::app::files;
    ui.horizontal_wrapped(|ui| {
        if ui.button("Download rules").clicked() {
            let result = world
                .rules()
                .to_toml()
                .map_err(|e| e.to_string())
                .and_then(|text| files::save_text("rules", "tinderbox-rules.toml", &text));
            match result {
                Ok(()) => files::set_notice("Downloaded the current tuned rules."),
                Err(err) => files::set_notice(err),
            }
        }
        if ui.button("Upload rules").clicked() {
            if let Err(err) = files::begin_upload("rules") {
                files::set_notice(err);
            }
        }
        if ui.button("Stored rules").clicked() {
            match files::load_text("rules") {
                Ok(text) => files::finish_upload("rules", Ok(text)),
                Err(err) => files::set_notice(err),
            }
        }
    });
    let notice = files::notice();
    if !notice.is_empty() {
        ui.label(notice);
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
