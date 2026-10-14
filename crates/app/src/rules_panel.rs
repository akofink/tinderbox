//! Data-generated live interaction controls and portable rules text import/export.
use crate::labels::{self, GROUPS};
use eframe::egui;
use std::sync::LazyLock;
use tinderbox_sim::{DEFAULT_RULES, Rules, World};

static DEFAULTS: LazyLock<Rules> =
    LazyLock::new(|| Rules::load(DEFAULT_RULES).expect("validated bundled rules"));

mod filter;
use filter::Filter;

#[cfg_attr(target_arch = "wasm32", derive(Default))]
pub struct RulesPanel {
    pub open: bool,
    #[cfg(not(target_arch = "wasm32"))]
    path: String,
    text: String,
    message: String,
    error: bool,
    filter: Filter,
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
            filter: Filter::default(),
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
        let defaults = &*DEFAULTS;
        // A loaded ruleset may remove the previously filtered element.
        if self
            .filter
            .element
            .as_ref()
            .is_some_and(|name| world.rules().id(name).is_none())
        {
            self.filter.element = None;
        }
        egui::Window::new("Rules")
            .open(&mut open)
            .default_width(470.0)
            .default_height(520.0)
            .default_pos(egui::pos2(190.0, 80.0))
            .movable(true)
            .constrain(true)
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
                ui.add(egui::TextEdit::singleline(&mut self.filter.query)
                    .hint_text("Search rules (e.g. fire tree, heat)")
                    .desired_width(f32::INFINITY));
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.filter.changed_only, "Changed from default");
                    if ui.button("Clear filters").clicked() {
                        self.filter = Filter::default();
                    }
                    if world.rules().id("fire").is_some() && ui.button("Fire rules").clicked() {
                        self.filter = Filter { element: Some("fire".into()), ..Default::default() };
                    }
                });
                ui.collapsing("Filter by element", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut self.filter.element, None, "All elements");
                        for element in &world.rules().elements {
                            ui.selectable_value(&mut self.filter.element, Some(element.name.clone()), labels::display_name(&element.name));
                        }
                    });
                });
                if let Some(element) = &self.filter.element {
                    ui.label(format!("Involving {} (inputs or outputs)", labels::display_name(element)));
                }
                let show_atmosphere = self.filter.atmosphere(&world.rules().atmosphere, &defaults.atmosphere);
                if show_atmosphere {
                egui::CollapsingHeader::new("Atmosphere thresholds and rates")
                    .default_open(false)
                    .show(ui, |ui| {
                        let mut atmosphere = world.rules().atmosphere.clone();
                        let mut changed = false;
                        changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut atmosphere.freezing_point,
                                    -40.0..=atmosphere.melting_point - 0.1,
                                )
                                .text("Freeze below °C"),
                            )
                            .changed();
                        changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut atmosphere.melting_point,
                                    atmosphere.freezing_point + 0.1
                                        ..=atmosphere.boiling_point - 0.1,
                                )
                                .text("Melt above °C"),
                            )
                            .changed();
                        changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut atmosphere.boiling_point,
                                    atmosphere.melting_point + 0.1
                                        ..=atmosphere.ignition_point - 0.1,
                                )
                                .text("Boil above °C"),
                            )
                            .changed();
                        changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut atmosphere.ignition_point,
                                    atmosphere.boiling_point + 0.1..=500.0,
                                )
                                .text("Ignition °C"),
                            )
                            .changed();
                        changed |= ui
                            .add(
                                egui::Slider::new(&mut atmosphere.heat_transfer, 0.0..=1.0)
                                    .text("Heat transfer"),
                            )
                            .changed();
                        changed |= ui
                            .add(
                                egui::Slider::new(&mut atmosphere.evaporation_rate, 0.0..=1.0)
                                    .text("Evaporation rate"),
                            )
                            .changed();
                        changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut atmosphere.condensation_humidity,
                                    0.0..=100.0,
                                )
                                .text("Condense humidity %"),
                            )
                            .changed();
                        changed |= ui
                            .add(
                                egui::Slider::new(&mut atmosphere.pressure_wind, 0.01..=1.0)
                                    .text("Pressure wind"),
                            )
                            .changed();
                        changed |= ui.add(egui::Slider::new(&mut atmosphere.oxygen_consumption, 0.0..=2.0).text("Oxygen consumed per fire update")).changed();
                        changed |= ui.add(egui::Slider::new(&mut atmosphere.oxygen_diffusion, 0.0..=1.0).text("Oxygen diffusion")).changed();
                        if changed {
                            self.report(
                                world
                                    .set_atmosphere_rules(atmosphere)
                                    .map_err(|e| e.to_string()),
                                "Atmosphere tuning updated.",
                            );
                        }
                    });
                }
                ui.collapsing("Decomposer and fungus behavior", |ui| {
                    for (name, label) in [
                        ("fungus", "Fungus lifespan (updates)"),
                        ("beetle", "Beetle lifespan (updates)"),
                        ("termite", "Termite lifespan (updates)"),
                    ] {
                        if let Some(index) = world.rules().id(name) {
                            let mut lifetime = world.rules().elements[index].lifetime;
                            if ui
                                .add(egui::Slider::new(&mut lifetime, 1..=20_000).text(label))
                                .changed()
                            {
                                let _ = world.set_material_lifetime(index, lifetime);
                            }
                        }
                    }
                    let mut ecology = world.rules().ecology.clone();
                    let mut changed = false;
                    changed |= ui
                        .add(egui::Slider::new(&mut ecology.critter_move_chance, 0.0..=1.0)
                            .text("Critter move chance"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut ecology.flee_chance, 0.0..=1.0)
                            .text("Flee chance near fire or water"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut ecology.breeding_chance, 0.0..=0.1)
                            .text("Local breeding chance"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut ecology.fungus_spread_chance, 0.0..=1.0)
                            .text("Fungus spread chance"))
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut ecology.fungus_starvation_chance, 0.0..=1.0)
                                .text("Fungus fade chance without food"),
                        )
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut ecology.fungus_min_humidity, 0.0..=100.0)
                            .text("Minimum fungal humidity %"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut ecology.fungus_kill_temperature, 30.0..=100.0)
                            .text("Fungus dies above °C"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(
                            &mut ecology.fungus_slow_temperature,
                            20.0..=(ecology.fungus_kill_temperature - 1.0).max(20.0),
                        )
                        .text("Fungal growth slows above °C"))
                        .changed();
                    if changed {
                        self.report(
                            world.set_ecology_rules(ecology).map_err(|e| e.to_string()),
                            "Ecology tuning updated.",
                        );
                    }
                });
                ui.collapsing("Material density, moisture, and fire response", |ui| {
                    ui.small("Density: air = 100; lower materials rise and higher materials sink when movable.");
                    let materials: Vec<_> = world.rules().elements.iter().enumerate().map(|(i, e)| (i, e.name.clone(), e.density, e.carries_moisture, e.moisture_rate, e.wet_flammability)).collect();
                    for (index, name, mut density, mut enabled, mut rate, mut wet_flammability) in materials {
                        ui.label(labels::display_name(&name));
                        if ui.add(egui::Slider::new(&mut density, 0..=255).text("Density")).changed() {
                            let _ = world.set_material_density(index, density);
                        }
                        let mut changed = ui.checkbox(&mut enabled, "Carries moisture").changed();
                        changed |= ui.add(egui::Slider::new(&mut rate, 0.0..=0.25).text("Humidity equilibration")).changed();
                        changed |= ui.add(egui::Slider::new(&mut wet_flammability, 0.0..=1.0).text("Wet flammability")).changed();
                        if changed { let _ = world.set_material_moisture(index, enabled, rate, wet_flammability); }
                    }
                });
                ui.separator();
                ui.collapsing("Save / load rules", |ui| {
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
                });
                ui.separator();
                let grouped = grouped_interactions(world, &self.filter, defaults);
                let count: usize = grouped.iter().map(|(_, indices)| indices.len()).sum();
                ui.label(format!("{count} / {} interactions", world.rules().interactions.len()));
                if count == 0 && !show_atmosphere {
                    ui.label("No matching rules. Clear filters to see everything.");
                }
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
                                .id_salt(("rule_group", group, self.filter.active()))
                                .default_open(self.filter.active())
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

fn grouped_interactions(
    world: &World,
    filter: &Filter,
    defaults: &Rules,
) -> Vec<(&'static str, Vec<usize>)> {
    GROUPS
        .into_iter()
        .map(|group| {
            let indices = (0..world.rules().interactions.len())
                .filter(|index| {
                    let when = world.rules().interactions[*index].when;
                    labels::group(&world.rules().elements[when].name) == group
                        && filter.matches(world.rules(), defaults, *index)
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
        _ => format!("{when} + {with} → {result}"),
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
