//! Human-readable, seeded world recipes. No clocks, filesystem, or graphics.
use crate::{Motion, Rules, Wind, WindDirection, World};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

pub const MAX_LEVEL_CELLS: usize = 1_048_576;
pub const MAX_LEVEL_DIMENSION: usize = 2048;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Generator {
    Empty,
    #[default]
    Hills,
    Island,
    Valley,
}
impl Generator {
    pub const ALL: [Self; 4] = [Self::Empty, Self::Hills, Self::Island, Self::Valley];
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "Empty canvas",
            Self::Hills => "Rolling hills",
            Self::Island => "Island",
            Self::Valley => "Valley",
        }
    }
}

fn default_ambient_temperature() -> i16 {
    20
}
fn default_ambient_humidity() -> u8 {
    45
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub version: u32,
    pub width: usize,
    pub height: usize,
    // TOML integers are signed 64-bit. A decimal string preserves every u64 seed.
    #[serde(with = "seed_format")]
    pub seed: u64,
    pub generator: Generator,
    #[serde(default)]
    pub wind: Wind,
    #[serde(default = "default_ambient_temperature")]
    pub ambient_temperature: i16,
    #[serde(default = "default_ambient_humidity")]
    pub ambient_humidity: u8,
    /// Density (0..=100) within each material's terrain region, keyed by name.
    pub amounts: BTreeMap<String, u8>,
    /// A small upwind fire, instead of scattering fire through the whole canopy.
    #[serde(default)]
    pub spark: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelError(pub String);
impl fmt::Display for LevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for LevelError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    DryForest,
    Island,
    SandstormValley,
}
impl Preset {
    pub const ALL: [Self; 3] = [Self::DryForest, Self::Island, Self::SandstormValley];
    pub fn label(self) -> &'static str {
        match self {
            Self::DryForest => "Dry forest, east wind",
            Self::Island => "Island",
            Self::SandstormValley => "Sandstorm valley",
        }
    }
    pub fn level(self, rules: &Rules, width: usize, height: usize) -> Level {
        let (generator, seed, amounts): (_, _, &[(&str, u8)]) = match self {
            Self::DryForest => (
                Generator::Hills,
                7,
                &[("stone", 100), ("soil", 100), ("tree", 75)],
            ),
            Self::Island => (
                Generator::Island,
                29,
                &[
                    ("stone", 100),
                    ("soil", 100),
                    ("sand", 65),
                    ("tree", 50),
                    ("water", 100),
                    ("plant", 45),
                ],
            ),
            Self::SandstormValley => (
                Generator::Valley,
                83,
                &[
                    ("stone", 100),
                    ("sand", 100),
                    ("ash", 15),
                    ("smoke", 15),
                    ("fire", 5),
                ],
            ),
        };
        let mut level = Level {
            version: 1,
            width,
            height,
            seed,
            generator,
            ambient_temperature: 20,
            ambient_humidity: 45,
            wind: match self {
                Self::DryForest => Wind {
                    direction: WindDirection::East,
                    strength: 1,
                },
                Self::Island => Wind::default(),
                Self::SandstormValley => Wind {
                    direction: WindDirection::East,
                    strength: 3,
                },
            },
            amounts: BTreeMap::new(),
            spark: matches!(self, Self::DryForest),
        };
        level.add_missing_elements(rules);
        for &(name, amount) in amounts {
            if let Some(value) = level.amounts.get_mut(name) {
                *value = amount;
            }
        }
        level
    }
}

impl Level {
    /// New ruleset elements automatically get an amount control; air is implicit.
    pub fn add_missing_elements(&mut self, rules: &Rules) {
        for element in &rules.elements {
            if element.name != "air" {
                self.amounts.entry(element.name.clone()).or_default();
            }
        }
    }
    pub fn load(text: &str) -> Result<Self, LevelError> {
        let level: Self =
            toml::from_str(text).map_err(|e| LevelError(format!("Invalid level TOML: {e}")))?;
        level.validate()?;
        Ok(level)
    }
    pub fn to_toml(&self) -> Result<String, LevelError> {
        self.validate()?;
        toml::to_string_pretty(self).map_err(|e| LevelError(format!("Cannot encode level: {e}")))
    }
    pub fn validate(&self) -> Result<(), LevelError> {
        if self.version != 1 {
            return Err(LevelError(format!(
                "Unsupported level version {}; expected 1",
                self.version
            )));
        }
        if self.width < 16
            || self.height < 16
            || self.width > MAX_LEVEL_DIMENSION
            || self.height > MAX_LEVEL_DIMENSION
            || self
                .width
                .checked_mul(self.height)
                .is_none_or(|n| n > MAX_LEVEL_CELLS)
        {
            return Err(LevelError(format!(
                "World must be 16..={MAX_LEVEL_DIMENSION} cells per side and at most {MAX_LEVEL_CELLS} cells total"
            )));
        }
        if self.wind.strength > 4 {
            return Err(LevelError("Wind strength must be 0..=4".into()));
        }
        if !(-40..=60).contains(&self.ambient_temperature) || self.ambient_humidity > 100 {
            return Err(LevelError(
                "Ambient temperature must be -40..=60 C and humidity 0..=100%".into(),
            ));
        }
        for (name, amount) in &self.amounts {
            if name.trim().is_empty() || name == "air" {
                return Err(LevelError("Amounts must name non-air elements".into()));
            }
            if *amount > 100 {
                return Err(LevelError(format!("Amount for '{name}' must be 0..=100")));
            }
        }
        Ok(())
    }
    pub fn generate(&self, rules: Rules) -> Result<World, LevelError> {
        self.validate()?;
        for name in self.amounts.keys() {
            if rules.id(name).is_none() {
                return Err(LevelError(format!(
                    "Level refers to unknown element '{name}'"
                )));
            }
        }
        let mut world = World::new(self.width, self.height, self.seed, rules)
            .map_err(|e| LevelError(e.to_string()))?;
        world.set_wind(self.wind.direction, self.wind.strength);
        world.set_ambient(self.ambient_temperature, self.ambient_humidity);
        // Empty means a blank canvas regardless of amounts, not floating terrain.
        if self.generator == Generator::Empty {
            return Ok(world);
        }
        let surface: Vec<usize> = (0..self.width).map(|x| self.surface(x)).collect();
        let waterline = self.height * 3 / 4;
        // Layers are ordered by geometry, never by ruleset IDs or map iteration order.
        let mut materials: Vec<_> = world
            .rules
            .elements
            .iter()
            .enumerate()
            .filter_map(|(id, e)| {
                let amount = self.amounts.get(&e.name).copied().unwrap_or(0);
                (amount > 0).then_some((layer(&e.name, e.motion), id, e, amount))
            })
            .collect();
        materials.sort_by(|a, b| (a.0, &a.2.name).cmp(&(b.0, &b.2.name)));
        for (_, id, element, amount) in materials {
            let salt = name_hash(&element.name);
            for (x, &ground) in surface.iter().enumerate() {
                let flooded =
                    ground > waterline && self.amounts.get("water").copied().unwrap_or(0) > 0;
                let canopy = (self.height / 12
                    + (sample(self.seed ^ name_hash("tree"), x / 4, 1)
                        % (self.height / 8).max(1) as u64) as usize)
                    .saturating_sub((x % 4).abs_diff(1))
                    .max(2);
                for y in 0..self.height {
                    let eligible = match element.name.as_str() {
                        "stone" => y >= ground,
                        "soil" => y >= ground && y < ground + (self.height / 32).max(2),
                        "tree" => {
                            // A trunk plus a wider crown, so a forest is not a row of bars.
                            let column = x % 4;
                            let above = ground.saturating_sub(y);
                            !flooded
                                && y < ground
                                && ((column == 1 && above <= canopy)
                                    || (column <= 2 && above > 1 && above <= canopy / 2))
                        }
                        "plant" | "grass" => !flooded && y < ground && y + 3 >= ground,
                        "water" => y >= waterline && y < ground,
                        "sand" | "ash" => y >= ground && y < ground + (self.height / 16).max(2),
                        "fire" => !flooded && y + (canopy / 2).max(1) == ground,
                        _ => match element.motion {
                            Motion::Static => y < ground && y + (self.height / 16).max(2) >= ground,
                            Motion::Fall => y >= ground && y < ground + (self.height / 24).max(2),
                            Motion::Liquid => y >= waterline && y < ground,
                            Motion::Gas => {
                                y < ground && y >= self.height / 3 && y < self.height / 2
                            }
                        },
                    };
                    // Column-coherent forest clumps; per-cell density for other materials.
                    let (density_x, density_y) = if element.name == "tree" {
                        (x / 4, 0)
                    } else {
                        (x, y)
                    };
                    if eligible
                        && sample(self.seed ^ salt, density_x, density_y) % 100 < u64::from(amount)
                    {
                        world.cells[y * self.width + x].element = id;
                    }
                }
            }
        }
        if self.spark {
            place_spark(&mut world);
        }
        world.mark_all_dirty();
        Ok(world)
    }
}

fn place_spark(world: &mut World) {
    let Some(fire) = world.rules.id("fire") else {
        return;
    };
    let tree = world.rules.id("tree");
    let limit = (world.width() / 8).max(2);
    let mut at = None;
    if let Some(tree) = tree {
        'search: for x in 0..limit {
            for y in 0..world.height() {
                if world.cells[y * world.width() + x].element == tree {
                    at = Some((x, y));
                    break 'search;
                }
            }
        }
    }
    let (x, y) = at.unwrap_or((limit / 2, world.height() * 2 / 3));
    for dy in -1isize..=1 {
        for dx in 0isize..=2 {
            let Some(px) = x.checked_add_signed(dx).filter(|px| *px < world.width()) else {
                continue;
            };
            let Some(py) = y.checked_add_signed(dy).filter(|py| *py < world.height()) else {
                continue;
            };
            world.paint(px, py, fire);
        }
    }
}

impl Level {
    fn surface(&self, x: usize) -> usize {
        let wave = |span: usize, salt: u64| -> usize {
            let section = x / span;
            let t = x % span;
            let a = sample(self.seed ^ salt, section, 0) % 1024;
            let b = sample(self.seed ^ salt, section + 1, 0) % 1024;
            (a as usize * (span - t) + b as usize * t) / span
        };
        let noise = wave((self.width / 10).max(4), 913) * self.height / 1024 / 12;
        let center_distance = x.abs_diff(self.width / 2) * 2;
        let base = match self.generator {
            Generator::Hills => {
                self.height * 2 / 3 + wave((self.width / 4).max(4), 23) * self.height / 1024 / 6
            }
            Generator::Island => self.height / 2 + center_distance * self.height / self.width / 2,
            Generator::Valley => {
                self.height * 5 / 6 - center_distance * self.height / self.width / 3
            }
            Generator::Empty => self.height,
        };
        (base + noise).min(self.height - 1)
    }
}

mod seed_format {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Seed {
            Text(String),
            Number(u64),
        }
        match Seed::deserialize(deserializer)? {
            Seed::Text(value) => value.parse().map_err(D::Error::custom),
            Seed::Number(value) => Ok(value),
        }
    }
}

fn layer(name: &str, motion: Motion) -> u8 {
    match name {
        "stone" => 0,
        "soil" => 1,
        "sand" | "ash" => 2,
        "tree" => 3,
        "water" => 5,
        "fire" => 7,
        _ => match motion {
            Motion::Static => 4,
            Motion::Fall => 2,
            Motion::Liquid => 6,
            Motion::Gas => 7,
        },
    }
}
fn name_hash(name: &str) -> u64 {
    name.bytes().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}
fn sample(seed: u64, x: usize, y: usize) -> u64 {
    let mut n = seed
        .wrapping_add((x as u64).wrapping_mul(0x9e3779b97f4a7c15))
        .wrapping_add((y as u64).wrapping_mul(0xd1b54a32d192ed03));
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_RULES;
    fn rules() -> Rules {
        Rules::load(DEFAULT_RULES).unwrap()
    }
    #[test]
    fn generators_and_presets_are_deterministic_including_simulation() {
        for preset in Preset::ALL {
            for generator in Generator::ALL {
                let mut level = preset.level(&rules(), 96, 64);
                level.generator = generator;
                let mut a = level.generate(rules()).unwrap();
                let mut b = level.generate(rules()).unwrap();
                assert_eq!(a.cells, b.cells);
                for _ in 0..30 {
                    a.step(1024);
                    b.step(1024);
                }
                assert_eq!(a.cells, b.cells);
            }
        }
    }
    #[test]
    fn every_preset_file_round_trips_and_reset_restores_start() {
        for preset in Preset::ALL {
            let level = preset.level(&rules(), 96, 64);
            let text = level.to_toml().unwrap();
            let loaded = Level::load(&text).unwrap();
            assert_eq!(level, loaded);
            let start = level.generate(rules()).unwrap();
            let mut played = loaded.generate(rules()).unwrap();
            played.paint(1, 1, played.rules.id("fire").unwrap());
            for _ in 0..30 {
                played.step(2048);
            }
            assert_ne!(start.cells, played.cells);
            let reset = loaded.generate(played.rules).unwrap();
            assert_eq!(start.cells, reset.cells);
            assert_eq!(start.wind(), reset.wind());
            assert_eq!(start.wind(), level.wind);
        }
    }
    #[test]
    fn seed_settings_and_amounts_change_world() {
        let mut level = Preset::DryForest.level(&rules(), 96, 64);
        let baseline = level.generate(rules()).unwrap().cells;
        level.seed += 1;
        assert_ne!(baseline, level.generate(rules()).unwrap().cells);
        level.seed -= 1;
        level.amounts.insert("tree".into(), 0);
        let no_tree = level.generate(rules()).unwrap();
        assert!(
            !no_tree
                .cells
                .iter()
                .any(|c| Some(c.element) == no_tree.rules.id("tree"))
        );
        assert_ne!(baseline, no_tree.cells);
        level.generator = Generator::Empty;
        let empty = level.generate(rules()).unwrap();
        assert!(
            empty
                .cells
                .iter()
                .all(|c| Some(c.element) == empty.rules.id("air"))
        );
    }
    #[test]
    fn preset_landmarks_exist_and_small_worlds_work() {
        for preset in Preset::ALL {
            let level = preset.level(&rules(), 96, 64);
            let w = level.generate(rules()).unwrap();
            let names: &[&str] = match preset {
                Preset::DryForest => &["stone", "tree", "fire"],
                Preset::Island => &["stone", "tree", "water", "sand"],
                Preset::SandstormValley => &["stone", "sand", "smoke"],
            };
            for name in names {
                assert!(
                    w.cells.iter().any(|c| Some(c.element) == w.rules.id(name)),
                    "{} lacks {name}",
                    preset.label()
                );
            }
            if preset == Preset::DryForest {
                let fire = w.rules.id("fire").unwrap();
                let fires = w.cells.iter().filter(|cell| cell.element == fire).count();
                assert!(
                    (1..=12).contains(&fires),
                    "spark should be a small west fire, got {fires}"
                );
            }
            preset.level(&rules(), 16, 16).generate(rules()).unwrap();
        }
    }
    #[test]
    fn full_width_seeds_round_trip_and_numeric_seeds_load() {
        let mut level = Preset::Island.level(&rules(), 96, 64);
        level.seed = u64::MAX;
        assert_eq!(Level::load(&level.to_toml().unwrap()).unwrap(), level);
        level.seed = 29;
        let numeric = level
            .to_toml()
            .unwrap()
            .replace("seed = \"29\"", "seed = 29");
        assert_eq!(Level::load(&numeric).unwrap(), level);
    }
    #[test]
    fn older_recipes_default_to_calm_and_invalid_wind_fails() {
        let old =
            "version=1\nwidth=32\nheight=24\nseed=7\ngenerator='hills'\n[amounts]\nstone=100\n";
        let legacy = Level::load(old).unwrap();
        assert_eq!(legacy.wind, Wind::default());
        assert_eq!(
            (legacy.ambient_temperature, legacy.ambient_humidity),
            (20, 45)
        );
        let bad = format!("{old}\n[wind]\ndirection='east'\nstrength=5\n");
        assert!(
            Level::load(&bad)
                .unwrap_err()
                .to_string()
                .contains("Wind strength")
        );
        assert!(
            Level::load(&bad.replace("east", "north"))
                .unwrap_err()
                .to_string()
                .contains("north")
        );
    }
    #[test]
    fn invalid_files_and_unknown_elements_fail_clearly() {
        assert!(
            Level::load("broken")
                .unwrap_err()
                .to_string()
                .contains("TOML")
        );
        let mut level = Preset::Island.level(&rules(), 96, 64);
        level.version = 2;
        assert!(
            level
                .validate()
                .unwrap_err()
                .to_string()
                .contains("version")
        );
        level.version = 1;
        level.width = usize::MAX;
        assert!(level.validate().is_err());
        level.width = 2048;
        level.height = 2048;
        assert!(level.validate().is_err());
        level.width = 96;
        level.height = 64;
        level.amounts.insert("tree".into(), 101);
        assert!(level.validate().unwrap_err().to_string().contains("tree"));
        level.amounts.remove("tree");
        level.amounts.insert("missing".into(), 1);
        assert!(
            level
                .generate(rules())
                .err()
                .unwrap()
                .to_string()
                .contains("unknown element 'missing'")
        );
    }
    #[test]
    fn added_elements_get_generic_amounts_and_generate_without_ui_changes() {
        let mut rules = rules();
        let mut custom = rules.elements[rules.id("sand").unwrap()].clone();
        custom.name = "confetti".into();
        rules.elements.push(custom);
        let mut level = Preset::Island.level(&rules, 96, 64);
        assert_eq!(level.amounts.get("confetti"), Some(&0));
        level.amounts.insert("confetti".into(), 100);
        let w = level.generate(rules).unwrap();
        assert!(
            w.cells
                .iter()
                .any(|c| Some(c.element) == w.rules.id("confetti"))
        );
    }
}
