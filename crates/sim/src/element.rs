//! Data shapes shared by the ruleset and simulation.

/// Movement behavior attached to an element in the ruleset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Motion {
    Static,
    Fall,
    Liquid,
    Gas,
}

/// Element presentation and lifecycle defaults loaded from the ruleset.
#[derive(Clone, Debug)]
pub struct Element {
    pub name: String,
    pub color: [u8; 3],
    pub motion: Motion,
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
