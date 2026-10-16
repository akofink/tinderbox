//! Data shapes shared by the ruleset and simulation.
use serde::{Deserialize, Serialize};

/// Movement behavior attached to an element in the ruleset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Motion {
    Static,
    Fall,
    Liquid,
    Gas,
}

/// Horizontal direction used by global wind and local gusts.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WindDirection {
    West,
    #[default]
    Calm,
    East,
}
impl WindDirection {
    pub(crate) const fn horizontal(self) -> i8 {
        match self {
            Self::West => -1,
            Self::Calm => 0,
            Self::East => 1,
        }
    }
}

/// Global wind configuration. Strength is clamped to 0..=4 by `World`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Wind {
    pub direction: WindDirection,
    pub strength: u8,
}

/// Element presentation and lifecycle defaults loaded from the ruleset.
#[derive(Clone, Debug)]
pub struct Element {
    pub name: String,
    pub color: [u8; 3],
    pub motion: Motion,
    /// Relative response to horizontal wind, from 0.0 (none) to 1.0 (full).
    pub wind_response: f32,
    /// Relative density for buoyancy and displacement. Lower values rise; air is the reference.
    pub density: u8,
    pub lifetime: u16,
    pub decay_to: Option<usize>,
    /// Optional probabilistic decay outcome, selected instead of `decay_to`.
    pub alternate_decay_to: Option<usize>,
    /// Chance of selecting `alternate_decay_to` when an element expires.
    pub alternate_decay_chance: f32,
    pub emission: Option<usize>,
    /// Whether this material exchanges moisture with its local atmosphere.
    pub carries_moisture: bool,
    /// Per-update fraction of the difference toward ambient humidity.
    pub moisture_rate: f32,
    /// Ignition/spread multiplier when fully moist, in the range 0..=1.
    pub wet_flammability: f32,
}

/// Read-only view of one cell in the grid.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cell {
    pub element: usize,
    pub(crate) age: u16,
    /// Consecutive updates under configured overhead burial cover.
    pub(crate) burial_age: u16,
    /// Intrinsic moisture on a 0..=255 scale.
    pub moisture: u8,
    /// Burn progress on a 0..=255 scale.
    pub burn_progress: u8,
    pub(crate) body_id: u32,
}

impl Cell {
    /// Number of simulation updates this cell has spent as its current element.
    pub const fn age(self) -> u16 {
        self.age
    }

    /// Whether the cell belongs to a moving cohesive body, such as a boat.
    pub const fn is_in_body(self) -> bool {
        self.body_id != 0
    }
}
