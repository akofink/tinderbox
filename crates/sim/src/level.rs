//! Human-readable, seeded world recipes. No clocks, filesystem, or graphics.
use crate::{Motion, Rules, Wind, WindDirection, World, world::MAX_EDGE_WATER_COLUMN};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

pub const MAX_LEVEL_CELLS: usize = 1_048_576;
pub const MAX_LEVEL_DIMENSION: usize = 2048;
/// Horizontal cell width of independently generated endless-world regions.
pub const ENDLESS_CHUNK_WIDTH: usize = 128;

/// A seed-generated strip addressed by its signed horizontal chunk coordinate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedChunk {
    pub index: i64,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<crate::Cell>,
}
impl GeneratedChunk {
    pub fn cell(&self, x: usize, y: usize) -> Option<crate::Cell> {
        (x < self.width && y < self.height).then(|| self.cells[y * self.width + x])
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LevelMode {
    #[default]
    Fixed,
    Endless,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Generator {
    Empty,
    #[default]
    Hills,
    Island,
    Valley,
    BoatLake,
    DensityLab,
}
impl Generator {
    pub const ALL: [Self; 4] = [Self::Empty, Self::Hills, Self::Island, Self::Valley];
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "Empty canvas",
            Self::Hills => "Rolling hills",
            Self::Island => "Island",
            Self::Valley => "Valley",
            Self::BoatLake => "Boat lake",
            Self::DensityLab => "Density lab",
        }
    }
}

fn is_fit_dimension(value: &usize) -> bool {
    *value == 0
}

fn default_ambient_temperature() -> i16 {
    20
}
fn default_ambient_humidity() -> u8 {
    45
}
fn default_ambient_oxygen() -> u8 {
    21
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub version: u32,
    #[serde(default, skip_serializing_if = "is_fit_dimension")]
    pub width: usize,
    #[serde(default, skip_serializing_if = "is_fit_dimension")]
    pub height: usize,
    #[serde(skip)]
    pub fit_window: bool,
    // TOML integers are signed 64-bit. A decimal string preserves every u64 seed.
    #[serde(with = "seed_format")]
    pub seed: u64,
    pub generator: Generator,
    #[serde(default)]
    pub mode: LevelMode,
    #[serde(default)]
    pub wind: Wind,
    #[serde(default = "default_ambient_temperature")]
    pub ambient_temperature: i16,
    #[serde(default = "default_ambient_humidity")]
    pub ambient_humidity: u8,
    #[serde(default = "default_ambient_oxygen")]
    pub ambient_oxygen: u8,
    /// Density (0..=100) within each material's terrain region, keyed by name.
    pub amounts: BTreeMap<String, u8>,
    /// A small upwind fire, instead of scattering fire through the whole canopy.
    #[serde(default)]
    pub spark: bool,
    /// Include a simple wooden boat over a small water patch.
    #[serde(default)]
    pub boat: bool,
    /// Number of sand cells stacked as cargo on the boat (0..=8).
    #[serde(default)]
    pub boat_sand: u8,
    /// Start a fire in a wooden hull plank.
    #[serde(default)]
    pub boat_burning: bool,
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
    BoatLake,
    DensityLab,
}
impl Preset {
    pub const ALL: [Self; 5] = [
        Self::DryForest,
        Self::Island,
        Self::SandstormValley,
        Self::BoatLake,
        Self::DensityLab,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::DryForest => "Dry forest, east wind",
            Self::Island => "Island",
            Self::SandstormValley => "Sandstorm valley",
            Self::BoatLake => "Boat lake",
            Self::DensityLab => "Density lab",
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
            Self::BoatLake => (Generator::BoatLake, 241, &[("stone", 100), ("water", 100)]),
            Self::DensityLab => (
                Generator::DensityLab,
                307,
                &[
                    ("stone", 100),
                    ("water", 100),
                    ("sand", 100),
                    ("ice", 100),
                    ("oil", 100),
                ],
            ),
        };
        let mut level = Level {
            version: 1,
            width,
            height,
            fit_window: false,
            seed,
            generator,
            mode: LevelMode::Fixed,
            ambient_temperature: if self == Self::DensityLab { 1 } else { 20 },
            ambient_humidity: 45,
            ambient_oxygen: 21,
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
                Self::BoatLake => Wind {
                    direction: WindDirection::East,
                    strength: 1,
                },
                Self::DensityLab => Wind::default(),
            },
            amounts: BTreeMap::new(),
            spark: matches!(self, Self::DryForest),
            boat: false,
            boat_sand: 0,
            boat_burning: false,
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
    /// Generate an initial resident span of deterministic chunks for endless play.
    /// The viewport-sized recipe width determines the number of preloaded chunks.
    pub fn generate_endless_region(&self, rules: &Rules) -> Result<World, LevelError> {
        self.validate()?;
        let chunks = self.width.div_ceil(ENDLESS_CHUNK_WIDTH).saturating_add(4);
        let start = -(chunks as i64 / 2);
        let mut world = self.generate_endless_region_at(start, chunks, rules)?;
        if self.spark {
            place_spark(&mut world);
        }
        if self.boat {
            self.place_recipe_boat(&mut world)?;
        }
        world.mark_all_dirty();
        Ok(world)
    }

    /// Generate a contiguous resident span at an explicit signed chunk origin.
    pub fn generate_endless_region_at(
        &self,
        start_chunk: i64,
        chunks: usize,
        rules: &Rules,
    ) -> Result<World, LevelError> {
        if chunks == 0 {
            return Err(LevelError(
                "Endless region must contain at least one chunk".into(),
            ));
        }
        let width = chunks
            .checked_mul(ENDLESS_CHUNK_WIDTH)
            .ok_or_else(|| LevelError("Endless region width overflow".into()))?;
        let mut world = World::new(width, self.height, self.seed, rules.clone())
            .map_err(|error| LevelError(error.to_string()))?;
        world.set_wind(self.wind.direction, self.wind.strength);
        world.set_ambient(
            self.ambient_temperature,
            self.ambient_humidity,
            self.ambient_oxygen,
        );
        for offset in 0..chunks {
            let chunk = self.generate_chunk(start_chunk + offset as i64, rules)?;
            for y in 0..self.height {
                let destination = y * width + offset * ENDLESS_CHUNK_WIDTH;
                let source = y * ENDLESS_CHUNK_WIDTH;
                world.cells[destination..destination + ENDLESS_CHUNK_WIDTH]
                    .copy_from_slice(&chunk.cells[source..source + ENDLESS_CHUNK_WIDTH]);
            }
        }
        if let Some(tree) = world.rules.id("tree") {
            for (index, cell) in world.cells.iter().enumerate() {
                if cell.element == tree {
                    world.tree_support_age[index] = (index % 64) as u8;
                }
            }
        }
        world
            .enable_chunk_span(start_chunk, ENDLESS_CHUNK_WIDTH)
            .map_err(|error| LevelError(error.to_string()))?;
        world.mark_all_dirty();
        Ok(world)
    }

    /// Generate one endless terrain chunk. Content is a pure function of the level seed,
    /// signed chunk index, and cell coordinate, so generation order cannot change the result.
    pub fn generate_chunk(&self, index: i64, rules: &Rules) -> Result<GeneratedChunk, LevelError> {
        self.validate()?;
        if self.height < 16 || self.height > MAX_LEVEL_DIMENSION {
            return Err(LevelError(format!(
                "Endless world height must be 16..={MAX_LEVEL_DIMENSION}"
            )));
        }
        for name in self.amounts.keys() {
            if rules.id(name).is_none() {
                return Err(LevelError(format!(
                    "Level refers to unknown element '{name}'"
                )));
            }
        }
        let width = ENDLESS_CHUNK_WIDTH;
        let height = self.height;
        let air = rules
            .id("air")
            .ok_or_else(|| LevelError("Ruleset must define 'air'".into()))?;
        let mut cells = vec![
            crate::Cell {
                element: air,
                ..crate::Cell::default()
            };
            width * height
        ];
        if self.generator == Generator::Empty {
            return Ok(GeneratedChunk {
                index,
                width,
                height,
                cells,
            });
        }
        let waterline = height * 3 / 4;
        let water_amount = self.amounts.get("water").copied().unwrap_or(0);
        let columns: Vec<_> = (0..width)
            .map(|local_x| {
                let world_x = index
                    .wrapping_mul(width as i64)
                    .wrapping_add(local_x as i64);
                let ground = endless_surface(self.seed, world_x, height, self.generator);
                let column = world_x.rem_euclid(4) as usize;
                let canopy = (height / 12
                    + (sample_signed(self.seed ^ name_hash("tree"), world_x.div_euclid(4), 1)
                        % (height / 8).max(1) as u64) as usize)
                    .saturating_sub(column.abs_diff(1))
                    .max(2);
                let flooded =
                    ground > waterline && water_column_selected(self.seed, world_x, water_amount);
                (world_x, ground, canopy, flooded)
            })
            .collect();
        let mut materials: Vec<_> = rules
            .elements
            .iter()
            .enumerate()
            .filter_map(|(id, element)| {
                let amount = self.amounts.get(&element.name).copied().unwrap_or(0);
                (amount > 0).then_some((layer(&element.name, element.motion), id, element, amount))
            })
            .collect();
        materials.sort_by(|a, b| (a.0, &a.2.name).cmp(&(b.0, &b.2.name)));
        for (_, id, element, amount) in materials {
            let salt = name_hash(&element.name);
            for (local_x, &(world_x, ground, canopy, flooded)) in columns.iter().enumerate() {
                let column = world_x.rem_euclid(4) as usize;
                let group_accepts = match element.name.as_str() {
                    "tree" => {
                        let group_x = world_x.div_euclid(4);
                        sample_signed(self.seed ^ salt, group_x, 0) % 100 < u64::from(amount)
                    }
                    "ash" => {
                        let group_x = world_x.div_euclid(8);
                        sample_signed(self.seed ^ salt, group_x, 0) % 100 < u64::from(amount)
                    }
                    _ => true,
                };
                if !group_accepts {
                    continue;
                }
                let tree_age = (element.name == "tree" && element.lifetime > 0)
                    .then(|| {
                        let age_span = u64::from(element.lifetime) * 98 / 100;
                        (age_span > 0).then(|| {
                            (sample_signed(
                                self.seed ^ name_hash("tree_age_phase"),
                                world_x.div_euclid(4),
                                0,
                            ) % age_span) as u16
                        })
                    })
                    .flatten();
                for y in 0..height {
                    let eligible = match element.name.as_str() {
                        "stone" => y >= ground,
                        "soil" => y >= ground && y < ground + (height / 32).max(2),
                        "tree" => {
                            let above = ground.saturating_sub(y);
                            !flooded
                                && y < ground
                                && ((column == 1 && above <= canopy)
                                    || (column <= 2 && above > 1 && above <= canopy / 2))
                        }
                        "plant" | "grass" => !flooded && y < ground && y + 3 >= ground,
                        "water" => y >= waterline && y < ground,
                        "sand" | "ash" => y >= ground && y < ground + (height / 16).max(2),
                        "fire" => !flooded && y + (canopy / 2).max(1) == ground,
                        _ => match element.motion {
                            Motion::Static => y < ground && y + (height / 16).max(2) >= ground,
                            Motion::Fall => y >= ground && y < ground + (height / 24).max(2),
                            Motion::Liquid => y >= waterline && y < ground,
                            Motion::Gas => y < ground && y >= height / 3 && y < height / 2,
                        },
                    };
                    let density_matches = match element.name.as_str() {
                        "tree" | "ash" => true,
                        "sand" => {
                            coherent_noise_2d(self.seed ^ salt, world_x, y as i64, 12, 6)
                                < u64::from(amount) * NOISE_SCALE / 100
                        }
                        "water" => flooded,
                        _ => {
                            sample_signed(self.seed ^ salt, world_x, y as i64) % 100
                                < u64::from(amount)
                        }
                    };
                    if eligible && density_matches {
                        let cell = &mut cells[y * width + local_x];
                        cell.element = id;
                        if let Some(tree_age) = tree_age {
                            cell.age = tree_age;
                        }
                    }
                }
            }
        }
        Ok(GeneratedChunk {
            index,
            width,
            height,
            cells,
        })
    }

    /// New ruleset elements automatically get an amount control; air is implicit.
    pub fn add_missing_elements(&mut self, rules: &Rules) {
        for element in &rules.elements {
            if element.name != "air" {
                self.amounts.entry(element.name.clone()).or_default();
            }
        }
    }
    pub fn load(text: &str) -> Result<Self, LevelError> {
        let mut level: Self =
            toml::from_str(text).map_err(|e| LevelError(format!("Invalid level TOML: {e}")))?;
        level.fit_window = level.width == 0 && level.height == 0;
        level.validate()?;
        Ok(level)
    }
    pub fn to_toml(&self) -> Result<String, LevelError> {
        self.validate()?;
        let mut serializable = self.clone();
        if serializable.fit_window {
            serializable.width = 0;
            serializable.height = 0;
        }
        toml::to_string_pretty(&serializable)
            .map_err(|e| LevelError(format!("Cannot encode level: {e}")))
    }
    pub fn validate(&self) -> Result<(), LevelError> {
        if self.version != 1 {
            return Err(LevelError(format!(
                "Unsupported level version {}; expected 1",
                self.version
            )));
        }
        let dimensions_omitted = self.width == 0 && self.height == 0;
        if (self.width == 0 || self.height == 0) && !dimensions_omitted {
            return Err(LevelError(
                "Width and height must both be specified, or both omitted to fit the window".into(),
            ));
        }
        if !dimensions_omitted
            && (self.width < 16
                || self.height < 16
                || self.width > MAX_LEVEL_DIMENSION
                || self.height > MAX_LEVEL_DIMENSION
                || self
                    .width
                    .checked_mul(self.height)
                    .is_none_or(|n| n > MAX_LEVEL_CELLS))
        {
            return Err(LevelError(format!(
                "World must be 16..={MAX_LEVEL_DIMENSION} cells per side and at most {MAX_LEVEL_CELLS} cells total"
            )));
        }
        if self.wind.strength > 4 {
            return Err(LevelError("Wind strength must be 0..=4".into()));
        }
        if self.boat_sand > 8 {
            return Err(LevelError("Boat sand cargo must be 0..=8 cells".into()));
        }
        if !self.boat && (self.boat_sand > 0 || self.boat_burning) {
            return Err(LevelError(
                "Boat cargo and fire options require the boat option".into(),
            ));
        }
        if !(-40..=60).contains(&self.ambient_temperature)
            || self.ambient_humidity > 100
            || self.ambient_oxygen > 100
        {
            return Err(LevelError(
                "Ambient temperature must be -40..=60 C, humidity and oxygen 0..=100%".into(),
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
        if self.width == 0 || self.height == 0 {
            return Err(LevelError(
                "A fit-window recipe needs canvas dimensions before generation".into(),
            ));
        }
        for name in self.amounts.keys() {
            if rules.id(name).is_none() {
                return Err(LevelError(format!(
                    "Level refers to unknown element '{name}'"
                )));
            }
        }
        if self.mode == LevelMode::Endless {
            return self.generate_endless_region(&rules);
        }
        let mut world = World::new(self.width, self.height, self.seed, rules.clone())
            .map_err(|e| LevelError(e.to_string()))?;
        world.set_wind(self.wind.direction, self.wind.strength);
        world.set_ambient(
            self.ambient_temperature,
            self.ambient_humidity,
            self.ambient_oxygen,
        );
        // Showcase scenes use exact, sealed geometry so the demonstrations remain legible.
        if matches!(self.generator, Generator::BoatLake | Generator::DensityLab) {
            self.generate_showcase(&mut world)?;
            world.mark_all_dirty();
            return Ok(world);
        }
        // Empty means a blank canvas regardless of amounts, not floating terrain.
        if self.generator == Generator::Empty {
            if self.boat {
                self.place_recipe_boat(&mut world)?;
            }
            world.mark_all_dirty();
            return Ok(world);
        }
        let surface: Vec<usize> = (0..self.width).map(|x| self.surface(x)).collect();
        let waterline = self.height * 3 / 4;
        let water_amount = self.amounts.get("water").copied().unwrap_or(0);
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
                    ground > waterline && water_column_selected(self.seed, x as i64, water_amount);
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
                        "water" => {
                            let edge = x == 0 || x + 1 == self.width;
                            flooded
                                && y >= waterline
                                && y < ground
                                && (!edge || y >= ground.saturating_sub(MAX_EDGE_WATER_COLUMN))
                        }
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
                    let density_matches = match element.name.as_str() {
                        "tree" => sample(self.seed ^ salt, x / 4, 0) % 100 < u64::from(amount),
                        "sand" => {
                            coherent_noise_2d(self.seed ^ salt, x as i64, y as i64, 12, 6)
                                < u64::from(amount) * NOISE_SCALE / 100
                        }
                        "water" => flooded,
                        _ => {
                            sample_signed(self.seed ^ salt, x as i64, y as i64) % 100
                                < u64::from(amount)
                        }
                    };
                    if eligible && density_matches {
                        let index = y * self.width + x;
                        world.cells[index].element = id;
                        world.cells[index].age = 0;
                        if element.name == "tree" {
                            world.tree_support_age[index] = (index % 64) as u8;
                        }
                        if element.name == "tree" && element.lifetime > 0 {
                            // A four-column forest cohort gets one deterministic lifecycle phase.
                            // This keeps generated tree death staggered without runtime randomness.
                            let age_span = u64::from(element.lifetime) * 98 / 100;
                            if age_span > 0 {
                                world.cells[index].age =
                                    (sample(self.seed ^ name_hash("tree_age_phase"), x / 4, 0)
                                        % age_span) as u16;
                            }
                        }
                    }
                }
            }
        }
        if self.boat {
            self.place_recipe_boat(&mut world)?;
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
    fn generate_showcase(&self, world: &mut World) -> Result<(), LevelError> {
        let stone = world
            .rules()
            .id("stone")
            .ok_or_else(|| LevelError("Showcase requires stone".into()))?;
        let water = world
            .rules()
            .id("water")
            .ok_or_else(|| LevelError("Showcase requires water".into()))?;
        let width = world.width();
        let height = world.height();
        let left = width / 10;
        let right = width - left;
        let floor = height * 4 / 5;
        let waterline = height * 3 / 5;
        for x in 0..left {
            let distance = left - x;
            let top = waterline
                .saturating_sub(height / 12 + distance * height / width / 4)
                .max(height / 3);
            for y in top..height {
                world.paint(x, y, stone);
            }
            if let Some(soil) = world.rules().id("soil") {
                world.paint(x, top, soil);
            }
        }
        for x in right..width {
            let distance = x - right + 1;
            let top = waterline
                .saturating_sub(height / 12 + distance * height / width / 4)
                .max(height / 3);
            for y in top..height {
                world.paint(x, y, stone);
            }
            if let Some(soil) = world.rules().id("soil") {
                world.paint(x, top, soil);
            }
        }
        for x in left..right {
            world.paint(x, floor, stone);
        }
        for y in waterline..floor {
            world.paint(left, y, stone);
            world.paint(right - 1, y, stone);
            for x in left + 1..right - 1 {
                world.paint(x, y, water);
            }
        }
        if self.generator == Generator::BoatLake {
            let centers = if width >= 48 {
                vec![width / 5, width * 2 / 5, width * 3 / 5, width * 4 / 5]
            } else {
                vec![width / 2]
            };
            for (index, x) in centers.into_iter().enumerate() {
                if x < 5 || x + 5 >= width {
                    continue;
                }
                let y = waterline - 1;
                if !world.place_boat(x, y) {
                    return Err(LevelError("Boat lake could not place its boats".into()));
                }
                if index == 1 || index == 2 {
                    let sand = world
                        .rules()
                        .id("sand")
                        .ok_or_else(|| LevelError("Boat lake requires sand".into()))?;
                    let cargo_count = if index == 1 { 1 } else { 8 };
                    for cargo in 0..cargo_count {
                        let dx = cargo % 4;
                        world.paint(x + dx - 2, y - 3 - cargo / 4, sand);
                    }
                } else if index == 3 {
                    let fire = world
                        .rules()
                        .id("fire")
                        .ok_or_else(|| LevelError("Boat lake requires fire".into()))?;
                    for dx in [-3isize, -1, 1] {
                        world.paint(x.checked_add_signed(dx).unwrap(), y - 2, fire);
                    }
                }
            }
        } else {
            let sand = world
                .rules()
                .id("sand")
                .ok_or_else(|| LevelError("Density lab requires sand".into()))?;
            let ice = world
                .rules()
                .id("ice")
                .ok_or_else(|| LevelError("Density lab requires ice".into()))?;
            let oil = world
                .rules()
                .id("oil")
                .ok_or_else(|| LevelError("Density lab requires oil".into()))?;
            let oil_fire = world
                .rules()
                .id("oil_fire")
                .ok_or_else(|| LevelError("Density lab requires oil fire".into()))?;
            let fire = world
                .rules()
                .id("fire")
                .ok_or_else(|| LevelError("Density lab requires fire".into()))?;
            let steam = world
                .rules()
                .id("steam")
                .ok_or_else(|| LevelError("Density lab requires steam".into()))?;
            let third = (right - left) / 3;
            for y in waterline.saturating_sub(height / 10)..waterline {
                for x in left + third / 2..left + third {
                    world.paint(x, y, sand);
                }
            }
            for x in left + third..left + third + width / 12 {
                world.paint(x, waterline - 1, ice);
            }
            let oil_x = left + third * 2;
            for x in oil_x..oil_x + width / 12 {
                world.paint(x, waterline - 1, oil);
            }
            for dx in 0..4 {
                world.paint(oil_x + dx, waterline - 2, oil_fire);
            }
            let vent_x = left + third + width / 20;
            world.paint(vent_x, waterline + 2, fire);
            world.paint(vent_x + 1, waterline + 2, steam);
        }
        if self.boat {
            self.place_recipe_boat(world)?;
        }
        Ok(())
    }

    fn place_recipe_boat(&self, world: &mut World) -> Result<(), LevelError> {
        let y = self.height * 3 / 4 - 1;
        let x = (4..self.width - 4)
            .min_by_key(|x| x.abs_diff(self.width / 2))
            .filter(|x| {
                [(-2isize, 3isize), (-1, 4), (0, 3)]
                    .into_iter()
                    .all(|(dy, span)| {
                        let row = y.checked_add_signed(dy).unwrap_or(self.height);
                        (-span..=span).all(|dx| {
                            let column = x.checked_add_signed(dx).unwrap_or(self.width);
                            if row >= self.height || column >= self.width {
                                return false;
                            }
                            let name = &world.rules.elements
                                [world.cells[row * self.width + column].element]
                                .name;
                            matches!(name.as_str(), "air" | "water")
                        })
                    })
            })
            .ok_or_else(|| {
                LevelError("Boat recipe option needs a clear 9-by-3 area near the waterline".into())
            })?;
        let water = world
            .rules()
            .id("water")
            .ok_or_else(|| LevelError("Boat recipe option requires a water element".into()))?;
        for dy in 1..=3 {
            let row = y + dy;
            if row >= self.height {
                break;
            }
            for dx in -6isize..=6 {
                let Some(column) = x
                    .checked_add_signed(dx)
                    .filter(|column| *column < self.width)
                else {
                    continue;
                };
                let index = row * self.width + column;
                if world.cells[index].element == world.rules.id("air").unwrap_or(usize::MAX) {
                    world.paint(column, row, water);
                }
            }
        }
        if !world.place_boat(x, y) {
            return Err(LevelError(
                "Boat recipe option needs wood and a clear 9-by-3 area near the waterline".into(),
            ));
        }
        if self.boat_sand > 0 {
            let sand = world
                .rules()
                .id("sand")
                .ok_or_else(|| LevelError("Boat cargo option requires sand".into()))?;
            for cargo in 0..self.boat_sand {
                world.paint(x, y - 3 - usize::from(cargo), sand);
            }
        }
        if self.boat_burning {
            let fire = world
                .rules()
                .id("fire")
                .ok_or_else(|| LevelError("Burning boat option requires fire".into()))?;
            world.paint(x + 3, y - 2, fire);
        }
        Ok(())
    }

    /// Generates a resize target while blending newly exposed terrain into the
    /// existing right edge. Existing cells are overlaid separately afterward.
    pub fn generate_for_resize(&self, rules: Rules, previous: &World) -> Result<World, LevelError> {
        let mut world = self.generate(rules)?;
        if self.width <= previous.width() || self.generator == Generator::Empty {
            return Ok(world);
        }

        let edge = previous.width() - 1;
        let vertical_offset = self.height as isize - previous.height() as isize;
        let mut previous_surface = terrain_surface(previous, edge)
            .map(|surface| {
                (surface as isize + vertical_offset).clamp(0, self.height as isize - 1) as usize
            })
            .unwrap_or_else(|| self.surface(edge).min(self.height - 1));
        let extension = self.width - previous.width();
        let air = world.rules().id("air").unwrap_or(0);
        for x in previous.width()..self.width {
            let generated_surface = self.surface(x).min(self.height - 1);
            let step = x - previous.width() + 1;
            let blended = previous_surface as isize
                + (generated_surface as isize - previous_surface as isize) * step as isize
                    / (extension + 1) as isize;
            let desired = blended.clamp(
                previous_surface.saturating_sub(2) as isize,
                previous_surface.saturating_add(2).min(self.height - 1) as isize,
            ) as usize;
            shift_column(&mut world, x, generated_surface, desired, air);
            previous_surface = desired;
        }
        world.mark_all_dirty();
        Ok(world)
    }

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
            Generator::BoatLake | Generator::DensityLab => self.height * 4 / 5,
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

fn terrain_surface(world: &World, x: usize) -> Option<usize> {
    let ground_ids: Vec<_> = ["stone", "soil", "sand", "ash"]
        .into_iter()
        .filter_map(|name| world.rules().id(name))
        .collect();
    (0..world.height())
        .find(|&y| ground_ids.contains(&world.cells()[y * world.width() + x].element))
}

fn shift_column(
    world: &mut World,
    x: usize,
    source_surface: usize,
    target_surface: usize,
    air: usize,
) {
    let height = world.height();
    let shift = target_surface as isize - source_surface as isize;
    let source: Vec<_> = (0..height)
        .map(|y| world.cells[y * world.width + x])
        .collect();
    for y in 0..height {
        let source_y = y as isize - shift;
        world.cells[y * world.width + x] = if source_y < 0 {
            crate::Cell {
                element: air,
                age: 0,
                moisture: 0,
                burn_progress: 0,
                body_id: 0,
            }
        } else if source_y >= height as isize {
            source[height - 1]
        } else {
            source[source_y as usize]
        };
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
const NOISE_SCALE: u64 = 1024;
const WATER_POOL_SPAN: i64 = 32;

fn name_hash(name: &str) -> u64 {
    name.bytes().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}

fn water_column_selected(seed: u64, x: i64, amount: u8) -> bool {
    if amount == 0 {
        return false;
    }
    coherent_noise_1d(seed ^ name_hash("water"), x, WATER_POOL_SPAN)
        < u64::from(amount) * NOISE_SCALE / 100
}

fn coherent_noise_1d(seed: u64, x: i64, span: i64) -> u64 {
    let section = x.div_euclid(span);
    let offset = x.rem_euclid(span);
    let weight = smooth_weight(offset, span);
    let a = sample_signed(seed, section, 0) % NOISE_SCALE;
    let b = sample_signed(seed, section.wrapping_add(1), 0) % NOISE_SCALE;
    interpolate(a, b, weight)
}

fn coherent_noise_2d(seed: u64, x: i64, y: i64, span_x: i64, span_y: i64) -> u64 {
    let section_x = x.div_euclid(span_x);
    let section_y = y.div_euclid(span_y);
    let weight_x = smooth_weight(x.rem_euclid(span_x), span_x);
    let weight_y = smooth_weight(y.rem_euclid(span_y), span_y);
    let top = interpolate(
        sample_signed(seed, section_x, section_y) % NOISE_SCALE,
        sample_signed(seed, section_x.wrapping_add(1), section_y) % NOISE_SCALE,
        weight_x,
    );
    let bottom = interpolate(
        sample_signed(seed, section_x, section_y.wrapping_add(1)) % NOISE_SCALE,
        sample_signed(seed, section_x.wrapping_add(1), section_y.wrapping_add(1)) % NOISE_SCALE,
        weight_x,
    );
    interpolate(top, bottom, weight_y)
}

fn smooth_weight(offset: i64, span: i64) -> u64 {
    let t = offset as u64 * NOISE_SCALE / span as u64;
    let squared = t * t / NOISE_SCALE;
    squared * (3 * NOISE_SCALE - 2 * t) / NOISE_SCALE
}

fn interpolate(a: u64, b: u64, weight: u64) -> u64 {
    (a * (NOISE_SCALE - weight) + b * weight) / NOISE_SCALE
}

fn endless_surface(seed: u64, x: i64, height: usize, generator: Generator) -> usize {
    let span = 96i64;
    let section = x.div_euclid(span);
    let offset = x.rem_euclid(span) as usize;
    let a = sample_signed(seed ^ 913, section, 0) % 1024;
    let b = sample_signed(seed ^ 913, section.wrapping_add(1), 0) % 1024;
    let noise = (a as usize * (span as usize - offset) + b as usize * offset) / span as usize;
    let phase = x.rem_euclid(1024) as usize;
    let distance = phase.abs_diff(512);
    let hill_span = 256i64;
    let hill_section = x.div_euclid(hill_span);
    let hill_offset = x.rem_euclid(hill_span) as usize;
    let hill_a = sample_signed(seed ^ 23, hill_section, 0) % 1024;
    let hill_b = sample_signed(seed ^ 23, hill_section.wrapping_add(1), 0) % 1024;
    let hills = (hill_a as usize * (hill_span as usize - hill_offset)
        + hill_b as usize * hill_offset)
        / hill_span as usize;
    let base = match generator {
        Generator::Hills => height * 2 / 3 + hills * height / 1024 / 6,
        Generator::Island => height / 3 + distance * height / 1024,
        Generator::Valley => height / 2 - distance * height / 3072,
        Generator::Empty => height,
        Generator::BoatLake | Generator::DensityLab => height * 4 / 5,
    };
    (base + noise * height / 1024 / 12).min(height - 1)
}

fn sample_signed(seed: u64, x: i64, y: i64) -> u64 {
    let mut n = seed
        .wrapping_add((x as u64).wrapping_mul(0x9e3779b97f4a7c15))
        .wrapping_add((y as u64).wrapping_mul(0xd1b54a32d192ed03));
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}
fn sample(seed: u64, x: usize, y: usize) -> u64 {
    sample_signed(seed, x as i64, y as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_RULES;
    fn rules() -> Rules {
        Rules::load(DEFAULT_RULES).unwrap()
    }

    #[test]
    fn endless_sand_forms_irregular_connected_lenses() {
        let rules = rules();
        let mut level = Preset::DryForest.level(&rules, 128, 240);
        level.amounts.clear();
        level.amounts.insert("stone".into(), 100);
        level.amounts.insert("soil".into(), 100);
        level.amounts.insert("sand".into(), 35);
        let chunk = level.generate_chunk(-1, &rules).unwrap();
        let sand = rules.id("sand").unwrap();
        let layer_depth = (chunk.height / 16).max(2);
        let mut sand_columns = 0;
        let mut partial_columns = 0;
        let mut connected_edges = 0;
        for x in 0..chunk.width {
            let ground =
                endless_surface(level.seed, -128 + x as i64, chunk.height, level.generator);
            let count = (ground..ground + layer_depth)
                .filter(|&y| chunk.cell(x, y).unwrap().element == sand)
                .count();
            sand_columns += usize::from(count > 0);
            partial_columns += usize::from(count > 0 && count < layer_depth);
            if x + 1 < chunk.width {
                connected_edges += (ground..ground + layer_depth)
                    .filter(|&y| {
                        chunk.cell(x, y).unwrap().element == sand
                            && chunk.cell(x + 1, y).unwrap().element == sand
                    })
                    .count();
            }
        }
        assert!(sand_columns > 0, "the seeded chunk should contain sand");
        assert!(
            sand_columns < chunk.width,
            "lenses should leave clear columns"
        );
        assert!(
            partial_columns > 0,
            "lenses should have uneven outlines, not slabs"
        );
        assert!(
            connected_edges > 0,
            "lenses should include connected neighboring cells"
        );
    }

    #[test]
    fn endless_hills_keep_forest_clearings_around_settled_pools() {
        let rules = rules();
        let mut level = Preset::DryForest.level(&rules, 1024, 240);
        level.mode = LevelMode::Endless;
        level.amounts.clear();
        level.amounts.insert("stone".into(), 100);
        level.amounts.insert("soil".into(), 100);
        level.amounts.insert("tree".into(), 65);
        level.amounts.insert("water".into(), 45);
        let chunk = level.generate_chunk(-2, &rules).unwrap();
        let tree = rules.id("tree").unwrap();
        let water = rules.id("water").unwrap();
        let tree_columns = (0..chunk.width)
            .filter(|&x| (0..chunk.height).any(|y| chunk.cell(x, y).unwrap().element == tree))
            .count();
        let waterline = chunk.height * 3 / 4;
        let water_columns = (0..chunk.width)
            .filter(|&x| chunk.cell(x, waterline).unwrap().element == water)
            .count();
        let mut longest_pool = 0;
        let mut current_pool = 0;
        for x in 0..chunk.width {
            if chunk.cell(x, waterline).unwrap().element == water {
                current_pool += 1;
            } else {
                longest_pool = longest_pool.max(current_pool);
                current_pool = 0;
            }
        }
        longest_pool = longest_pool.max(current_pool);
        assert!(tree_columns >= 20, "chunk -2 should contain forest clumps");
        assert!(
            tree_columns < chunk.width,
            "forest clumps should leave clearings"
        );
        assert!(water_columns > 0, "the seeded basin should contain a pool");
        assert!(
            longest_pool >= 8,
            "water should form a lake, not isolated columns"
        );
        for x in 0..chunk.width {
            let world_x = -2 * ENDLESS_CHUNK_WIDTH as i64 + x as i64;
            let ground = endless_surface(level.seed, world_x, chunk.height, level.generator);
            let selected = water_column_selected(level.seed, world_x, 45);
            for y in waterline..ground {
                assert_eq!(
                    chunk.cell(x, y).unwrap().element == water,
                    selected,
                    "water column at world x={world_x} should be fully settled"
                );
            }
        }
    }

    #[test]
    fn endless_trees_have_solid_trunks_and_reach_stable_support() {
        let mut rules = rules();
        let tree = rules.id("tree").unwrap();
        rules.elements[tree].lifetime = 0;
        let mut level = Preset::DryForest.level(&rules, 128, 96);
        level.amounts.clear();
        level.amounts.insert("stone".into(), 100);
        level.amounts.insert("soil".into(), 100);
        level.amounts.insert("tree".into(), 100);
        let mut world = level.generate_endless_region_at(0, 1, &rules).unwrap();
        let initial_trees = world
            .cells()
            .iter()
            .filter(|cell| cell.element == tree)
            .count();
        assert!(initial_trees > 0);
        for x in (0..world.width()).filter(|x| x % 4 == 1) {
            let ground = endless_surface(level.seed, x as i64, world.height(), level.generator);
            let canopy = (world.height() / 12
                + (sample_signed(level.seed ^ name_hash("tree"), x as i64 / 4, 1)
                    % (world.height() / 8).max(1) as u64) as usize)
                .max(2);
            for y in ground.saturating_sub(canopy)..ground {
                assert_eq!(world.cells()[y * world.width() + x].element, tree);
            }
        }
        for _ in 0..64 {
            world.step(100_000);
        }
        assert_eq!(
            world
                .cells()
                .iter()
                .filter(|cell| cell.element == tree)
                .count(),
            initial_trees,
            "generated trees should not collapse or become deadwood"
        );
        assert!(
            world
                .tree_support_age
                .iter()
                .enumerate()
                .filter(|(index, _)| world.cells()[*index].element == tree)
                .all(|(_, age)| *age == u8::MAX)
        );
    }

    #[test]
    fn endless_chunks_are_seed_and_coordinate_deterministic() {
        let rules = rules();
        let level = Preset::DryForest.level(&rules, 128, 96);
        let east = level.generate_chunk(8, &rules).unwrap();
        let west = level.generate_chunk(-3, &rules).unwrap();
        assert_eq!(level.generate_chunk(8, &rules).unwrap(), east);
        assert_eq!(level.generate_chunk(-3, &rules).unwrap(), west);
        let mut changed_seed = level.clone();
        changed_seed.seed = changed_seed.seed.wrapping_add(1);
        assert_ne!(changed_seed.generate_chunk(8, &rules).unwrap(), east);
        assert_eq!(east.cells.len(), ENDLESS_CHUNK_WIDTH * east.height);
    }

    #[test]
    fn coherent_material_fields_match_the_contiguous_chunk_region() {
        let rules = rules();
        let mut level = Preset::DryForest.level(&rules, 1024, 240);
        level.amounts.clear();
        level.amounts.insert("stone".into(), 100);
        level.amounts.insert("soil".into(), 100);
        level.amounts.insert("tree".into(), 65);
        level.amounts.insert("sand".into(), 35);
        level.amounts.insert("water".into(), 45);
        let region = level.generate_endless_region_at(-1, 2, &rules).unwrap();
        for (offset, chunk_index) in [-1, 0].into_iter().enumerate() {
            let chunk = level.generate_chunk(chunk_index, &rules).unwrap();
            for y in 0..chunk.height {
                for x in 0..chunk.width {
                    assert_eq!(
                        chunk.cell(x, y).unwrap(),
                        region.cells()[y * region.width() + offset * chunk.width + x],
                        "chunk {chunk_index} differs from its contiguous-region cells at ({x},{y})"
                    );
                }
            }
        }
    }

    #[test]
    fn endless_mode_builds_a_resident_span_and_preserves_offset_state() {
        let rules = rules();
        let mut level = Preset::DryForest.level(&rules, 128, 64);
        level.mode = LevelMode::Endless;
        let mut original = level.generate(rules.clone()).unwrap();
        assert_eq!(original.width() % ENDLESS_CHUNK_WIDTH, 0);
        let sand = rules.id("sand").unwrap();
        original.paint(3, 5, sand);
        let old_width = original.width();
        let expanded = level
            .generate_endless_region_at(-3, old_width / ENDLESS_CHUNK_WIDTH + 1, &rules)
            .unwrap();
        let mut expanded = expanded;
        expanded.preserve_resize_state_from_offset(&original, ENDLESS_CHUNK_WIDTH);
        assert_eq!(
            expanded.cells()[5 * expanded.width() + ENDLESS_CHUNK_WIDTH + 3].element,
            sand
        );
        assert!(expanded.width() > old_width);
    }

    #[test]
    fn endless_extension_preserves_pending_texture_uploads() {
        let rules = rules();
        let mut level = Preset::DryForest.level(&rules, 128, 64);
        level.mode = LevelMode::Endless;
        let mut world = level.generate_endless_region(&rules).unwrap();
        let old_pending = world.pending_dirty_regions();
        let index = world.width() as i64 / ENDLESS_CHUNK_WIDTH as i64 - 2;
        let chunk = level.generate_chunk(index, &rules).unwrap();
        world.extend_horizontal(&chunk, false).unwrap();
        assert!(world.pending_dirty_regions() > old_pending);
    }

    #[test]
    fn adjacent_endless_chunks_join_with_continuous_terrain() {
        let rules = rules();
        let base_level = Preset::DryForest.level(&rules, 128, 96);
        let surface = |chunk: &GeneratedChunk, x: usize| {
            (0..chunk.height)
                .find(|&y| {
                    let id = chunk.cell(x, y).unwrap().element;
                    ["stone", "soil", "sand", "ash"]
                        .into_iter()
                        .filter_map(|name| rules.id(name))
                        .any(|ground| ground == id)
                })
                .unwrap()
        };
        for generator in [Generator::Hills, Generator::Island, Generator::Valley] {
            let mut level = base_level.clone();
            level.generator = generator;
            let left = level.generate_chunk(-1, &rules).unwrap();
            let right = level.generate_chunk(0, &rules).unwrap();
            let left_edge = surface(&left, left.width - 1);
            let right_edge = surface(&right, 0);
            assert!(
                left_edge.abs_diff(right_edge) <= 1,
                "{generator:?} terrain seam: {left_edge} vs {right_edge}"
            );
            assert_eq!(right.index, left.index + 1);
        }
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
    fn generated_water_never_forms_a_tall_column_at_world_edges() {
        let mut wet_hills = Preset::DryForest.level(&rules(), 400, 250);
        wet_hills.amounts.insert("water".into(), 100);
        let recipes = [Preset::Island.level(&rules(), 400, 250), wet_hills];
        for level in recipes {
            let world = level.generate(rules()).unwrap();
            let water = world.rules.id("water").unwrap();
            for x in [0, world.width - 1] {
                let column = (0..world.height)
                    .filter(|y| world.cells[y * world.width + x].element == water)
                    .count();
                assert!(
                    column <= MAX_EDGE_WATER_COLUMN,
                    "edge column x={x} contains {column} water cells"
                );
            }
            assert!(
                world
                    .cells
                    .iter()
                    .filter(|cell| cell.element == water)
                    .count()
                    <= world.width * world.height / 2,
                "water generation remains bounded"
            );
        }
    }

    #[test]
    fn fixed_hills_water_forms_level_pools_at_reasonable_volume() {
        let rules = rules();
        let mut level = Preset::DryForest.level(&rules, 320, 240);
        level.amounts.clear();
        level.amounts.insert("stone".into(), 100);
        level.amounts.insert("soil".into(), 100);
        level.amounts.insert("water".into(), 45);
        let world = level.generate(rules.clone()).unwrap();
        let water = rules.id("water").unwrap();
        let waterline = level.height * 3 / 4;
        let mut longest_pool = 0;
        let mut current_pool = 0;
        for x in 1..level.width - 1 {
            if world.cells()[waterline * world.width() + x].element == water {
                current_pool += 1;
            } else {
                longest_pool = longest_pool.max(current_pool);
                current_pool = 0;
            }
        }
        longest_pool = longest_pool.max(current_pool);
        assert!(
            longest_pool >= 6,
            "fixed Hills should generate visible pools"
        );
        for x in 1..level.width - 1 {
            let ground = level.surface(x);
            let selected = ground > waterline && water_column_selected(level.seed, x as i64, 45);
            for y in waterline..ground {
                assert_eq!(
                    world.cells()[y * world.width() + x].element == water,
                    selected,
                    "water column x={x} should be fully settled"
                );
            }
        }
        let water_cells = world
            .cells()
            .iter()
            .filter(|cell| cell.element == water)
            .count();
        assert!(water_cells > 0, "the seeded hills should contain pools");
        assert!(water_cells < world.width() * world.height() / 8);
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
    fn later_generated_materials_do_not_inherit_tree_lifecycle_age() {
        let mut level = Preset::DryForest.level(&rules(), 96, 64);
        level.seed = 20_261_013;
        level.amounts.insert("beetle".into(), 0);
        let trees = level.generate(rules()).unwrap();
        let tree = trees.rules.id("tree").unwrap();

        level.amounts.insert("beetle".into(), 100);
        let forest = level.generate(rules()).unwrap();
        let beetle = forest.rules.id("beetle").unwrap();
        let overlays: Vec<_> = forest
            .cells
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| {
                (cell.element == beetle && trees.cells[index].element == tree).then_some(index)
            })
            .collect();

        assert!(
            !overlays.is_empty(),
            "beetles should overlay some tree cells"
        );
        assert!(
            overlays.iter().all(|&index| forest.cells[index].age == 0),
            "a generated critter must start with its own age, not the tree's lifecycle phase"
        );
    }

    #[test]
    fn empty_level_recipe_can_include_a_deterministic_wooden_boat() {
        let mut level = Preset::Island.level(&rules(), 96, 64);
        level.generator = Generator::Empty;
        level.boat = true;
        level.boat_sand = 4;
        level.boat_burning = true;
        let encoded = level.to_toml().unwrap();
        assert_eq!(Level::load(&encoded).unwrap(), level);
        let first = level.generate(rules()).unwrap();
        let second = level.generate(rules()).unwrap();
        let wood = first.rules.id("wood").unwrap();
        let water = first.rules.id("water").unwrap();
        let sand = first.rules.id("sand").unwrap();
        let fire = first.rules.id("fire").unwrap();
        assert_eq!(first.cells, second.cells);
        assert_eq!(
            first
                .cells
                .iter()
                .filter(|cell| cell.element == wood)
                .count(),
            22
        );
        assert!(first.cells.iter().any(|cell| cell.element == water));
        assert!(first.cells.iter().any(|cell| cell.element == fire));
        assert_eq!(
            first
                .cells
                .iter()
                .filter(|cell| cell.element == sand && cell.body_id != 0)
                .count(),
            4
        );
        assert_eq!(
            first.cells.iter().filter(|cell| cell.body_id != 0).count(),
            26
        );
        let mut played_a = first;
        let mut played_b = second;
        for _ in 0..12 {
            played_a.step(1_024);
            played_b.step(1_024);
        }
        assert_eq!(played_a.cells, played_b.cells);
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
                Preset::BoatLake => &["stone", "water", "wood", "sand", "fire"],
                Preset::DensityLab => &["stone", "water", "sand", "ice", "oil", "fire"],
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
    fn showcase_presets_have_sealed_water_and_distinct_demonstrations() {
        let rules = rules();
        let mut boats = Preset::BoatLake
            .level(&rules, 96, 64)
            .generate(rules.clone())
            .unwrap();
        let stone = boats.rules.id("stone").unwrap();
        let sand = boats.rules.id("sand").unwrap();
        let fire = boats.rules.id("fire").unwrap();
        let wood = boats.rules.id("wood").unwrap();
        assert_eq!(
            boats
                .cells
                .iter()
                .filter(|cell| cell.body_id != 0)
                .map(|cell| cell.body_id)
                .max(),
            Some(4)
        );
        assert!(
            boats
                .cells
                .iter()
                .filter(|cell| cell.element == fire)
                .count()
                >= 3
        );
        for _ in 0..4 {
            boats.step(8_000);
        }
        let cargo_in = |body_id| {
            boats
                .cells
                .iter()
                .filter(|cell| cell.element == sand && cell.body_id == body_id)
                .count()
        };
        assert!(
            cargo_in(2) >= 1,
            "the lightly loaded boat should carry one sand cell"
        );
        assert!(
            cargo_in(3) >= 8,
            "the overloaded boat should carry eight sand cells"
        );
        let hull_top = |body_id| {
            boats
                .cells
                .iter()
                .enumerate()
                .filter_map(|(index, cell)| {
                    (cell.body_id == body_id && cell.element == wood).then_some(index / boats.width)
                })
                .min()
                .unwrap()
        };
        assert!(
            hull_top(2) < hull_top(3),
            "one sand cell should leave its boat higher than the overloaded hull"
        );
        let lab = Preset::DensityLab
            .level(&rules, 96, 64)
            .generate(rules)
            .unwrap();
        for name in ["water", "stone", "sand", "ice", "oil", "fire"] {
            let id = lab.rules.id(name).unwrap();
            assert!(
                lab.cells.iter().any(|cell| cell.element == id),
                "Density lab lacks {name}"
            );
        }
        let water = lab.rules.id("water").unwrap();
        let oil_fire = lab.rules.id("oil_fire").unwrap();
        let steam = lab.rules.id("steam").unwrap();
        assert!(
            lab.cells
                .iter()
                .filter(|cell| cell.element == oil_fire)
                .count()
                >= 4
        );
        assert_eq!(lab.ambient_temperature, 1.0);
        let width = lab.width();
        let wall_x = width / 10;
        let wall_y = lab.height() * 4 / 5 - 1;
        assert_eq!(lab.cells[wall_y * width + wall_x].element, stone);
        assert!(
            lab.cells
                .iter()
                .filter(|cell| cell.element == water)
                .count()
                > 100
        );
        let mut lab = lab;
        for _ in 0..300 {
            lab.step(2_048);
        }
        assert!(
            lab.cells.iter().any(|cell| cell.element == steam),
            "submerged vent should produce steam"
        );
        assert!(
            lab.cells
                .iter()
                .filter(|cell| cell.element == water)
                .count()
                > 100,
            "lake should remain contained"
        );
        assert!(
            lab.cells
                .iter()
                .filter(|cell| cell.element == lab.rules.id("ice").unwrap())
                .count()
                >= 8,
            "near-freezing Density lab should keep its ice afloat"
        );
    }

    #[test]
    fn room_temperature_boat_lake_does_not_freeze_after_five_simulated_seconds() {
        let rules = rules();
        let mut world = Preset::BoatLake
            .level(&rules, 400, 250)
            .generate(rules)
            .unwrap();
        for _ in 0..300 {
            world.step(12_000);
        }
        let ice = world.rules.id("ice").unwrap();
        assert_eq!(
            world
                .cells
                .iter()
                .filter(|cell| cell.element == ice)
                .count(),
            0,
            "a 20 C Boat lake should not freeze in five simulated seconds"
        );
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
    fn recipes_without_dimensions_fit_the_window_and_omit_size_on_export() {
        let recipe = "version=1\nseed=7\ngenerator='hills'\n[amounts]\nstone=100\n";
        let mut level = Level::load(recipe).unwrap();
        assert!(level.fit_window);
        assert_eq!(level.mode, LevelMode::Fixed);
        assert_eq!((level.width, level.height), (0, 0));
        assert!(level.generate(rules()).is_err());
        level.width = 96;
        level.height = 64;
        let text = level.to_toml().unwrap();
        assert!(!text.contains("width ="));
        assert!(!text.contains("height ="));
        assert!(Level::load(&text).unwrap().fit_window);
        level.mode = LevelMode::Endless;
        assert_eq!(
            Level::load(&level.to_toml().unwrap()).unwrap().mode,
            LevelMode::Endless
        );
    }

    #[test]
    fn shrinking_height_keeps_every_cell_in_the_bottom_terrain_band() {
        let rules = rules();
        let level = Preset::Island.level(&rules, 64, 48);
        let original = level.generate(rules.clone()).unwrap();
        let original_cells = original.cells().to_vec();
        let mut smaller_level = level.clone();
        smaller_level.height = 32;
        let mut smaller = smaller_level.generate(rules).unwrap();
        smaller.preserve_resize_state_from(&original);
        for y in 32..48 {
            for x in 0..64 {
                let previous = original_cells[y * 64 + x];
                if previous.element != original.rules().id("air").unwrap() {
                    assert_eq!(smaller.cells()[(y - 16) * 64 + x], previous);
                }
            }
        }
        let air = original.rules().id("air").unwrap();
        assert!(
            original.cells()[..16 * 64]
                .iter()
                .all(|cell| cell.element == air)
        );
    }

    #[test]
    fn width_extension_blends_terrain_with_a_bounded_edge_slope() {
        let rules = rules();
        let level = Preset::Island.level(&rules, 64, 48);
        let original = level.generate(rules.clone()).unwrap();
        let mut wider_level = level.clone();
        wider_level.width = 128;
        wider_level.height = 32;
        let mut wider = wider_level.generate_for_resize(rules, &original).unwrap();
        wider.preserve_resize_state_from(&original);
        let mut last_surface = terrain_surface(&wider, 63).unwrap();
        for x in 64..wider.width() {
            let surface = terrain_surface(&wider, x).unwrap();
            assert!(
                surface.abs_diff(last_surface) <= 2,
                "terrain step at x={x}: {last_surface} -> {surface}"
            );
            last_surface = surface;
        }
    }

    #[test]
    fn non_aligned_vertical_resize_preserves_nearest_atmosphere_row() {
        let rules = rules();
        let level = Preset::Island.level(&rules, 64, 48);
        let mut original = level.generate(rules.clone()).unwrap();
        original.atmosphere[0].temperature = 42.0;
        let mut taller_level = level.clone();
        taller_level.height += 1;
        let mut taller = taller_level.generate(rules).unwrap();
        taller.preserve_resize_state_from(&original);
        assert_eq!(taller.atmosphere_at(0, 0).unwrap().temperature, 42.0);
    }

    #[test]
    fn growing_height_adds_sky_above_existing_terrain() {
        let rules = rules();
        let level = Preset::Island.level(&rules, 64, 48);
        let original = level.generate(rules.clone()).unwrap();
        let original_cells = original.cells().to_vec();
        let mut taller_level = level.clone();
        taller_level.height = 64;
        let mut taller = taller_level.generate(rules).unwrap();
        taller.preserve_resize_state_from(&original);
        assert_eq!(&taller.cells()[16 * 64..], &original_cells[..]);
        let air = taller.rules().id("air").unwrap();
        assert!(
            taller.cells()[..16 * 64]
                .iter()
                .all(|cell| cell.element == air)
        );
    }

    #[test]
    fn older_recipes_default_to_calm_and_invalid_wind_fails() {
        let old =
            "version=1\nwidth=32\nheight=24\nseed=7\ngenerator='hills'\n[amounts]\nstone=100\n";
        let legacy = Level::load(old).unwrap();
        assert_eq!(legacy.wind, Wind::default());
        assert_eq!(
            (
                legacy.ambient_temperature,
                legacy.ambient_humidity,
                legacy.ambient_oxygen
            ),
            (20, 45, 21)
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
