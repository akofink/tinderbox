//! Focused movement, foraging-adjacent habitat, breeding, and fungal growth behavior.
use crate::World;

impl World {
    /// Applies ecology-specific behavior for one scheduled organism update.
    pub(super) fn update_ecology(&mut self, index: usize) -> bool {
        let element = self.cells[index].element;
        let name = self.rules.elements[element].name.as_str();
        match name {
            "beetle" | "termite" => self.update_critter(index, element),
            "fungus" => self.update_fungus(index),
            _ => false,
        }
    }

    fn update_critter(&mut self, index: usize, species: usize) -> bool {
        let should_move = self.random_f32() < self.rules.ecology.critter_move_chance;
        let x = index % self.width;
        let y = index / self.width;
        let air = self.rules.id("air").unwrap_or(usize::MAX);
        let fire = self.rules.id("fire");
        let water = self.rules.id("water");
        let mut candidates = [None; 4];
        let mut candidate_count = 0;
        for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
            let Some(nx) = x.checked_add_signed(dx).filter(|v| *v < self.width) else {
                continue;
            };
            let Some(ny) = y.checked_add_signed(dy).filter(|v| *v < self.height) else {
                continue;
            };
            let target = ny * self.width + nx;
            if self.cells[target].body_id != 0 {
                continue;
            }
            let target_element = self.cells[target].element;
            let meal = self
                .rules
                .interactions
                .iter()
                .enumerate()
                .find(|(_, rule)| {
                    rule.enabled
                        && rule.chance > 0.0
                        && rule.when == species
                        && rule.with == target_element
                })
                .map(|(rank, rule)| (rank, rule.result));
            let food_result = if target_element == air {
                // Walk on supported ground or climb a stable tree/deadwood side.
                let supported_below = ny
                    .checked_add(1)
                    .filter(|floor_y| *floor_y < self.height)
                    .is_some_and(|floor_y| {
                        let floor_index = floor_y * self.width + nx;
                        floor_index != index && self.is_surface(floor_index)
                    });
                let supported_side = [nx.checked_sub(1), (nx + 1 < self.width).then_some(nx + 1)]
                    .into_iter()
                    .flatten()
                    .any(|side_x| {
                        let side = ny * self.width + side_x;
                        let name = self.rules.elements[self.cells[side].element].name.as_str();
                        side != index
                            && matches!(
                                name,
                                "tree"
                                    | "wood"
                                    | "deadwood"
                                    | "stone"
                                    | "soil"
                                    | "wet_soil"
                                    | "fertile_soil"
                            )
                            && self.is_surface(side)
                    });
                if !supported_below && !supported_side {
                    continue;
                }
                None
            } else if let Some((_, result)) = meal {
                // Critters can tunnel through edible matter instead of remaining embedded in it.
                Some(result)
            } else if matches!(
                self.rules.elements[target_element].name.as_str(),
                "wet_soil" | "fertile_soil"
            ) {
                // Decomposers burrow through soft soil when food-rich material packs them in.
                self.rules.id("fertile_soil")
            } else {
                continue;
            };
            let danger = self.nearby_hazard_distance(nx, ny, fire, water);
            let food_distance = meal
                .map(|(rank, _)| rank)
                .or_else(|| {
                    self.neighbors(target)
                        .into_iter()
                        .flatten()
                        .filter_map(|neighbor| {
                            self.rules
                                .interactions
                                .iter()
                                .enumerate()
                                .filter(|(_, rule)| {
                                    rule.enabled
                                        && rule.when == species
                                        && rule.with == self.cells[neighbor].element
                                })
                                .map(|(rank, _)| rank)
                                .min()
                        })
                        .min()
                })
                .unwrap_or(usize::MAX);
            candidates[candidate_count] = Some((target, danger, food_distance, food_result));
            candidate_count += 1;
        }
        let danger_here = self.nearby_hazard_distance(x, y, fire, water);
        let fleeing = danger_here <= 3;
        if should_move && fleeing && self.random_f32() >= self.rules.ecology.flee_chance {
            return false;
        }
        if should_move && fleeing {
            let Some(safest) = candidates[..candidate_count]
                .iter()
                .flatten()
                .map(|(_, distance, _, _)| *distance)
                .max()
            else {
                return false;
            };
            if safest <= danger_here {
                return false;
            }
            for candidate in &mut candidates[..candidate_count] {
                if candidate.is_some_and(|(_, distance, _, _)| distance != safest) {
                    *candidate = None;
                }
            }
        } else if should_move {
            let best_food = candidates[..candidate_count]
                .iter()
                .flatten()
                .map(|(_, _, food_distance, _)| *food_distance)
                .min()
                .unwrap_or(usize::MAX);
            if best_food != usize::MAX {
                for candidate in &mut candidates[..candidate_count] {
                    if candidate.is_some_and(|(_, _, food_distance, _)| food_distance != best_food)
                    {
                        *candidate = None;
                    }
                }
            }
        }
        if should_move
            && !fleeing
            && candidate_count == 0
            && y + 1 < self.height
            && self.cells[index + self.width].element == air
        {
            self.swap_if_empty(index, index + self.width);
            return true;
        }
        let available = candidates[..candidate_count].iter().flatten().count();
        if should_move && available > 0 {
            let choice = self.random_u64() as usize % available;
            let target = candidates[..candidate_count]
                .iter()
                .flatten()
                .nth(choice)
                .expect("choice is bounded by available moves")
                .0;
            let food_result = candidates[..candidate_count]
                .iter()
                .flatten()
                .nth(choice)
                .expect("choice is bounded by available moves")
                .3;
            if let Some(result) = food_result {
                self.set_element(index, result);
                self.set_element(target, species);
                return true;
            }
            if self.swap_if_empty(index, target) {
                return true;
            }
        }
        // Breeding requires both a nearby mate and food, so depleted habitat cannot support growth.
        let neighbors = self.neighbors(index);
        let has_mate = neighbors
            .into_iter()
            .flatten()
            .any(|neighbor| self.cells[neighbor].element == species);
        let has_food = neighbors.into_iter().flatten().any(|neighbor| {
            self.rules.interactions.iter().any(|rule| {
                rule.enabled
                    && rule.chance > 0.0
                    && rule.when == species
                    && rule.with == self.cells[neighbor].element
            })
        });
        if has_mate && has_food && self.random_f32() < self.rules.ecology.breeding_chance {
            for neighbor in self.neighbors(index).into_iter().flatten() {
                if self.cells[neighbor].element == air {
                    let below = neighbor
                        .checked_add(self.width)
                        .filter(|v| *v < self.cells.len());
                    if below.is_some_and(|v| self.cells[v].element != air) {
                        self.set_element(neighbor, species);
                        return true;
                    }
                }
            }
        }
        false
    }

    fn nearby_hazard_distance(
        &self,
        x: usize,
        y: usize,
        fire: Option<usize>,
        water: Option<usize>,
    ) -> usize {
        let mut nearest = usize::MAX;
        for hy in y.saturating_sub(3)..=y.saturating_add(3).min(self.height - 1) {
            for hx in x.saturating_sub(3)..=x.saturating_add(3).min(self.width - 1) {
                let distance = x.abs_diff(hx) + y.abs_diff(hy);
                if distance <= 3 {
                    let element = self.cells[hy * self.width + hx].element;
                    if Some(element) == fire || Some(element) == water {
                        nearest = nearest.min(distance);
                    }
                }
            }
        }
        nearest
    }

    fn update_fungus(&mut self, index: usize) -> bool {
        let x = index % self.width;
        let y = index / self.width;
        let conditions = self.atmosphere_at(x, y).unwrap_or_default();
        let min_humidity = self.rules.ecology.fungus_min_humidity;
        let kill_temperature = self.rules.ecology.fungus_kill_temperature;
        let slow_temperature = self.rules.ecology.fungus_slow_temperature;
        let spread_chance = self.rules.ecology.fungus_spread_chance;
        let starvation_chance = self.rules.ecology.fungus_starvation_chance;
        if conditions.temperature >= kill_temperature {
            if let Some(decay_to) = self.rules.elements[self.cells[index].element].decay_to {
                self.set_element(index, decay_to);
                return true;
            }
        }
        let has_food = self.neighbors(index).into_iter().flatten().any(|neighbor| {
            let cell = self.cells[neighbor];
            let name = self.rules.elements[cell.element].name.as_str();
            matches!(name, "wet_soil" | "fertile_soil")
                || (matches!(name, "deadwood" | "leaf_litter" | "humus") && cell.moisture >= 70)
        });
        if !has_food && self.random_f32() < starvation_chance {
            if let Some(decay_to) = self.rules.elements[self.cells[index].element].decay_to {
                self.set_element(index, decay_to);
                return true;
            }
        }
        if conditions.humidity < min_humidity || conditions.temperature >= slow_temperature {
            return false;
        }
        let air = self.rules.id("air").unwrap_or(usize::MAX);
        let Some(fungus) = self.rules.id("fungus") else {
            return false;
        };
        let mut targets = [None; 4];
        let mut target_count = 0;
        for target in self.neighbors(index).into_iter().flatten() {
            if self.cells[target].element != air {
                continue;
            }
            let tx = target % self.width;
            let ty = target / self.width;
            let supported = ty + 1 < self.height && self.is_surface(target + self.width);
            if !supported {
                continue;
            }
            // Colonize only a damp litter/wood/soil surface, not arbitrary empty terrain.
            let habitat = self.neighbors(target).into_iter().flatten().any(|n| {
                let cell = self.cells[n];
                let matter = self.rules.elements[cell.element].name.as_str();
                matches!(
                    matter,
                    "deadwood" | "leaf_litter" | "humus" | "wet_soil" | "fertile_soil"
                ) && (cell.moisture >= 70 || matches!(matter, "wet_soil" | "fertile_soil"))
            });
            if habitat
                && self
                    .atmosphere_at(tx, ty)
                    .is_some_and(|a| a.humidity >= min_humidity)
            {
                targets[target_count] = Some(target);
                target_count += 1;
            }
        }
        let temperature_factor = if conditions.temperature > 20.0 {
            ((slow_temperature - conditions.temperature) / (slow_temperature - 20.0))
                .clamp(0.0, 1.0)
        } else {
            1.0
        };
        if target_count > 0 && self.random_f32() < spread_chance * temperature_factor {
            let choice = self.random_u64() as usize % target_count;
            let target = targets[..target_count]
                .iter()
                .flatten()
                .nth(choice)
                .expect("choice is bounded by available fungal targets");
            self.set_element(*target, fungus);
            return true;
        }
        false
    }

    fn is_surface(&self, index: usize) -> bool {
        let element = self.cells[index].element;
        if element == self.rules.id("air").unwrap_or(usize::MAX) {
            return false;
        }
        if self.rules.elements[element].motion == crate::Motion::Fall {
            let below = index + self.width;
            return below >= self.cells.len()
                || self.cells[below].element != self.rules.id("air").unwrap_or(usize::MAX);
        }
        true
    }

    fn neighbors(&self, index: usize) -> [Option<usize>; 4] {
        let x = index % self.width;
        let y = index / self.width;
        [
            (y > 0).then_some(index.wrapping_sub(self.width)),
            (x + 1 < self.width).then_some(index + 1),
            (y + 1 < self.height).then_some(index + self.width),
            (x > 0).then_some(index.wrapping_sub(1)),
        ]
    }
}
