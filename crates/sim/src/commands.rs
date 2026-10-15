//! Player commands that modify world state.
use crate::{Cell, World};

impl World {
    /// Clears all cells and restores the seeded random sequence.
    pub fn reset(&mut self, seed: u64) {
        let air = self.rules.id("air").unwrap_or(0);
        self.cells.fill(Cell {
            element: air,
            age: 0,
            moisture: 0,
            burn_progress: 0,
            body_id: 0,
        });
        self.tree_support_age.fill(0);
        self.scheduler.reset();
        self.bodies.clear();
        self.gusts.fill(Default::default());
        self.atmosphere.fill(crate::AtmosphereCell {
            temperature: self.ambient_temperature,
            humidity: self.ambient_humidity,
            pressure: 0.0,
            oxygen: self.ambient_oxygen,
        });
        self.atmosphere_cursor = 0;
        self.atmosphere_credit = 0;
        self.dirty_regions.mark_all();
        self.dormant_chunks.clear();
        self.dormant_bodies.clear();
        self.rng = seed.max(1);
        self.decay_rng = (seed ^ 0xDEC4_9EED_51A7_0001).max(1);
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
        if self.cells[index].element != element {
            self.dirty_regions.mark_cell(x, y);
        }
        self.detach_body_member(index);
        self.cells[index] = Cell {
            element,
            age: 0,
            moisture: 0,
            burn_progress: 0,
            body_id: 0,
        };
        self.tree_support_age[index] = (index % 64) as u8;
        self.attach_cargo(index);
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
