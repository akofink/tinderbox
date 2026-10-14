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
        let mut world = World::new(self.width, self.height, self.seed, rules)
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
        assert_eq!((level.width, level.height), (0, 0));
        assert!(level.generate(rules()).is_err());
        level.width = 96;
        level.height = 64;
        let text = level.to_toml().unwrap();
        assert!(!text.contains("width ="));
        assert!(!text.contains("height ="));
        assert!(Level::load(&text).unwrap().fit_window);
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
