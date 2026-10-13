//! Headless falling-material simulation facade.
//!
//! [`World`] owns the grid and deterministic scheduler. Create one with a validated
//! [`Rules`] set, then use [`World::step`] with a per-update work budget and [`World::paint`]
//! for player actions. Read world dimensions, cells, rules, and deferred player-focus work via
//! the immutable accessors. Rule parsing, element types, commands, storage, and scheduling are
//! kept in focused sibling modules so new elements and interactions do not expand this facade.
mod commands;
mod element;
mod rules;
mod scheduler;
mod step;
mod world;

#[cfg(test)]
mod tests;

pub use element::{Cell, Element, Motion, Wind, WindDirection};
pub use rules::{DEFAULT_RULES, Interaction, Rules, RulesError};
pub use world::{World, WorldError};
