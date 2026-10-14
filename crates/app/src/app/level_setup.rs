//! Setup UI is derived from the current rules, not a hard-coded element list.
use super::files;
use crate::labels::{self, GROUPS};
use eframe::egui;
use tinderbox_sim::{
    Rules, WindDirection,
    level::{Generator, Level, LevelMode, MAX_LEVEL_DIMENSION, Preset},
};

pub(crate) enum SetupAction {
    Play(Level),
    Cancel,
}

pub(crate) struct LevelSetup {
    pub open: bool,
    pub draft: Level,
    pub error: Option<String>,
    pub revision: u64,
    text: String,
    seed_text: String,
    #[cfg(not(target_arch = "wasm32"))]
    file_path: String,
    #[cfg(not(target_arch = "wasm32"))]
    loaded_path: Option<std::path::PathBuf>,
    message: String,
}
impl LevelSetup {
    pub fn new(level: Level) -> Self {
        Self {
            open: true,
            seed_text: level.seed.to_string(),
            draft: level,
            error: None,
            revision: 0,
            text: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            file_path: "tinderbox-level.toml".into(),
            #[cfg(not(target_arch = "wasm32"))]
            loaded_path: None,
            message: String::new(),
        }
    }
    pub fn begin(&mut self, level: &Level, rules: &Rules) {
        self.revision = self.revision.wrapping_add(1);
        self.draft = level.clone();
        self.draft.add_missing_elements(rules);
        self.seed_text = self.draft.seed.to_string();
        self.error = None;
        self.message.clear();
        self.open = true;
    }
    pub(super) fn import_text(&mut self, text: String, rules: &Rules) {
        self.text = text;
        self.load_text(rules);
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn take_loaded_path(&mut self) -> Option<std::path::PathBuf> {
        self.loaded_path.take()
    }
    pub(super) fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }
    fn load_text(&mut self, rules: &Rules) {
        match Level::load(&self.text).and_then(|level| {
            // Validate names before replacing the draft, but don't touch the game.
            for name in level.amounts.keys() {
                if rules.id(name).is_none() {
                    return Err(tinderbox_sim::level::LevelError(format!(
                        "Level refers to unknown element '{name}'"
                    )));
                }
            }
            Ok(level)
        }) {
            Ok(mut level) => {
                level.add_missing_elements(rules);
                self.seed_text = level.seed.to_string();
                self.draft = level;
                self.revision = self.revision.wrapping_add(1);
                self.error = None;
                self.message = "Loaded settings. The preview will update shortly.".into();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn encode_draft(&self) -> Result<String, tinderbox_sim::level::LevelError> {
        self.seed_text.parse::<u64>().map_err(|_| {
            tinderbox_sim::level::LevelError(
                "Seed must be a whole number from 0 to 18446744073709551615".into(),
            )
        })?;
        self.draft.to_toml()
    }
    pub fn show(&mut self, ctx: &egui::Context, rules: &Rules) -> Option<SetupAction> {
        if !self.open {
            return None;
        }
        let mut action = None;
        let original_draft = self.draft.clone();
        // No title-bar close button: Cancel explicitly restores the paused game.
        egui::Window::new("Level setup")
            .id(egui::Id::new("level_setup"))
            .collapsible(false)
            .resizable(true)
            .default_width(760.0)
            .default_height(640.0)
            .max_height((ctx.screen_rect().height() - 32.0).max(180.0))
            .max_width((ctx.screen_rect().width() - 32.0).max(320.0))
            .default_pos(egui::pos2(48.0, 48.0))
            .movable(true)
            .constrain(true)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label("Edit the stage and watch the world preview update. Cancel restores your game.");
                ui.horizontal_wrapped(|ui| {
                    ui.label("Presets:");
                    for preset in Preset::ALL {
                        if ui.button(preset.label()).clicked() {
                            self.draft = preset.level(rules, self.draft.width, self.draft.height);
                            self.seed_text = self.draft.seed.to_string();
                            self.error = None;
                            self.message.clear();
                        }
                    }
                });
                ui.separator();
                let seed_valid = self.seed_text.parse::<u64>().is_ok();
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.draft.validate().is_ok() && seed_valid, egui::Button::new("Play level")).clicked() {
                        action = Some(SetupAction::Play(self.draft.clone()));
                    }
                    if ui.button("Cancel / return to game").clicked() { action = Some(SetupAction::Cancel); }
                });
                ui.checkbox(&mut self.draft.fit_window, "Fit world to canvas");
                egui::ComboBox::from_label("World mode")
                    .selected_text(match self.draft.mode {
                        LevelMode::Fixed => "Fixed size",
                        LevelMode::Endless => "Endless chunks",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.draft.mode, LevelMode::Fixed, "Fixed size");
                        ui.selectable_value(&mut self.draft.mode, LevelMode::Endless, "Endless chunks");
                    });
                ui.collapsing("Advanced world size override", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Width");
                        ui.add(egui::DragValue::new(&mut self.draft.width).range(16..=MAX_LEVEL_DIMENSION));
                        ui.label("Height");
                        ui.add(egui::DragValue::new(&mut self.draft.height).range(16..=MAX_LEVEL_DIMENSION));
                        if ui.button("Use these dimensions").clicked() {
                            self.draft.fit_window = false;
                        }
                    });
                    ui.small("Disable Fit world to canvas to use these dimensions as an explicit recipe size.");
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Seed");
                    if ui.add(egui::TextEdit::singleline(&mut self.seed_text).desired_width(145.0)).changed()
                        && let Ok(seed) = self.seed_text.parse::<u64>() {
                        self.draft.seed = seed;
                    }
                    if ui.button("Next seed").clicked() {
                        self.draft.seed = self.draft.seed.wrapping_add(1);
                        self.seed_text = self.draft.seed.to_string();
                    }
                });
                egui::ComboBox::from_label("Terrain")
                    .selected_text(self.draft.generator.label())
                    .show_ui(ui, |ui| {
                        for generator in Generator::ALL {
                            ui.selectable_value(&mut self.draft.generator, generator, generator.label());
                        }
                    });
                ui.checkbox(&mut self.draft.boat, "Include a wooden boat over water");
                if self.draft.boat {
                    ui.add(egui::Slider::new(&mut self.draft.boat_sand, 0..=8).text("Sand cargo cells"));
                    ui.checkbox(&mut self.draft.boat_burning, "Start the hull burning");
                }
                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("Starting wind")
                        .selected_text(format!("{:?}", self.draft.wind.direction))
                        .show_ui(ui, |ui| {
                            for direction in [WindDirection::Calm, WindDirection::West, WindDirection::East] {
                                ui.selectable_value(&mut self.draft.wind.direction, direction, format!("{direction:?}"));
                            }
                        });
                    ui.add(egui::Slider::new(&mut self.draft.wind.strength, 0..=4).text("strength"));
                });
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::Slider::new(&mut self.draft.ambient_temperature, -40..=60).text("Ambient temperature").suffix(" °C"));
                    ui.add(egui::Slider::new(&mut self.draft.ambient_humidity, 0..=100).text("Ambient humidity").suffix("%"));
                    ui.add(egui::Slider::new(&mut self.draft.ambient_oxygen, 0..=21).text("Ambient oxygen").suffix("%"));
                });
                ui.small("Amounts are density in each material's terrain region, not percent of the whole world. Empty canvas ignores amounts.");
                let names: Vec<String> = rules.elements.iter().map(|element| element.name.clone()).collect();
                ui.horizontal(|ui| {
                    let split = GROUPS.len().div_ceil(2);
                    let column_groups = [&GROUPS[..split], &GROUPS[split..]];
                    for groups in column_groups {
                        ui.vertical(|ui| {
                            ui.set_min_width(330.0);
                            for group in groups {
                                let grouped: Vec<_> = names.iter().filter(|name| *name != "air" && labels::group(name) == *group).cloned().collect();
                                if grouped.is_empty() { continue; }
                                ui.label(egui::RichText::new(*group).small().strong());
                                for name in grouped {
                                    let amount = self.draft.amounts.entry(name.clone()).or_default();
                                    ui.add(egui::Slider::new(amount, 0..=100).text(labels::display_name(&name)).suffix("%"));
                                }
                            }
                        });
                    }
                });
                if let Some(error) = &self.error { ui.colored_label(egui::Color32::LIGHT_RED, error); }
                if let Err(e) = self.draft.validate() { ui.colored_label(egui::Color32::LIGHT_RED, e.to_string()); }
                if !seed_valid { ui.colored_label(egui::Color32::LIGHT_RED, "Seed must be a whole number from 0 to 18446744073709551615"); }
                if !self.message.is_empty() { ui.label(&self.message); }
                ui.separator();
                ui.collapsing("Save / load level (TOML)", |ui| {
                    #[cfg(not(target_arch = "wasm32"))]
                    ui.horizontal(|ui| {
                        ui.label("File");
                        ui.text_edit_singleline(&mut self.file_path);
                        if ui.button("Save file").clicked() {
                            match self.encode_draft().map_err(|e| e.to_string())
                                .and_then(|text| files::save_text("level", &self.file_path, &text)) {
                                Ok(()) => { self.error = None; self.message = format!("Saved {}", self.file_path); }
                                Err(e) => self.error = Some(e),
                            }
                        }
                        if ui.button("Load file").clicked() {
                            match files::load_text(&self.file_path) {
                                Ok(text) => {
                                    self.import_text(text, rules);
                                    if self.error.is_none() {
                                        self.loaded_path = Some(self.file_path.clone().into());
                                    }
                                }
                                Err(e) => self.error = Some(e),
                            }
                        }
                    });
                    #[cfg(target_arch = "wasm32")]
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Download level").clicked() {
                            match self.encode_draft().map_err(|e| e.to_string())
                                .and_then(|text| files::save_text("level", "tinderbox-level.toml", &text)) {
                                Ok(()) => { self.error = None; self.message = "Downloaded level and stored it in this browser.".into(); }
                                Err(e) => self.error = Some(e),
                            }
                        }
                        if ui.button("Upload level").clicked() {
                            if let Err(e) = files::begin_upload("level") { self.error = Some(e); }
                        }
                        if ui.button("Stored level").clicked() {
                            match files::load_text("level") {
                                Ok(text) => self.import_text(text, rules),
                                Err(e) => self.error = Some(e),
                            }
                        }
                        let notice = files::notice();
                        if !notice.is_empty() { ui.label(notice); }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Export settings to text").clicked() {
                            match self.encode_draft() {
                                Ok(text) => { self.text = text; self.error = None; }
                                Err(e) => self.error = Some(e.to_string()),
                            }
                        }
                        if ui.button("Copy text").clicked() { ctx.copy_text(self.text.clone()); }
                        if ui.button("Load text").clicked() { self.load_text(rules); }
                    });
                    ui.small("Paste a level file below, or copy the exported text to share it.");
                    egui::ScrollArea::vertical().id_salt("level_text").max_height(160.0).show(ui, |ui| {
                        ui.add(egui::TextEdit::multiline(&mut self.text).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).desired_rows(8));
                    });
                });

            });
        if self.draft != original_draft {
            self.revision = self.revision.wrapping_add(1);
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinderbox_sim::DEFAULT_RULES;
    #[test]
    fn setup_draws_without_gpu_at_small_viewport() {
        let rules = Rules::load(DEFAULT_RULES).unwrap();
        let mut setup = LevelSetup::new(Preset::DryForest.level(&rules, 384, 240));
        let ctx = egui::Context::default();
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(720.0, 520.0),
                )),
                ..Default::default()
            },
            |ctx| {
                setup.show(ctx, &rules);
            },
        );
        assert!(!out.shapes.is_empty());
    }
    #[test]
    fn invalid_seed_cannot_be_saved_and_large_seed_round_trips() {
        let rules = Rules::load(DEFAULT_RULES).unwrap();
        let mut level = Preset::DryForest.level(&rules, 96, 64);
        level.seed = u64::MAX;
        let mut setup = LevelSetup::new(level.clone());
        assert_eq!(Level::load(&setup.encode_draft().unwrap()).unwrap(), level);
        setup.seed_text = "bad seed".into();
        assert!(setup.encode_draft().is_err());
        assert_eq!(setup.draft.seed, u64::MAX);
    }
    #[test]
    fn loading_bad_text_preserves_draft_and_valid_text_adds_missing_elements() {
        let rules = Rules::load(DEFAULT_RULES).unwrap();
        let level = Preset::DryForest.level(&rules, 96, 64);
        let mut setup = LevelSetup::new(level.clone());
        setup.text = "bad file".into();
        setup.load_text(&rules);
        assert_eq!(setup.draft, level);
        assert!(setup.error.is_some());
        let mut island = Preset::Island.level(&rules, 96, 64);
        island.amounts.remove("fire");
        setup.text = island.to_toml().unwrap();
        setup.load_text(&rules);
        assert!(setup.error.is_none());
        assert_eq!(setup.draft.amounts.get("fire"), Some(&0));
        assert_eq!(setup.draft.generator, Generator::Island);
        let saved = setup.draft.clone();
        island.amounts.insert("unavailable".into(), 10);
        setup.text = island.to_toml().unwrap();
        setup.load_text(&rules);
        assert_eq!(setup.draft, saved);
        assert!(setup.error.as_ref().unwrap().contains("unknown element"));
    }
}
