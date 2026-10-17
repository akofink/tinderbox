//! Active-span retention for endless worlds.
//!
//! Boundary rule: sealed active edge, no catch-up. The resident grid is the active span, so a cell
//! at that edge uses the same missing-neighbor rule as a fixed world edge. Fire, water, falling
//! sand, and wind-carried ash neither read nor write dormant chunks. A chunk that leaves the span
//! freezes. If it still matches its seed generation, it is dropped and regenerated on return;
//! otherwise its cells, tree-support ages, outcome-relevant plant/tree darkness exposure, and
//! atmosphere columns are stored and restored exactly. Derived light is recomputed on wake. Local
//! gusts expire. Woken chunks do not catch up.
//!
//! A cohesive body is never split. If it intersects the span, every chunk it occupies stays
//! resident, which adds at most one chunk at an edge for the current boat hull.
//!
//! Rebuilding the span shifts resident cell indices. A caller that mirrors those indices in a
//! texture must invalidate the visible rect when [`SpanAdjustment::added_left`] or
//! [`SpanAdjustment::removed_left`] is non-zero. Fresh chunks are marked dirty; dormant chunks are
//! not resident, so they receive no texture uploads.

use crate::bodies::Body;
use crate::scheduler::Scheduler;
use crate::world::{ATMOSPHERE_CELL_SIZE, GustCell};
use crate::{AtmosphereCell, Cell, GeneratedChunk, World, WorldError};
use std::collections::BTreeMap;

/// How a span retain changed the resident grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpanAdjustment {
    /// Signed index of the leftmost resident chunk after the retain.
    pub origin: i64,
    /// Resident width in cells after the retain.
    pub width: usize,
    /// Cells inserted on the left. Local camera coordinates increase by this amount.
    pub added_left: usize,
    /// Cells removed from the left. Local camera coordinates decrease by this amount.
    pub removed_left: usize,
}

#[derive(Clone, Debug)]
pub(super) struct DormantChunk {
    cells: Vec<u8>,
    support_age: Vec<u8>,
    darkness_exposure: Vec<(u32, u32)>,
    atmosphere: Vec<AtmosphereCell>,
}

#[derive(Clone, Debug)]
pub(super) struct DormantBody {
    members: Vec<(i64, u32)>,
    hull_cells: usize,
    bob_phase: u8,
    active: bool,
}

struct ChunkColumn {
    cells: Vec<Cell>,
    support_age: Vec<u8>,
    light: Vec<u8>,
    darkness_age: Vec<u32>,
    gusts: Vec<GustCell>,
    atmosphere: Vec<AtmosphereCell>,
    fresh: bool,
}

struct SpanPlan {
    first: i64,
    last: i64,
    columns: Vec<ChunkColumn>,
    kept_bodies: Vec<DormantBody>,
    dormant_bodies: Vec<DormantBody>,
    snapshots: BTreeMap<i64, DormantChunk>,
    pending_dirty: Vec<(i64, usize)>,
}

impl World {
    /// Enables chunk residency for an endless world whose width is already a span of chunks.
    pub fn enable_chunk_span(&mut self, origin: i64, chunk_width: usize) -> Result<(), WorldError> {
        if chunk_width == 0 || chunk_width % ATMOSPHERE_CELL_SIZE != 0 {
            return Err(WorldError(
                "chunk width must be a positive multiple of the atmosphere cell".into(),
            ));
        }
        if self.width == 0 || self.width % chunk_width != 0 {
            return Err(WorldError(
                "world width must be a multiple of the chunk width".into(),
            ));
        }
        self.chunk_origin = origin;
        self.chunk_width = chunk_width;
        self.dormant_chunks.clear();
        self.dormant_bodies.clear();
        Ok(())
    }

    /// Whether distant chunks can leave the resident grid.
    pub const fn chunk_span_enabled(&self) -> bool {
        self.chunk_width > 0
    }

    /// Signed index of the leftmost resident chunk, or 0 when the span is disabled.
    pub const fn chunk_origin(&self) -> i64 {
        self.chunk_origin
    }

    /// Extra chunks kept on each side of the requested camera view.
    pub const fn active_margin_chunks(&self) -> usize {
        self.active_margin_chunks
    }

    /// Sets the active-span margin. Values above 64 are clamped.
    pub fn set_active_margin_chunks(&mut self, chunks: usize) {
        self.active_margin_chunks = chunks.min(64);
    }

    /// Modified chunks stored outside the resident grid.
    pub fn dormant_chunk_count(&self) -> usize {
        self.dormant_chunks.len()
    }

    /// Bytes used by compact dormant chunk payloads, excluding body records.
    pub fn dormant_payload_bytes(&self) -> usize {
        self.dormant_chunks
            .values()
            .map(|chunk| {
                chunk.cells.len()
                    + chunk.support_age.len()
                    + chunk.darkness_exposure.len() * std::mem::size_of::<(u32, u32)>()
                    + chunk.atmosphere.len() * std::mem::size_of::<AtmosphereCell>()
            })
            .sum()
    }

    /// Chunks that should be resident for a camera position, including the margin and any
    /// cohesive body that would otherwise be split. `None` when the span is disabled.
    pub fn active_chunk_range(
        &self,
        global_camera_x: i64,
        view_width: usize,
    ) -> Option<(i64, i64)> {
        (self.chunk_width > 0)
            .then(|| self.covered_chunk_range(global_camera_x, view_width.max(1)))
            .and_then(Result::ok)
    }

    /// Whether a signed chunk index is currently in the resident grid.
    pub fn chunk_is_resident(&self, index: i64) -> bool {
        let Some(count) = self.resident_chunk_count() else {
            return false;
        };
        (self.chunk_origin..self.chunk_origin + count as i64).contains(&index)
    }

    /// Keeps only the chunks covering `global_camera_x` and `view_width`, plus the margin.
    ///
    /// `generate` must be a pure function of the signed chunk index. Fixed worlds, and endless
    /// worlds that have not enabled a span, return without calling it. The method does not mutate
    /// the world if generation fails.
    pub fn retain_active_span(
        &mut self,
        global_camera_x: i64,
        view_width: usize,
        mut generate: impl FnMut(i64) -> Result<GeneratedChunk, WorldError>,
    ) -> Result<SpanAdjustment, WorldError> {
        if self.chunk_width == 0 {
            return Ok(self.current_adjustment());
        }
        if self.width % self.chunk_width != 0 {
            return Err(WorldError(
                "resident width is no longer a multiple of the chunk width".into(),
            ));
        }
        let (first, last) = self.covered_chunk_range(global_camera_x, view_width.max(1))?;
        let current_last = self
            .chunk_origin
            .saturating_add(self.resident_chunk_count().unwrap_or(0) as i64)
            .saturating_sub(1);
        if first == self.chunk_origin && last == current_last {
            return Ok(self.current_adjustment());
        }
        let plan = self.plan_span(first, last, &mut generate)?;
        let adjustment = self.adjustment_for(plan.first, plan.last);
        self.apply_span(plan)?;
        Ok(adjustment)
    }

    fn current_adjustment(&self) -> SpanAdjustment {
        SpanAdjustment {
            origin: self.chunk_origin,
            width: self.width,
            added_left: 0,
            removed_left: 0,
        }
    }

    fn adjustment_for(&self, first: i64, last: i64) -> SpanAdjustment {
        let chunk_width = self.chunk_width;
        let added_left = if first < self.chunk_origin {
            (self.chunk_origin - first) as usize * chunk_width
        } else {
            0
        };
        let removed_left = if first > self.chunk_origin {
            (first - self.chunk_origin) as usize * chunk_width
        } else {
            0
        };
        SpanAdjustment {
            origin: first,
            width: (last - first + 1) as usize * chunk_width,
            added_left,
            removed_left,
        }
    }

    fn resident_chunk_count(&self) -> Option<usize> {
        (self.chunk_width > 0 && self.width % self.chunk_width == 0)
            .then_some(self.width / self.chunk_width)
    }

    fn covered_chunk_range(
        &self,
        global_camera_x: i64,
        view_width: usize,
    ) -> Result<(i64, i64), WorldError> {
        let chunk_width = i64::try_from(self.chunk_width)
            .map_err(|_| WorldError("chunk width does not fit in a signed coordinate".into()))?;
        let view = i64::try_from(view_width).unwrap_or(i64::MAX);
        let margin = chunk_width.saturating_mul(self.active_margin_chunks as i64);
        let cover_left = global_camera_x.saturating_sub(margin);
        let cover_right = global_camera_x.saturating_add(view).saturating_add(margin);
        let mut first = div_floor(cover_left, chunk_width);
        let mut last = div_floor(cover_right.saturating_sub(1), chunk_width);
        if last < first {
            last = first;
        }
        self.expand_range_for_bodies(&mut first, &mut last);
        let count = last.saturating_sub(first).saturating_add(1);
        if count <= 0 {
            return Err(WorldError("active span is empty".into()));
        }
        Ok((first, last))
    }

    fn expand_range_for_bodies(&self, first: &mut i64, last: &mut i64) {
        for body in &self.bodies {
            self.expand_range(first, last, body_chunk_indices(self, body));
        }
        for body in &self.dormant_bodies {
            self.expand_range(first, last, body.members.iter().map(|(chunk, _)| *chunk));
        }
    }

    fn expand_range(&self, first: &mut i64, last: &mut i64, chunks: impl IntoIterator<Item = i64>) {
        let mut min_chunk = None;
        let mut max_chunk = None;
        let mut intersects = false;
        for chunk in chunks {
            min_chunk = Some(min_chunk.map_or(chunk, |current: i64| current.min(chunk)));
            max_chunk = Some(max_chunk.map_or(chunk, |current: i64| current.max(chunk)));
            if (*first..=*last).contains(&chunk) {
                intersects = true;
            }
        }
        if intersects && let (Some(min_chunk), Some(max_chunk)) = (min_chunk, max_chunk) {
            *first = (*first).min(min_chunk);
            *last = (*last).max(max_chunk);
        }
    }

    fn plan_span(
        &self,
        first: i64,
        last: i64,
        generate: &mut impl FnMut(i64) -> Result<GeneratedChunk, WorldError>,
    ) -> Result<SpanPlan, WorldError> {
        let mut generated = BTreeMap::new();
        let mut snapshots = BTreeMap::new();
        for (&index, chunk) in &self.dormant_chunks {
            if !(first..=last).contains(&index) {
                snapshots.insert(index, chunk.clone());
            }
        }
        let current_count = self.resident_chunk_count().unwrap_or(0) as i64;
        for offset in 0..current_count {
            let index = self.chunk_origin + offset;
            if (first..=last).contains(&index) {
                continue;
            }
            if let Some(snapshot) =
                self.snapshot_chunk(offset as usize, index, &mut generated, generate)?
            {
                snapshots.insert(index, snapshot);
            }
        }

        let new_width = (last - first + 1) as usize * self.chunk_width;
        let mut columns = Vec::new();
        for index in first..=last {
            columns.push(self.column_for(index, first, new_width, &mut generated, generate)?);
        }
        let (mut kept_bodies, mut dormant_bodies) = self.partition_bodies(first, last);
        for body in &self.dormant_bodies {
            if body_covers(body, first, last) {
                kept_bodies.push(body.clone());
            } else {
                dormant_bodies.push(body.clone());
            }
        }
        let pending_dirty = self
            .dirty_regions
            .pending_origins()
            .map(|(x, y)| (self.chunk_origin * self.chunk_width as i64 + x as i64, y))
            .collect();
        Ok(SpanPlan {
            first,
            last,
            columns,
            kept_bodies,
            dormant_bodies,
            snapshots,
            pending_dirty,
        })
    }

    fn column_for(
        &self,
        index: i64,
        first: i64,
        new_width: usize,
        generated: &mut BTreeMap<i64, GeneratedChunk>,
        generate: &mut impl FnMut(i64) -> Result<GeneratedChunk, WorldError>,
    ) -> Result<ChunkColumn, WorldError> {
        if let Some(local) = self.local_chunk(index) {
            return Ok(self.extract_chunk(local, false));
        }
        if let Some(snapshot) = self.dormant_chunks.get(&index) {
            return self.column_from_snapshot(snapshot);
        }
        let chunk = cached_chunk(index, generated, generate)?;
        self.column_from_generated(&chunk, index, first, new_width)
    }

    fn snapshot_chunk(
        &self,
        local: usize,
        index: i64,
        generated: &mut BTreeMap<i64, GeneratedChunk>,
        generate: &mut impl FnMut(i64) -> Result<GeneratedChunk, WorldError>,
    ) -> Result<Option<DormantChunk>, WorldError> {
        let chunk = cached_chunk(index, generated, generate)?;
        if !self.chunk_is_modified(local, &chunk) {
            return Ok(None);
        }
        let column = self.extract_chunk(local, false);
        let darkness_exposure = column
            .cells
            .iter()
            .zip(&column.darkness_age)
            .enumerate()
            .filter_map(|(index, (cell, &age))| {
                let name = self.rules.elements[cell.element].name.as_str();
                ((name == "plant" || name == "tree") && age > 0).then_some((index as u32, age))
            })
            .collect();
        Ok(Some(DormantChunk {
            cells: encode_cells(&column.cells)?,
            support_age: column.support_age,
            darkness_exposure,
            atmosphere: column.atmosphere,
        }))
    }

    fn chunk_is_modified(&self, local: usize, generated: &GeneratedChunk) -> bool {
        let width = self.chunk_width;
        if generated.width != width || generated.cells.len() != width * self.height {
            return true;
        }
        let plant = self.rules.id("plant");
        let tree = self.rules.id("tree");
        for y in 0..self.height {
            let resident = y * self.width + local * width;
            let source = y * width;
            for x in 0..width {
                let cell = self.cells[resident + x];
                let fresh = generated.cells[source + x];
                if cell.element != fresh.element
                    || cell.age != fresh.age
                    || cell.burial_age != fresh.burial_age
                    || cell.moisture != fresh.moisture
                    || cell.burn_progress != fresh.burn_progress
                    || cell.body_id != 0
                {
                    return true;
                }
                if (plant == Some(cell.element) || tree == Some(cell.element))
                    && self.darkness_age[resident + x] > 0
                {
                    return true;
                }
            }
        }
        let ambient = self.ambient_atmosphere();
        let ax_count = width / ATMOSPHERE_CELL_SIZE;
        let ax0 = local * ax_count;
        for y in 0..self.atmosphere_height {
            let row = y * self.atmosphere_width + ax0;
            if self.atmosphere[row..row + ax_count]
                .iter()
                .any(|cell| *cell != ambient)
            {
                return true;
            }
        }
        for y in 0..self.height {
            let row = y * self.width + local * width;
            if self.gusts[row..row + width]
                .iter()
                .any(|gust| gust.remaining > 0)
            {
                return true;
            }
        }
        false
    }

    fn extract_chunk(&self, local: usize, fresh: bool) -> ChunkColumn {
        let width = self.chunk_width;
        let mut cells = Vec::with_capacity(width * self.height);
        let mut support_age = Vec::with_capacity(width * self.height);
        let mut light = Vec::with_capacity(width * self.height);
        let mut darkness_age = Vec::with_capacity(width * self.height);
        let mut gusts = Vec::with_capacity(width * self.height);
        for y in 0..self.height {
            let start = y * self.width + local * width;
            cells.extend(self.cells[start..start + width].iter().map(|cell| Cell {
                body_id: 0,
                ..*cell
            }));
            support_age.extend_from_slice(&self.tree_support_age[start..start + width]);
            light.extend_from_slice(&self.light[start..start + width]);
            darkness_age.extend_from_slice(&self.darkness_age[start..start + width]);
            gusts.extend_from_slice(&self.gusts[start..start + width]);
        }
        ChunkColumn {
            cells,
            support_age,
            light,
            darkness_age,
            gusts,
            atmosphere: self.chunk_atmosphere(local),
            fresh,
        }
    }

    fn column_from_snapshot(&self, snapshot: &DormantChunk) -> Result<ChunkColumn, WorldError> {
        let expected = self.chunk_width * self.height;
        let cells = decode_cells(&snapshot.cells, expected)?;
        if snapshot.support_age.len() != expected {
            return Err(WorldError(
                "dormant chunk support ages do not match the world".into(),
            ));
        }
        let ax_count = self.chunk_width / ATMOSPHERE_CELL_SIZE;
        if snapshot.atmosphere.len() != self.atmosphere_height * ax_count {
            return Err(WorldError(
                "dormant chunk atmosphere does not match the world".into(),
            ));
        }
        let plant = self.rules.id("plant");
        let tree = self.rules.id("tree");
        let mut darkness_age = vec![0; expected];
        for &(index, age) in &snapshot.darkness_exposure {
            let index = index as usize;
            if index >= expected
                || (plant != Some(cells[index].element) && tree != Some(cells[index].element))
            {
                return Err(WorldError(
                    "dormant chunk darkness exposure does not match its cells".into(),
                ));
            }
            darkness_age[index] = age;
        }
        Ok(ChunkColumn {
            cells,
            support_age: snapshot.support_age.clone(),
            light: vec![
                if self.rules.daylight.enabled {
                    crate::world::daylight_sky(self.rules.daylight.cycle_ticks, self.day_phase)
                } else {
                    u8::MAX
                };
                expected
            ],
            darkness_age,
            gusts: vec![GustCell::default(); expected],
            atmosphere: snapshot.atmosphere.clone(),
            fresh: true,
        })
    }

    fn column_from_generated(
        &self,
        generated: &GeneratedChunk,
        index: i64,
        first: i64,
        new_width: usize,
    ) -> Result<ChunkColumn, WorldError> {
        if generated.index != index
            || generated.width != self.chunk_width
            || generated.height != self.height
            || generated.cells.len() != self.chunk_width * self.height
        {
            return Err(WorldError(
                "generated chunk does not match the active span".into(),
            ));
        }
        let expected = self.chunk_width * self.height;
        let mut support_age = vec![0; expected];
        if let Some(tree) = self.rules.id("tree") {
            let local = (index - first) as usize;
            for y in 0..self.height {
                for x in 0..self.chunk_width {
                    let source = y * self.chunk_width + x;
                    if generated.cells[source].element == tree {
                        let grid_index = y * new_width + local * self.chunk_width + x;
                        support_age[source] = (grid_index % 64) as u8;
                    }
                }
            }
        }
        Ok(ChunkColumn {
            cells: generated.cells.clone(),
            support_age,
            light: vec![
                crate::world::daylight_sky(self.rules.daylight.cycle_ticks, self.day_phase,);
                expected
            ],
            darkness_age: vec![0; expected],
            gusts: vec![GustCell::default(); expected],
            atmosphere: vec![self.ambient_atmosphere(); self.atmosphere_columns()],
            fresh: true,
        })
    }

    fn atmosphere_columns(&self) -> usize {
        self.atmosphere_height * (self.chunk_width / ATMOSPHERE_CELL_SIZE)
    }

    fn chunk_atmosphere(&self, local: usize) -> Vec<AtmosphereCell> {
        let ax_count = self.chunk_width / ATMOSPHERE_CELL_SIZE;
        let ax0 = local * ax_count;
        let mut atmosphere = Vec::with_capacity(self.atmosphere_height * ax_count);
        for y in 0..self.atmosphere_height {
            let start = y * self.atmosphere_width + ax0;
            atmosphere.extend_from_slice(&self.atmosphere[start..start + ax_count]);
        }
        atmosphere
    }

    fn ambient_atmosphere(&self) -> AtmosphereCell {
        AtmosphereCell {
            temperature: self.ambient_temperature,
            humidity: self.ambient_humidity,
            pressure: 0.0,
            oxygen: self.ambient_oxygen,
        }
    }

    fn local_chunk(&self, index: i64) -> Option<usize> {
        let count = self.resident_chunk_count()? as i64;
        (self.chunk_origin..self.chunk_origin + count)
            .contains(&index)
            .then_some((index - self.chunk_origin) as usize)
    }

    fn partition_bodies(&self, first: i64, last: i64) -> (Vec<DormantBody>, Vec<DormantBody>) {
        let mut kept = Vec::new();
        let mut dormant = Vec::new();
        for body in &self.bodies {
            if body.members.is_empty() {
                continue;
            }
            let stored = dormant_body_from(self, body);
            if body_covers(&stored, first, last) {
                kept.push(stored);
            } else {
                dormant.push(stored);
            }
        }
        (kept, dormant)
    }

    fn apply_span(&mut self, plan: SpanPlan) -> Result<(), WorldError> {
        let chunk_width = self.chunk_width;
        let count = plan.columns.len();
        let new_width = count
            .checked_mul(chunk_width)
            .ok_or_else(|| WorldError("active span width overflow".into()))?;
        let expected = chunk_width * self.height;
        if plan.columns.iter().any(|column| {
            column.cells.len() != expected
                || column.support_age.len() != expected
                || column.light.len() != expected
                || column.darkness_age.len() != expected
                || column.gusts.len() != expected
        }) {
            return Err(WorldError(
                "active span column has the wrong cell count".into(),
            ));
        }
        let cell_count = new_width
            .checked_mul(self.height)
            .filter(|count| *count <= 16_777_216)
            .ok_or_else(|| WorldError("active span exceeds the 16,777,216-cell limit".into()))?;
        let new_atmosphere_width = new_width.div_ceil(ATMOSPHERE_CELL_SIZE);
        let mut cells = vec![Cell::default(); cell_count];
        let mut support_age = vec![0; cell_count];
        let mut light = vec![0; cell_count];
        let mut darkness_age = vec![0; cell_count];
        let mut gusts = vec![GustCell::default(); cell_count];
        let mut atmosphere =
            vec![self.ambient_atmosphere(); new_atmosphere_width * self.atmosphere_height];
        let ax_count = chunk_width / ATMOSPHERE_CELL_SIZE;
        for (chunk_index, column) in plan.columns.iter().enumerate() {
            let x0 = chunk_index * chunk_width;
            for y in 0..self.height {
                let source = y * chunk_width;
                let destination = y * new_width + x0;
                cells[destination..destination + chunk_width]
                    .copy_from_slice(&column.cells[source..source + chunk_width]);
                support_age[destination..destination + chunk_width]
                    .copy_from_slice(&column.support_age[source..source + chunk_width]);
                light[destination..destination + chunk_width]
                    .copy_from_slice(&column.light[source..source + chunk_width]);
                darkness_age[destination..destination + chunk_width]
                    .copy_from_slice(&column.darkness_age[source..source + chunk_width]);
                gusts[destination..destination + chunk_width]
                    .copy_from_slice(&column.gusts[source..source + chunk_width]);
            }
            let ax0 = chunk_index * ax_count;
            for y in 0..self.atmosphere_height {
                let source = y * ax_count;
                let destination = y * new_atmosphere_width + ax0;
                atmosphere[destination..destination + ax_count]
                    .copy_from_slice(&column.atmosphere[source..source + ax_count]);
            }
        }
        self.width = new_width;
        self.cells = cells;
        self.tree_support_age = support_age;
        self.light = light;
        self.darkness_age = darkness_age;
        self.light_cursor = 0;
        self.light_scan_phase = self.day_phase;
        self.gusts = gusts;
        self.atmosphere = atmosphere;
        self.atmosphere_width = new_atmosphere_width;
        self.atmosphere_cursor = 0;
        self.atmosphere_credit = 0;
        self.chunk_origin = plan.first;
        self.scheduler = Scheduler::new(cell_count);
        self.bodies.clear();
        self.dormant_chunks = plan.snapshots;
        self.dormant_bodies = plan.dormant_bodies;
        self.install_bodies(plan.kept_bodies, plan.first);
        self.dirty_regions = crate::dirty::DirtyRegions::new(new_width, self.height);
        for (global_x, y) in plan.pending_dirty {
            let local_x = global_x - plan.first * chunk_width as i64;
            if local_x >= 0 && (local_x as usize) < new_width {
                self.dirty_regions.mark_cell(local_x as usize, y);
            }
        }
        for (chunk_index, column) in plan.columns.iter().enumerate() {
            if !column.fresh {
                continue;
            }
            let x0 = chunk_index * chunk_width;
            for y in 0..self.height {
                for x in 0..chunk_width {
                    self.dirty_regions.mark_cell(x0 + x, y);
                }
            }
        }
        Ok(())
    }

    fn install_bodies(&mut self, bodies: Vec<DormantBody>, first: i64) {
        for body in bodies {
            let Some(members) = self.body_members(&body, first) else {
                self.dormant_bodies.push(body);
                continue;
            };
            let id = self.bodies.len() as u32 + 1;
            if id == 0 {
                self.dormant_bodies.push(body);
                continue;
            }
            for &index in &members {
                self.cells[index].body_id = id;
            }
            self.bodies.push(Body {
                anchor: members.iter().copied().min().unwrap_or(0),
                members,
                hull_cells: body.hull_cells,
                bob_phase: body.bob_phase,
                active: body.active,
            });
        }
    }

    fn body_members(&self, body: &DormantBody, first: i64) -> Option<Vec<usize>> {
        let mut members = Vec::with_capacity(body.members.len());
        for &(chunk, local) in &body.members {
            let local = local as usize;
            let x = local % self.chunk_width;
            let y = local / self.chunk_width;
            if y >= self.height || chunk < first {
                return None;
            }
            let new_x = (chunk - first) as usize * self.chunk_width + x;
            if new_x >= self.width {
                return None;
            }
            members.push(y * self.width + new_x);
        }
        (!members.is_empty()).then_some(members)
    }
}

fn body_chunk_indices<'a>(world: &'a World, body: &'a Body) -> impl Iterator<Item = i64> + 'a {
    body.members.iter().map(move |index| {
        let x = index % world.width;
        world.chunk_origin + (x / world.chunk_width) as i64
    })
}

fn dormant_body_from(world: &World, body: &Body) -> DormantBody {
    DormantBody {
        members: body
            .members
            .iter()
            .map(|index| {
                let x = index % world.width;
                let y = index / world.width;
                let chunk = world.chunk_origin + (x / world.chunk_width) as i64;
                let local_x = x % world.chunk_width;
                (chunk, (y * world.chunk_width + local_x) as u32)
            })
            .collect(),
        hull_cells: body.hull_cells,
        bob_phase: body.bob_phase,
        active: body.active,
    }
}

fn body_covers(body: &DormantBody, first: i64, last: i64) -> bool {
    body.members
        .iter()
        .all(|(chunk, _)| (first..=last).contains(chunk))
}

fn cached_chunk(
    index: i64,
    generated: &mut BTreeMap<i64, GeneratedChunk>,
    generate: &mut impl FnMut(i64) -> Result<GeneratedChunk, WorldError>,
) -> Result<GeneratedChunk, WorldError> {
    if let Some(chunk) = generated.get(&index) {
        return Ok(chunk.clone());
    }
    let chunk = generate(index)?;
    generated.insert(index, chunk.clone());
    Ok(chunk)
}

fn div_floor(value: i64, divisor: i64) -> i64 {
    let quotient = value / divisor;
    let remainder = value % divisor;
    if remainder < 0 {
        quotient - 1
    } else {
        quotient
    }
}

fn encode_cells(cells: &[Cell]) -> Result<Vec<u8>, WorldError> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < cells.len() {
        let cell = cells[index];
        if cell.element > u16::MAX as usize {
            return Err(WorldError(
                "dormant chunk element id does not fit in 16 bits".into(),
            ));
        }
        let mut len = 1;
        while index + len < cells.len() && cells[index + len] == cell && len < u16::MAX as usize {
            len += 1;
        }
        out.extend_from_slice(&(cell.element as u16).to_le_bytes());
        out.extend_from_slice(&cell.age.to_le_bytes());
        out.extend_from_slice(&cell.burial_age.to_le_bytes());
        out.push(cell.moisture);
        out.push(cell.burn_progress);
        out.extend_from_slice(&(len as u16).to_le_bytes());
        index += len;
    }
    Ok(out)
}

fn decode_cells(bytes: &[u8], expected: usize) -> Result<Vec<Cell>, WorldError> {
    let mut cells = Vec::with_capacity(expected);
    let mut offset = 0;
    while offset + 10 <= bytes.len() && cells.len() < expected {
        let element = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
        let age = u16::from_le_bytes(bytes[offset + 2..offset + 4].try_into().unwrap());
        let burial_age = u16::from_le_bytes(bytes[offset + 4..offset + 6].try_into().unwrap());
        let moisture = bytes[offset + 6];
        let burn_progress = bytes[offset + 7];
        let len = u16::from_le_bytes(bytes[offset + 8..offset + 10].try_into().unwrap()) as usize;
        offset += 10;
        if len == 0 || cells.len() + len > expected {
            return Err(WorldError("dormant chunk cell run is invalid".into()));
        }
        cells.extend(std::iter::repeat_n(
            Cell {
                element,
                age,
                burial_age,
                moisture,
                burn_progress,
                body_id: 0,
            },
            len,
        ));
    }
    if cells.len() != expected {
        return Err(WorldError(
            "dormant chunk cell payload has the wrong length".into(),
        ));
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{ENDLESS_CHUNK_WIDTH, Level, LevelMode, Preset};
    use crate::{DEFAULT_RULES, Rules};

    fn endless_sized(width: usize, height: usize) -> (Rules, Level, World) {
        let rules = Rules::load(DEFAULT_RULES).unwrap();
        let mut level = Preset::DryForest.level(&rules, width, height);
        level.mode = LevelMode::Endless;
        level.fit_window = false;
        level.spark = false;
        level.boat = false;
        level.amounts.clear();
        level.amounts.insert("stone".into(), 100);
        let world = level.generate(rules.clone()).unwrap();
        (rules, level, world)
    }

    fn endless() -> (Rules, Level, World) {
        endless_sized(128, 32)
    }

    fn retain(
        world: &mut World,
        rules: &Rules,
        level: &Level,
        global_camera_x: i64,
        view_width: usize,
    ) -> SpanAdjustment {
        world
            .retain_active_span(global_camera_x, view_width, |index| {
                level
                    .generate_chunk(index, rules)
                    .map_err(|error| WorldError(error.to_string()))
            })
            .unwrap()
    }

    fn chunk_cells(world: &World, index: i64) -> Vec<Cell> {
        let local = (index - world.chunk_origin()) as usize * ENDLESS_CHUNK_WIDTH;
        let mut cells = Vec::new();
        for y in 0..world.height() {
            let start = y * world.width() + local;
            cells.extend_from_slice(&world.cells()[start..start + ENDLESS_CHUNK_WIDTH]);
        }
        cells
    }

    #[test]
    fn fixed_levels_ignore_span_retention_and_keep_stepping() {
        let rules = Rules::load(DEFAULT_RULES).unwrap();
        let level = Preset::DryForest.level(&rules, 64, 48);
        let mut stepped = level.generate(rules.clone()).unwrap();
        let mut retained = level.generate(rules).unwrap();
        assert!(!stepped.chunk_span_enabled());
        for _ in 0..12 {
            stepped.step(200);
            retained
                .retain_active_span(0, 16, |_| {
                    unreachable!("fixed worlds do not generate chunks")
                })
                .unwrap();
            retained.step(200);
        }
        assert_eq!(retained.width(), 64);
        assert_eq!(retained.dormant_chunk_count(), 0);
        assert_eq!(retained.cells(), stepped.cells());
    }

    #[test]
    fn dormant_boundary_edit_survives_travel_in_both_directions_at_native_and_web_sizes() {
        // Native defaults are 400x250. The web smoke viewport produces a smaller fitted world;
        // 336x200 exercises that browser-sized span without relying on display geometry.
        for (width, height) in [(400, 250), (336, 200)] {
            let (rules, level, mut world) = endless_sized(width, height);
            world.set_active_margin_chunks(0);
            let stone = world.rules().id("stone").unwrap();
            let edited = world.chunk_origin();
            let local_x = ENDLESS_CHUNK_WIDTH - 1;
            let local_y = 8;
            world.paint(local_x, local_y, stone);
            let expected = chunk_cells(&world, edited);
            assert_eq!(
                expected[local_y * ENDLESS_CHUNK_WIDTH + local_x].element,
                stone,
                "edited boundary cell must be present before travel ({width}x{height})"
            );

            for travel_chunks in [12, -12] {
                let away = (edited + travel_chunks) * ENDLESS_CHUNK_WIDTH as i64;
                retain(&mut world, &rules, &level, away, ENDLESS_CHUNK_WIDTH);
                assert!(!world.chunk_is_resident(edited));
                assert!(world.dormant_chunk_count() >= 1);
                retain(
                    &mut world,
                    &rules,
                    &level,
                    edited * ENDLESS_CHUNK_WIDTH as i64,
                    ENDLESS_CHUNK_WIDTH,
                );
                assert!(world.chunk_is_resident(edited));
                assert_eq!(
                    chunk_cells(&world, edited),
                    expected,
                    "chunk {edited} changed after traveling {travel_chunks} chunks at {width}x{height}"
                );
            }
        }
    }

    #[test]
    fn untouched_chunks_regenerate_and_resident_width_stays_bounded() {
        let (rules, level, mut world) = endless();
        world.set_active_margin_chunks(1);
        let untouched = world.chunk_origin() + 1;
        let expected = level.generate_chunk(untouched, &rules).unwrap();
        let far = (world.chunk_origin() + 30) * ENDLESS_CHUNK_WIDTH as i64;
        retain(&mut world, &rules, &level, far, ENDLESS_CHUNK_WIDTH);
        assert_eq!(world.dormant_chunk_count(), 0);
        assert!(world.width() <= 4 * ENDLESS_CHUNK_WIDTH);
        for step in 1..24 {
            retain(
                &mut world,
                &rules,
                &level,
                far + step * ENDLESS_CHUNK_WIDTH as i64,
                ENDLESS_CHUNK_WIDTH,
            );
            assert!(
                world.width() <= 4 * ENDLESS_CHUNK_WIDTH,
                "resident width grew to {}",
                world.width()
            );
        }
        retain(
            &mut world,
            &rules,
            &level,
            untouched * ENDLESS_CHUNK_WIDTH as i64,
            ENDLESS_CHUNK_WIDTH,
        );
        let restored = chunk_cells(&world, untouched);
        assert_eq!(restored, expected.cells);
        assert_eq!(world.dormant_chunk_count(), 0);
    }

    #[test]
    fn active_edge_does_not_spill_fire_into_a_dormant_neighbor() {
        let (rules, level, mut world) = endless();
        world.set_active_margin_chunks(0);
        let origin = world.chunk_origin();
        retain(
            &mut world,
            &rules,
            &level,
            origin * ENDLESS_CHUNK_WIDTH as i64,
            ENDLESS_CHUNK_WIDTH,
        );
        let fire = world.rules().id("fire").unwrap();
        world.paint(ENDLESS_CHUNK_WIDTH - 1, 6, fire);
        for _ in 0..20 {
            world.step(world.width() * world.height());
        }
        let neighbor = origin + 1;
        retain(
            &mut world,
            &rules,
            &level,
            neighbor * ENDLESS_CHUNK_WIDTH as i64,
            ENDLESS_CHUNK_WIDTH,
        );
        let generated = level.generate_chunk(neighbor, &rules).unwrap();
        assert_eq!(chunk_cells(&world, neighbor), generated.cells);
        assert!(
            chunk_cells(&world, neighbor)
                .iter()
                .all(|cell| cell.element != fire)
        );
    }

    #[test]
    fn modified_snapshot_is_smaller_than_raw_cells_and_prepend_tracks_origin() {
        let (rules, level, mut world) = endless();
        world.set_active_margin_chunks(0);
        let stone = world.rules().id("stone").unwrap();
        let edited = world.chunk_origin();
        world.paint(4, 4, stone);
        let origin = world.chunk_origin();
        retain(
            &mut world,
            &rules,
            &level,
            (edited + 8) * ENDLESS_CHUNK_WIDTH as i64,
            ENDLESS_CHUNK_WIDTH,
        );
        let raw_bytes = ENDLESS_CHUNK_WIDTH * world.height() * std::mem::size_of::<Cell>();
        assert!(world.dormant_chunk_count() >= 1);
        assert!(world.dormant_payload_bytes() < raw_bytes);
        let chunk = level.generate_chunk(origin - 1, &rules).unwrap();
        let before = world.chunk_origin();
        world.extend_horizontal(&chunk, true).unwrap();
        assert_eq!(world.chunk_origin(), before - 1);
    }

    #[test]
    fn derived_light_alone_does_not_create_dormant_snapshots() {
        let (rules, level, mut world) = endless();
        world.set_active_margin_chunks(0);
        let origin = world.chunk_origin();
        let index = 4 + 4 * world.width();
        world.light[index] = if world.light[index] == 0 { u8::MAX } else { 0 };
        retain(
            &mut world,
            &rules,
            &level,
            (origin + 8) * ENDLESS_CHUNK_WIDTH as i64,
            ENDLESS_CHUNK_WIDTH,
        );
        assert_eq!(world.dormant_chunk_count(), 0);
        assert_eq!(world.dormant_payload_bytes(), 0);
    }

    #[test]
    fn dormant_living_darkness_exposure_restores_compactly_without_catch_up() {
        let (rules, level, mut world) = endless();
        world.set_active_margin_chunks(0);
        let origin = world.chunk_origin();
        let plant = world.rules().id("plant").unwrap();
        world.paint(4, 4, plant);
        let index = 4 + 4 * world.width();
        world.darkness_age[index] = 12_345;
        let baseline = if world.rules.daylight.enabled {
            crate::world::daylight_sky(world.rules.daylight.cycle_ticks, world.day_phase)
        } else {
            u8::MAX
        };
        world.light[index] = if baseline == 0 { u8::MAX } else { 0 };
        retain(
            &mut world,
            &rules,
            &level,
            (origin + 8) * ENDLESS_CHUNK_WIDTH as i64,
            ENDLESS_CHUNK_WIDTH,
        );
        assert_eq!(world.dormant_chunk_count(), 1);
        let raw_bytes = ENDLESS_CHUNK_WIDTH * world.height() * std::mem::size_of::<Cell>();
        let payload_bytes = world.dormant_payload_bytes();
        assert!(
            payload_bytes < raw_bytes / 2,
            "one living exposure should keep the payload compact ({payload_bytes} bytes vs {raw_bytes} raw bytes)"
        );
        retain(
            &mut world,
            &rules,
            &level,
            origin * ENDLESS_CHUNK_WIDTH as i64,
            ENDLESS_CHUNK_WIDTH,
        );
        let restored = 4 + 4 * world.width();
        assert_eq!(world.cells[restored].element, plant);
        assert_eq!(world.darkness_age[restored], 12_345);
        assert_eq!(world.light[restored], baseline);
    }

    #[test]
    fn a_boat_on_the_chunk_edge_is_kept_whole_and_restored() {
        let (rules, level, mut world) = endless();
        world.set_active_margin_chunks(0);
        let air = world.rules().id("air").unwrap();
        for x in 110..140 {
            for y in 4..12 {
                world.paint(x, y, air);
            }
        }
        assert!(world.place_boat(126, 8), "boat should fit on cleared air");
        assert!(world.cells()[8 * world.width() + 126].is_in_body());
        let boundary = world.chunk_origin();
        retain(
            &mut world,
            &rules,
            &level,
            (boundary + 1) * ENDLESS_CHUNK_WIDTH as i64,
            ENDLESS_CHUNK_WIDTH,
        );
        assert!(world.chunk_is_resident(boundary));
        assert!(world.chunk_is_resident(boundary + 1));
        assert!(world.cells().iter().any(|cell| cell.is_in_body()));
        retain(
            &mut world,
            &rules,
            &level,
            (boundary + 10) * ENDLESS_CHUNK_WIDTH as i64,
            ENDLESS_CHUNK_WIDTH,
        );
        assert!(world.cells().iter().all(|cell| !cell.is_in_body()));
        retain(
            &mut world,
            &rules,
            &level,
            boundary * ENDLESS_CHUNK_WIDTH as i64,
            ENDLESS_CHUNK_WIDTH,
        );
        assert!(world.chunk_is_resident(boundary));
        assert!(world.chunk_is_resident(boundary + 1));
        assert!(
            world.cells()[8 * world.width() + 126].is_in_body(),
            "the restored hull should still be a body"
        );
    }
}
