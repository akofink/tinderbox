//! Player commands that modify world state.
use crate::{Cell, World};

impl World {
    /// Clears all cells and restores the seeded random sequence.
    pub fn reset(&mut self, seed: u64) {
        let air = self.rules.id("air").unwrap_or(0);
        self.cells.fill(Cell {
            element: air,
            age: 0,
        });
        self.scheduler.reset();
        self.gusts.fill(Default::default());
        self.rng = seed.max(1);
    }

    /// Paints an in-bounds cell and prioritizes it and its neighbors for simulation.
    /// Invalid coordinates or element IDs are ignored.
    pub fn paint(&mut self, x: usize, y: usize, element: usize) {
        if element >= self.rules.elements.len() {
            return;
        }
        let Some(index) = y
            .checked_mul(self.width)
            .and_then(|i| i.checked_add(x))
            .filter(|_| x < self.width && y < self.height)
        else {
            return;
        };
        self.cells[index] = Cell { element, age: 0 };
        self.scheduler.enqueue_focus(index);
        let x = index % self.width;
        let y = index / self.width;
        for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
            if let Some(nx) = x.checked_add_signed(dx).filter(|v| *v < self.width)
                && let Some(ny) = y.checked_add_signed(dy).filter(|v| *v < self.height)
            {
                self.scheduler.enqueue_focus(ny * self.width + nx);
            }
        }
    }
}
