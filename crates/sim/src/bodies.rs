//! Small ID-tagged rigid groups for the first cohesive-body prototype.
use crate::{Cell, World};

pub(super) const MAX_BODY_CELLS: usize = 64;
const MAX_BODIES: usize = 4_096;
const WOOD_MASS: usize = 1;
const SAND_MASS: usize = 5;
const WATER_MASS: usize = 4;
const WOOD_DENSITY_REFERENCE: f32 = 90.0;
const SAND_DENSITY_REFERENCE: f32 = 220.0;
const WATER_DENSITY_REFERENCE: f32 = 150.0;

#[derive(Clone, Debug, Default)]
pub(super) struct Body {
    pub(super) members: Vec<usize>,
    pub(super) anchor: usize,
    pub(super) hull_cells: usize,
    pub(super) bob_phase: u8,
    pub(super) active: bool,
}

impl World {
    /// Places a small wooden boat with its keel centered at `(x, y)`.
    /// The stamp may cover only air or water. It does not consume either material.
    pub fn place_boat(&mut self, x: usize, y: usize) -> bool {
        let Some(wood) = self.rules.id("wood") else {
            return false;
        };
        if x >= self.width || y >= self.height || self.bodies.len() >= MAX_BODIES {
            return false;
        }
        let Some(water_row) = y.checked_add(1).filter(|row| *row < self.height) else {
            return false;
        };
        let water_support = (-3isize..=3)
            .filter_map(|dx| x.checked_add_signed(dx))
            .filter(|column| *column < self.width)
            .filter(|column| {
                self.rules.elements[self.cells[water_row * self.width + column].element].name
                    == "water"
            })
            .count();
        if water_support < 3 {
            return false;
        }
        let mut indices = Vec::with_capacity(23);
        for (dy, span) in [(-2isize, 3isize), (-1, 4), (0, 3)] {
            let Some(row) = y.checked_add_signed(dy).filter(|row| *row < self.height) else {
                return false;
            };
            for dx in -span..=span {
                let Some(column) = x
                    .checked_add_signed(dx)
                    .filter(|column| *column < self.width)
                else {
                    return false;
                };
                let index = row * self.width + column;
                let element = self.cells[index].element;
                let name = self.rules.elements[element].name.as_str();
                if !matches!(name, "air" | "water") || self.cells[index].body_id != 0 {
                    return false;
                }
                indices.push(index);
            }
        }
        if indices.len() > MAX_BODY_CELLS {
            return false;
        }
        let id = self.bodies.len() as u32 + 1;
        let anchor = *indices.iter().min().unwrap_or(&0);
        for &index in &indices {
            self.cells[index] = Cell {
                element: wood,
                body_id: id,
                ..Cell::default()
            };
            self.mark_cell_dirty(index);
            self.scheduler.enqueue_focus(index);
        }
        self.bodies.push(Body {
            members: indices,
            anchor,
            hull_cells: 23,
            bob_phase: 0,
            active: true,
        });
        true
    }

    pub(super) fn body_work_cost(&self, index: usize) -> usize {
        let id = self.cells[index].body_id;
        if id == 0 {
            return 1;
        }
        self.bodies
            .get(id as usize - 1)
            .filter(|body| body.active && body.anchor == index)
            .map_or(1, |body| body.members.len().max(1))
    }

    pub(super) fn detach_body_member(&mut self, index: usize) {
        let id = self.cells[index].body_id;
        if id == 0 {
            return;
        }
        let body_index = id as usize - 1;
        let Some(body) = self.bodies.get_mut(body_index) else {
            return;
        };
        body.members.retain(|member| *member != index);
        body.active = !body.members.is_empty();
        body.anchor = body.members.iter().copied().min().unwrap_or(0);
        let has_hull = body
            .members
            .iter()
            .any(|member| self.rules.elements[self.cells[*member].element].name == "wood");
        if !has_hull {
            for member in std::mem::take(&mut body.members) {
                self.cells[member].body_id = 0;
            }
            body.active = false;
        }
    }

    pub(super) fn attach_cargo(&mut self, index: usize) {
        if self.rules.elements[self.cells[index].element].name != "sand" {
            return;
        }
        let Some(id) = self.neighbor_body_id(index) else {
            return;
        };
        let Some(body) = self.bodies.get_mut(id as usize - 1) else {
            return;
        };
        if body.active && body.members.len() < MAX_BODY_CELLS {
            self.cells[index].body_id = id;
            body.members.push(index);
            body.anchor = body.anchor.min(index);
        }
    }

    fn neighbor_body_id(&self, index: usize) -> Option<u32> {
        let x = index % self.width;
        let y = index / self.width;
        for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
            let Some(nx) = x
                .checked_add_signed(dx)
                .filter(|column| *column < self.width)
            else {
                continue;
            };
            let Some(ny) = y.checked_add_signed(dy).filter(|row| *row < self.height) else {
                continue;
            };
            let id = self.cells[ny * self.width + nx].body_id;
            if id != 0 {
                return Some(id);
            }
        }
        None
    }

    /// Scale a ruleset density into the boat's existing material-mass units.
    pub(super) fn body_material_density(&self, element: usize) -> f32 {
        let element = &self.rules.elements[element];
        let density = f32::from(element.density);
        match element.name.as_str() {
            "wood" => WOOD_MASS as f32 * density / WOOD_DENSITY_REFERENCE,
            "sand" => SAND_MASS as f32 * density / SAND_DENSITY_REFERENCE,
            "water" => density / WATER_DENSITY_REFERENCE,
            _ => density / 100.0,
        }
    }

    pub(super) fn update_body_at(&mut self, index: usize) {
        let id = self.cells[index].body_id;
        let Some(body_index) = id.checked_sub(1).map(|id| id as usize) else {
            return;
        };
        let Some(body) = self.bodies.get(body_index) else {
            return;
        };
        if !body.active || body.anchor != index {
            return;
        }
        let mut mass = 0.0f32;
        let mut hull_cells = 0usize;
        let mut water_contacts = 0usize;
        let water_density = self
            .rules
            .id("water")
            .map_or(1.0, |water| self.body_material_density(water));
        let mut water_above = false;
        for &member in &body.members {
            let x = member % self.width;
            let y = member / self.width;
            let element_name = self.rules.elements[self.cells[member].element]
                .name
                .as_str();
            if element_name == "wood" {
                hull_cells += 1;
            }
            mass += self.body_material_density(self.cells[member].element);
            for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
                let Some(nx) = x
                    .checked_add_signed(dx)
                    .filter(|column| *column < self.width)
                else {
                    continue;
                };
                let Some(ny) = y.checked_add_signed(dy).filter(|row| *row < self.height) else {
                    continue;
                };
                let neighbor = ny * self.width + nx;
                if self.cells[neighbor].body_id == id {
                    continue;
                }
                if self.rules.elements[self.cells[neighbor].element].name == "water"
                    && element_name == "wood"
                {
                    water_contacts += usize::from(dy == 1);
                    water_above |= dy == -1;
                }
            }
        }

        let wind = self.wind_at(index);
        // Water directly below the hull is the displaced volume. Losing wood scales that
        // volume down, so even a loaded boat can sink through a hole in its planks.
        let capacity =
            water_contacts as f32 * WATER_MASS as f32 * water_density * hull_cells as f32
                / body.hull_cells.max(1) as f32;
        let bob_phase = {
            let body = &mut self.bodies[body_index];
            body.bob_phase = body.bob_phase.wrapping_add(1) % 64;
            body.bob_phase
        };
        let buoyancy = if capacity == 0.0 || mass > capacity {
            1
        } else if water_above && mass * 2.0 < capacity {
            -1
        } else {
            0
        };
        let vertical = if buoyancy != 0 {
            buoyancy
        } else if bob_phase == 32 {
            -1
        } else if bob_phase == 0 {
            1
        } else {
            0
        };
        if vertical != 0 && self.translate_body(body_index, 0, vertical) {
            return;
        }
        if wind.strength > 0 && wind.direction != crate::WindDirection::Calm {
            let dx = wind.direction.horizontal() as isize;
            if self.translate_body(body_index, dx, 0) {
                return;
            }
        }
        // Slow deterministic current from horizontally adjacent water. It is intentionally
        // a local nudge, not a fluid velocity solver.
        let left = index % self.width > 0
            && self.rules.elements[self.cells[index - 1].element].name == "water";
        let right = index % self.width + 1 < self.width
            && self.rules.elements[self.cells[index + 1].element].name == "water";
        if left != right {
            self.translate_body(body_index, if left { -1 } else { 1 }, 0);
        }
    }

    fn translate_body(&mut self, body_index: usize, dx: isize, dy: isize) -> bool {
        let id = body_index as u32 + 1;
        let Some(body) = self.bodies.get(body_index) else {
            return false;
        };
        if !body.active || body.members.is_empty() {
            return false;
        }
        for &source in &body.members {
            let x = source % self.width;
            let y = source / self.width;
            let Some(nx) = x
                .checked_add_signed(dx)
                .filter(|column| *column < self.width)
            else {
                return false;
            };
            let Some(ny) = y.checked_add_signed(dy).filter(|row| *row < self.height) else {
                return false;
            };
            let target = ny * self.width + nx;
            if self.cells[target].body_id != id {
                let name = self.rules.elements[self.cells[target].element]
                    .name
                    .as_str();
                if !matches!(name, "air" | "water") {
                    return false;
                }
            }
        }

        let mut members = std::mem::take(&mut self.bodies[body_index].members);
        members.sort_unstable_by_key(|member| {
            let x = *member % self.width;
            let y = *member / self.width;
            match (dx, dy) {
                (dx, 0) if dx > 0 => (usize::MAX - y, usize::MAX - x),
                (dx, 0) if dx < 0 => (y, x),
                (0, dy) if dy > 0 => (usize::MAX - y, x),
                (0, dy) if dy < 0 => (y, x),
                _ => (y, x),
            }
        });
        for &source in &members {
            let x = source % self.width;
            let y = source / self.width;
            let target = y.checked_add_signed(dy).unwrap_or(y) * self.width
                + x.checked_add_signed(dx).unwrap_or(x);
            if self.cells[target].body_id != id {
                self.cells.swap(source, target);
                self.mark_cell_dirty(source);
                self.mark_cell_dirty(target);
            }
        }
        let body = &mut self.bodies[body_index];
        for member in &mut members {
            let x = *member % self.width;
            let y = *member / self.width;
            *member = y.checked_add_signed(dy).unwrap_or(y) * self.width
                + x.checked_add_signed(dx).unwrap_or(x);
        }
        body.anchor = members.iter().copied().min().unwrap_or(0);
        body.members = members;
        true
    }
}
