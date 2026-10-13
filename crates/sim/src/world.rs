//! World storage, construction, and read-only accessors.
use crate::scheduler::Scheduler;
use crate::{Cell, Rules, RulesError};
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

/// Deterministic material grid and bounded-work update state.
pub struct World {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) cells: Vec<Cell>,
    pub(super) rules: Rules,
    pub(super) scheduler: Scheduler,
    pub(super) rng: u64,
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
}
