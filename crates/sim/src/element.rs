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
    /// Relative density used to sort neighboring liquids. Higher values sink.
    pub density: u8,
    pub lifetime: u16,
    pub decay_to: Option<usize>,
    pub emission: Option<usize>,
}

/// Read-only view of one cell in the grid.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cell {
    pub element: usize,
    pub(crate) age: u16,
}
