//! World storage, construction, and read-only accessors.
use crate::bodies::Body;
use crate::dirty::DirtyRegions;
use crate::scheduler::Scheduler;
use crate::span::{DormantBody, DormantChunk};
use crate::{Cell, DirtyRegion, Rules, RulesError, Wind, WindDirection};
use std::collections::BTreeMap;
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
pub(super) const MAX_EDGE_WATER_COLUMN: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtmosphereCell {
    pub temperature: f32,
    pub humidity: f32,
    pub pressure: f32,
    pub oxygen: f32,
}
impl Default for AtmosphereCell {
    fn default() -> Self {
        Self {
            temperature: 20.0,
            humidity: 45.0,
            pressure: 0.0,
            oxygen: 21.0,
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
    pub(super) tree_support_age: Vec<u8>,
    pub(super) bodies: Vec<Body>,
    pub(super) rules: Rules,
    interaction_lookup: Vec<usize>,
    pub(super) scheduler: Scheduler,
    pub(super) dirty_regions: DirtyRegions,
    pub(super) rng: u64,
    pub(super) density_rng: u64,
    pub(super) weather_rng: u64,
    pub(super) wind: Wind,
    pub(super) gusts: Vec<GustCell>,
    pub(super) atmosphere: Vec<AtmosphereCell>,
    pub(super) atmosphere_width: usize,
    pub(super) atmosphere_height: usize,
    pub(super) atmosphere_cursor: usize,
    pub(super) atmosphere_credit: usize,
    pub(super) ambient_temperature: f32,
    pub(super) ambient_humidity: f32,
    pub(super) ambient_oxygen: f32,
    /// Signed index of the leftmost resident chunk. Unused while `chunk_width` is 0.
    pub(super) chunk_origin: i64,
    /// Endless chunk width. Zero keeps the fixed-world whole-grid behavior.
    pub(super) chunk_width: usize,
    pub(super) active_margin_chunks: usize,
    pub(super) dormant_chunks: BTreeMap<i64, DormantChunk>,
    pub(super) dormant_bodies: Vec<DormantBody>,
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
        let interaction_lookup = build_interaction_lookup(&rules);
        Ok(Self {
            width,
            height,
            cells: vec![
                Cell {
                    element: air,
                    age: 0,
                    moisture: 0,
                    burn_progress: 0,
                    body_id: 0,
                };
                count
            ],
            tree_support_age: vec![0; count],
            rules,
            interaction_lookup,
            bodies: Vec::new(),
            scheduler: Scheduler::new(count),
            dirty_regions: DirtyRegions::new(width, height),
            rng: seed.max(1),
            density_rng: (seed ^ 0xD1CE_BA5E_5EED_1234).max(1),
            weather_rng: seed.wrapping_add(0x9e37_79b9_7f4a_7c15).max(1),
            wind: Wind::default(),
            gusts: vec![GustCell::default(); count],
            atmosphere: vec![AtmosphereCell::default(); atmosphere_width * atmosphere_height],
            atmosphere_width,
            atmosphere_height,
            atmosphere_cursor: 0,
            atmosphere_credit: 0,
            ambient_temperature: 20.0,
            ambient_humidity: 45.0,
            ambient_oxygen: 21.0,
            chunk_origin: 0,
            chunk_width: 0,
            active_margin_chunks: 2,
            dormant_chunks: BTreeMap::new(),
            dormant_bodies: Vec::new(),
        })
    }

    /// Append or prepend one generated horizontal chunk, preserving resident simulation state.
    pub fn extend_horizontal(
        &mut self,
        chunk: &crate::GeneratedChunk,
        prepend: bool,
    ) -> Result<(), WorldError> {
        if chunk.height != self.height
            || chunk.width == 0
            || chunk.cells.len() != chunk.width * chunk.height
        {
            return Err(WorldError(
                "generated chunk dimensions do not match the world".into(),
            ));
        }
        let old_width = self.width;
        let pending_dirty = self.dirty_regions.pending_origins().collect::<Vec<_>>();
        let new_width = old_width
            .checked_add(chunk.width)
            .filter(|width| {
                width
                    .checked_mul(self.height)
                    .is_some_and(|count| count <= 16_777_216)
            })
            .ok_or_else(|| WorldError("extended world exceeds the 16,777,216-cell limit".into()))?;
        let offset_x = if prepend { chunk.width } else { 0 };
        let count = new_width * self.height;
        let mut cells = vec![Cell::default(); count];
        let mut support_age = vec![0; count];
        let mut gusts = vec![GustCell::default(); count];
        let old_cells = std::mem::take(&mut self.cells);
        let old_support_age = std::mem::take(&mut self.tree_support_age);
        let old_gusts = std::mem::take(&mut self.gusts);
        for y in 0..self.height {
            let source = y * old_width;
            let target = y * new_width + offset_x;
            cells[target..target + old_width]
                .copy_from_slice(&old_cells[source..source + old_width]);
            support_age[target..target + old_width]
                .copy_from_slice(&old_support_age[source..source + old_width]);
            gusts[target..target + old_width]
                .copy_from_slice(&old_gusts[source..source + old_width]);
            let chunk_target = y * new_width + if prepend { 0 } else { old_width };
            let chunk_source = y * chunk.width;
            cells[chunk_target..chunk_target + chunk.width]
                .copy_from_slice(&chunk.cells[chunk_source..chunk_source + chunk.width]);
            if let Some(tree) = self.rules.id("tree") {
                for x in 0..chunk.width {
                    if cells[chunk_target + x].element == tree {
                        support_age[chunk_target + x] =
                            ((y * new_width + if prepend { x } else { old_width + x }) % 64) as u8;
                    }
                }
            }
        }
        let mut atmosphere = vec![
            AtmosphereCell {
                temperature: self.ambient_temperature,
                humidity: self.ambient_humidity,
                pressure: 0.0,
                oxygen: self.ambient_oxygen,
            };
            new_width.div_ceil(ATMOSPHERE_CELL_SIZE) * self.atmosphere_height
        ];
        let new_atmosphere_width = new_width.div_ceil(ATMOSPHERE_CELL_SIZE);
        let atmosphere_offset = offset_x / ATMOSPHERE_CELL_SIZE;
        for y in 0..self.atmosphere_height {
            let source = y * self.atmosphere_width;
            let target = y * new_atmosphere_width + atmosphere_offset;
            atmosphere[target..target + self.atmosphere_width]
                .copy_from_slice(&self.atmosphere[source..source + self.atmosphere_width]);
        }
        for body in &mut self.bodies {
            for member in &mut body.members {
                let x = *member % old_width;
                let y = *member / old_width;
                *member = y * new_width + offset_x + x;
            }
            body.anchor = *body.members.iter().min().unwrap_or(&0);
        }
        self.width = new_width;
        self.cells = cells;
        self.tree_support_age = support_age;
        self.gusts = gusts;
        self.atmosphere = atmosphere;
        self.atmosphere_width = new_atmosphere_width;
        self.atmosphere_cursor = 0;
        self.atmosphere_credit = 0;
        self.scheduler = Scheduler::new(count);
        self.dirty_regions = DirtyRegions::new(new_width, self.height);
        for (x, y) in pending_dirty {
            self.dirty_regions.mark_cell(x + offset_x, y);
        }
        if prepend {
            self.mark_all_dirty();
        } else {
            for y in 0..self.height {
                for x in old_width..new_width {
                    self.dirty_regions.mark_cell(x, y);
                }
            }
        }
        if prepend && self.chunk_width > 0 && chunk.width == self.chunk_width {
            self.chunk_origin -= 1;
        }
        Ok(())
    }

    /// Overlays the overlapping cells and live simulation state from an earlier world.
    /// The receiver supplies newly exposed cells, typically generated from the same level seed.
    pub fn preserve_resize_state_from(&mut self, previous: &World) {
        self.preserve_resize_state_from_offset(previous, 0);
    }

    /// Preserves a previous world at a horizontal cell offset in a larger regenerated region.
    pub fn preserve_resize_state_from_offset(&mut self, previous: &World, offset_x: usize) {
        if offset_x >= self.width {
            return;
        }
        let copy_width = previous.width.min(self.width - offset_x);
        // Keep the world bottom-anchored: shrinking discards sky from the top,
        // while growing adds rows of generated sky above existing terrain.
        let row_offset = self.height as isize - previous.height as isize;

        self.bodies.clear();
        for cell in &mut self.cells {
            cell.body_id = 0;
        }
        for y in 0..self.height {
            let source_y = y as isize - row_offset;
            if !(0..previous.height as isize).contains(&source_y) {
                continue;
            }
            for x in 0..copy_width {
                let source = previous.cells[source_y as usize * previous.width + x];
                let target = y * self.width + offset_x + x;
                self.cells[target] = source;
                self.cells[target].body_id = 0;
                if self.rules.id("tree") == Some(source.element) {
                    self.tree_support_age[target] = (target % 64) as u8;
                }
            }
        }
        for old_body in &previous.bodies {
            let mut body = old_body.clone();
            body.members = old_body
                .members
                .iter()
                .filter_map(|&index| {
                    let x = index % previous.width;
                    let y = index / previous.width;
                    let target_y = y as isize + row_offset;
                    (x + offset_x < self.width && (0..self.height as isize).contains(&target_y))
                        .then(|| target_y as usize * self.width + offset_x + x)
                })
                .collect();
            if body.members.is_empty() {
                continue;
            }
            body.anchor = *body.members.iter().min().unwrap_or(&0);
            body.active = old_body.active;
            let id = self.bodies.len() as u32 + 1;
            for &index in &body.members {
                self.cells[index].body_id = id;
            }
            self.bodies.push(body);
        }

        self.rng = previous.rng;
        self.density_rng = previous.density_rng;
        self.weather_rng = previous.weather_rng;
        self.wind = previous.wind;
        self.ambient_temperature = previous.ambient_temperature;
        self.ambient_humidity = previous.ambient_humidity;
        self.ambient_oxygen = previous.ambient_oxygen;
        for y in 0..self.atmosphere_height {
            let target_cell_y =
                (y * ATMOSPHERE_CELL_SIZE + ATMOSPHERE_CELL_SIZE / 2).min(self.height - 1);
            let source_cell_y = target_cell_y as isize - row_offset;
            if !(0..previous.height as isize).contains(&source_cell_y) {
                continue;
            }
            let source_y = source_cell_y as usize / ATMOSPHERE_CELL_SIZE;
            let target_start = offset_x / ATMOSPHERE_CELL_SIZE;
            let column_count = self
                .atmosphere_width
                .saturating_sub(target_start)
                .min(previous.atmosphere_width);
            for x in 0..column_count {
                if source_y < previous.atmosphere_height {
                    self.atmosphere[y * self.atmosphere_width + target_start + x] =
                        previous.atmosphere[source_y * previous.atmosphere_width + x];
                }
            }
        }
        self.mark_all_dirty();
    }

    pub(super) fn rebuild_interaction_lookup(&mut self) {
        self.interaction_lookup = build_interaction_lookup(&self.rules);
    }

    /// Returns the first enabled rule for an element pair, matching rules-file order.
    pub(super) fn interaction(&self, when: usize, with: usize) -> Option<&crate::Interaction> {
        if self.interaction_lookup.is_empty() {
            return self
                .rules
                .interactions
                .iter()
                .find(|rule| rule.enabled && rule.when == when && rule.with == with);
        }
        let index = self.interaction_lookup[when * self.rules.elements.len() + with];
        self.rules.interactions.get(index)
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

    /// Marks texture regions intersecting a clipped rectangle dirty.
    pub fn mark_dirty_rect(&mut self, x: usize, y: usize, width: usize, height: usize) {
        let x_end = x.saturating_add(width).min(self.width);
        let y_end = y.saturating_add(height).min(self.height);
        if x >= x_end || y >= y_end {
            return;
        }
        let side = crate::dirty::DIRTY_REGION_SIDE;
        let x_start = x / side * side;
        let y_start = y / side * side;
        for region_y in (y_start..y_end).step_by(side) {
            for region_x in (x_start..x_end).step_by(side) {
                self.dirty_regions.mark_cell(region_x, region_y);
            }
        }
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
    pub fn set_ambient(&mut self, temperature: i16, humidity: u8, oxygen: u8) {
        self.ambient_temperature = f32::from(temperature.clamp(-40, 60));
        self.ambient_humidity = f32::from(humidity.min(100));
        self.ambient_oxygen = f32::from(oxygen.min(100));
        self.atmosphere.fill(AtmosphereCell {
            temperature: self.ambient_temperature,
            humidity: self.ambient_humidity,
            pressure: 0.0,
            oxygen: self.ambient_oxygen,
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

    /// Effective horizontal wind at a cell, including local pressure gradients and active gusts.
    pub fn local_wind_at(&self, x: usize, y: usize) -> Option<Wind> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(self.wind_at(y * self.width + x))
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

fn build_interaction_lookup(rules: &Rules) -> Vec<usize> {
    const MAX_LOOKUP_CELLS: usize = 1_048_576;
    let Some(length) = rules
        .elements
        .len()
        .checked_mul(rules.elements.len())
        .filter(|length| *length <= MAX_LOOKUP_CELLS)
    else {
        return Vec::new();
    };
    let mut lookup = vec![usize::MAX; length];
    let element_count = rules.elements.len();
    for (index, rule) in rules.interactions.iter().enumerate() {
        if rule.enabled && rule.when < element_count && rule.with < element_count {
            let slot = &mut lookup[rule.when * element_count + rule.with];
            if *slot == usize::MAX {
                *slot = index;
            }
        }
    }
    lookup
}
