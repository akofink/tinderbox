//! World storage, construction, and read-only accessors.
use crate::dirty::DirtyRegions;
use crate::scheduler::Scheduler;
use crate::{Cell, DirtyRegion, Rules, RulesError, Wind, WindDirection};
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

pub(super) const ATMOSPHERE_CELL_SIZE: usize = 8;
pub(super) const ATMOSPHERE_INTERVAL: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtmosphereCell {
    pub temperature: f32,
    pub humidity: f32,
    pub pressure: f32,
}
impl Default for AtmosphereCell {
    fn default() -> Self {
        Self {
            temperature: 20.0,
            humidity: 45.0,
            pressure: 0.0,
        }
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
    pub(super) dirty_regions: DirtyRegions,
    pub(super) rng: u64,
    pub(super) wind: Wind,
    pub(super) gusts: Vec<GustCell>,
    pub(super) atmosphere: Vec<AtmosphereCell>,
    pub(super) atmosphere_width: usize,
    pub(super) atmosphere_height: usize,
    pub(super) atmosphere_cursor: usize,
    pub(super) atmosphere_credit: usize,
    pub(super) ambient_temperature: f32,
    pub(super) ambient_humidity: f32,
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
        let atmosphere_width = width.div_ceil(ATMOSPHERE_CELL_SIZE);
        let atmosphere_height = height.div_ceil(ATMOSPHERE_CELL_SIZE);
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
            dirty_regions: DirtyRegions::new(width, height),
            rng: seed.max(1),
            wind: Wind::default(),
            gusts: vec![GustCell::default(); count],
            atmosphere: vec![AtmosphereCell::default(); atmosphere_width * atmosphere_height],
            atmosphere_width,
            atmosphere_height,
            atmosphere_cursor: 0,
            atmosphere_credit: 0,
            ambient_temperature: 20.0,
            ambient_humidity: 45.0,
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

    /// Number of deduplicated regions waiting for presentation.
    pub fn pending_dirty_regions(&self) -> usize {
        self.dirty_regions.len()
    }

    /// Marks every texture region dirty after a bulk visual change.
    pub fn mark_all_dirty(&mut self) {
        self.dirty_regions.mark_all();
    }

    /// Removes the oldest dirty texture region, if any.
    ///
    /// Callers should drain a bounded number per frame and preserve the returned region until it
    /// has been uploaded. Dirty regions are clipped at the world edges and coalesce changes until
    /// popped; changes made after a pop enqueue the region again.
    pub fn pop_dirty_region(&mut self) -> Option<DirtyRegion> {
        self.dirty_regions.pop()
    }

    /// Set level-wide ambient air conditions, clamped to the supported ranges.
    pub fn set_ambient(&mut self, temperature: i16, humidity: u8) {
        self.ambient_temperature = f32::from(temperature.clamp(-40, 60));
        self.ambient_humidity = f32::from(humidity.min(100));
        self.atmosphere.fill(AtmosphereCell {
            temperature: self.ambient_temperature,
            humidity: self.ambient_humidity,
            pressure: 0.0,
        });
    }

    /// Atmospheric conditions at a cell coordinate, sampled from the fixed coarse grid.
    pub fn atmosphere_at(&self, x: usize, y: usize) -> Option<AtmosphereCell> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y / ATMOSPHERE_CELL_SIZE) * self.atmosphere_width + x / ATMOSPHERE_CELL_SIZE;
        self.atmosphere.get(index).copied()
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
            return Wind {
                direction: gust.direction,
                strength: gust.strength,
            };
        }
        let x = index % self.width;
        let y = index / self.width;
        let ax = x / ATMOSPHERE_CELL_SIZE;
        let ay = y / ATMOSPHERE_CELL_SIZE;
        let here = self.atmosphere[ay * self.atmosphere_width + ax].pressure;
        let west = if ax > 0 {
            self.atmosphere[ay * self.atmosphere_width + ax - 1].pressure
        } else {
            here
        };
        let east = if ax + 1 < self.atmosphere_width {
            self.atmosphere[ay * self.atmosphere_width + ax + 1].pressure
        } else {
            here
        };
        let local = east - west;
        let threshold = self.rules.atmosphere.pressure_wind;
        let local_component = if local.abs() > threshold {
            let strength = ((local.abs() / (threshold.max(0.01) * 4.0)).ceil() as i8).clamp(1, 4);
            if local < 0.0 { strength } else { -strength }
        } else {
            0
        };
        let combined =
            self.wind.direction.horizontal() * self.wind.strength as i8 + local_component;
        match combined.cmp(&0) {
            std::cmp::Ordering::Greater => Wind {
                direction: WindDirection::East,
                strength: combined.min(4) as u8,
            },
            std::cmp::Ordering::Less => Wind {
                direction: WindDirection::West,
                strength: (-combined).min(4) as u8,
            },
            std::cmp::Ordering::Equal => Wind::default(),
        }
    }
}
