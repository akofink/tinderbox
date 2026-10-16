//! Headless falling-material simulation facade.
//!
//! [`World`] owns the grid and deterministic scheduler. Create one with a validated
//! [`Rules`] set, then use [`World::step`] with a per-update work budget and [`World::paint`]
//! for player actions. Read world dimensions, cells, rules, and deferred player-focus work via
//! the immutable accessors; drain [`World::pop_dirty_region`] under a presentation budget when
//! synchronizing a renderer. Rule parsing, element types, commands, storage, dirty tracking, and
//! scheduling are kept in focused sibling modules so new interactions do not expand this facade.
mod bodies;
mod canopy;
mod commands;
mod dirty;
mod ecology;
mod element;
pub mod level;
mod rule_controls;
mod rules;
mod scheduler;
mod span;
mod step;
mod world;

#[cfg(test)]
mod tests;

pub use dirty::DirtyRegion;
pub use element::{Cell, Element, Motion, Wind, WindDirection};
pub use level::{ENDLESS_CHUNK_WIDTH, GeneratedChunk, LevelMode};
pub use rules::{
    AtmosphereRules, BurialRule, DEFAULT_RULES, EcologyRules, Interaction, Rules, RulesError,
};
pub use span::SpanAdjustment;
pub use world::{AtmosphereCell, World, WorldError};
