//! Tree canopy support and leaf fall.
use crate::World;

impl World {
    pub(super) fn is_ground_element(&self, element: usize) -> bool {
        matches!(
            self.rules.elements[element].name.as_str(),
            "soil" | "wet_soil" | "fertile_soil" | "stone" | "sand"
        )
    }

    /// Whether this tree cell has a living, downhill tree path to stable ground.
    pub(super) fn tree_has_ground_support(&self, index: usize) -> bool {
        let tree = self.rules.id("tree").unwrap_or(usize::MAX);
        let x = (index % self.width) as isize;
        let mut row = index / self.width;
        let mut current = [false; 5];
        current[2] = true;

        loop {
            // Tree crowns are at most three cells wide. Follow same-row branches
            // locally, then allow a one-cell lean as the path descends.
            for _ in 0..2 {
                let prior = current;
                for column in 1..4 {
                    for dx in [-1isize, 1] {
                        let candidate_x = x + column as isize - 2 + dx;
                        if candidate_x >= 0
                            && candidate_x < self.width as isize
                            && prior[column]
                            && self.cells[row * self.width + candidate_x as usize].element == tree
                        {
                            current[(column as isize + dx) as usize] = true;
                        }
                    }
                }
            }

            if row + 1 >= self.height {
                return true;
            }
            let below = row + 1;
            let mut next = [false; 5];
            for (column, connected) in current.iter().copied().enumerate() {
                if !connected {
                    continue;
                }
                let world_x = x + column as isize - 2;
                for dx in [-1isize, 0, 1] {
                    let candidate_x = world_x + dx;
                    if !(0..self.width as isize).contains(&candidate_x) {
                        continue;
                    }
                    let below_index = below * self.width + candidate_x as usize;
                    let element = self.cells[below_index].element;
                    if element == tree {
                        let offset = candidate_x - x;
                        if (-2..=2).contains(&offset) {
                            next[(offset + 2) as usize] = true;
                        }
                    } else {
                        if self.is_ground_element(element) {
                            return true;
                        }
                    }
                }
            }
            if next.iter().all(|connected| !connected) {
                return false;
            }
            current = next;
            row = below;
        }
    }

    /// Remove unsupported tree cells from a generated or resized world.
    pub(super) fn drop_unsupported_canopy(&mut self) {
        let Some(tree) = self.rules.id("tree") else {
            return;
        };
        let Some(deadwood) = self.rules.id("deadwood") else {
            return;
        };
        for index in 0..self.cells.len() {
            if self.cells[index].element == tree && !self.tree_has_ground_support(index) {
                self.set_element_inner(index, deadwood, false);
            }
        }
    }

    /// Drop tree cells disconnected by a trunk's removal, including their unsupported crown.
    pub(super) fn drop_unsupported_canopy_near(&mut self, index: usize) {
        let Some(tree) = self.rules.id("tree") else {
            return;
        };
        let Some(deadwood) = self.rules.id("deadwood") else {
            return;
        };
        let x = index % self.width;
        let y = index / self.width;
        let mut pending = Vec::with_capacity(8);
        for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
            let Some(nx) = x.checked_add_signed(dx).filter(|x| *x < self.width) else {
                continue;
            };
            let Some(ny) = y.checked_add_signed(dy).filter(|y| *y < self.height) else {
                continue;
            };
            pending.push(ny * self.width + nx);
        }
        while let Some(candidate) = pending.pop() {
            if self.cells[candidate].element != tree || self.tree_has_ground_support(candidate) {
                continue;
            }
            self.set_element_inner(candidate, deadwood, false);
            let x = candidate % self.width;
            let y = candidate / self.width;
            for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                let Some(nx) = x.checked_add_signed(dx).filter(|x| *x < self.width) else {
                    continue;
                };
                let Some(ny) = y.checked_add_signed(dy).filter(|y| *y < self.height) else {
                    continue;
                };
                pending.push(ny * self.width + nx);
            }
        }
    }
}
