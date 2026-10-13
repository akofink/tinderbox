//! World storage, construction, and read-only accessors.
use crate::scheduler::Scheduler;
use crate::{Cell, Rules, RulesError, Wind, WindDirection};
use std::fmt;

/// Invalid grid dimensions or missing required world definitions.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorldError(pub String);
impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for WorldError {}
impl From<RulesError> for WorldError {
    fn from(error: RulesError) -> Self {
        Self(error.to_string())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct GustCell {
    pub(super) direction: WindDirection,
    pub(super) strength: u8,
    pub(super) remaining: u8,
}

/// Deterministic material grid and bounded-work update state.
pub struct World {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) cells: Vec<Cell>,
    pub(super) rules: Rules,
    pub(super) scheduler: Scheduler,
    pub(super) rng: u64,
    pub(super) wind: Wind,
    pub(super) gusts: Vec<GustCell>,
}
impl World {
    /// Creates a world with preallocated cells and focus-queue storage.
    pub fn new(width: usize, height: usize, seed: u64, rules: Rules) -> Result<Self, WorldError> {
        let count = width
            .checked_mul(height)
            .filter(|n| *n > 0 && *n <= 16_777_216)
            .ok_or_else(|| {
                WorldError("world dimensions must be non-zero and at most 16,777,216 cells".into())
            })?;
        let air = rules
            .id("air")
            .ok_or_else(|| WorldError("ruleset must define 'air'".into()))?;
        Ok(Self {
            width,
            height,
            cells: vec![
                Cell {
                    element: air,
                    age: 0
                };
                count
            ],
            rules,
            scheduler: Scheduler::new(count),
            rng: seed.max(1),
            wind: Wind::default(),
            gusts: vec![GustCell::default(); count],
        })
    }

    /// Grid width in cells.
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Grid height in cells.
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Immutable row-major cell storage.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Immutable active ruleset.
    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    /// Count of deferred player-focused updates waiting ahead of background work.
    pub fn pending_work(&self) -> usize {
        self.scheduler.pending_focus()
    }

    /// Current global horizontal wind.
    pub const fn wind(&self) -> Wind {
        self.wind
    }

    /// Sets global wind. Strength is clamped to 0..=4.
    pub fn set_wind(&mut self, direction: WindDirection, strength: u8) {
        self.wind = Wind {
            direction,
            strength: strength.min(4),
        };
    }

    /// Applies a short-lived local gust over a circular brush centered at `(x, y)`.
    /// Radius is capped at 64 cells and strength is clamped to 0..=4.
    pub fn gust(
        &mut self,
        x: usize,
        y: usize,
        radius: usize,
        direction: WindDirection,
        strength: u8,
    ) {
        if x >= self.width || y >= self.height || direction == WindDirection::Calm || strength == 0
        {
            return;
        }
        let radius = radius.min(64);
        let start_x = x.saturating_sub(radius);
        let end_x = x.saturating_add(radius).min(self.width - 1);
        let start_y = y.saturating_sub(radius);
        let end_y = y.saturating_add(radius).min(self.height - 1);
        let radius_squared = radius * radius;
        for gy in start_y..=end_y {
            for gx in start_x..=end_x {
                let dx = gx.abs_diff(x);
                let dy = gy.abs_diff(y);
                if dx * dx + dy * dy <= radius_squared {
                    self.gusts[gy * self.width + gx] = GustCell {
                        direction,
                        strength: strength.min(4),
                        remaining: 12,
                    };
                }
            }
        }
    }

    pub(super) fn wind_at(&self, index: usize) -> Wind {
        let gust = self.gusts[index];
        if gust.remaining > 0 {
            Wind {
                direction: gust.direction,
                strength: gust.strength,
            }
        } else {
            self.wind
        }
    }
}
