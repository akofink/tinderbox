//! Bounded scheduler updates, interactions, and material movement.
use crate::{
    Motion, World,
    world::{ATMOSPHERE_CELL_SIZE, ATMOSPHERE_INTERVAL},
};

impl World {
    /// Processes at most `budget` cells, servicing recent player actions first.
    /// Returns the number of cell updates completed.
    pub fn step(&mut self, budget: usize) -> usize {
        let work = budget.min(self.cells.len());
        let mut completed = 0;
        while completed < work {
            let index = self
                .scheduler
                .pop_focus()
                .unwrap_or_else(|| self.scheduler.next_background(self.cells.len()));
            self.update_cell(index);
            self.atmosphere_credit += 1;
            if self.atmosphere_credit >= ATMOSPHERE_INTERVAL {
                self.atmosphere_credit -= ATMOSPHERE_INTERVAL;
                self.update_atmosphere_cell();
            }
            completed += 1;
        }
        completed
    }

    fn update_cell(&mut self, i: usize) {
        let wind = self.wind_at(i);
        if self.gusts[i].remaining > 0 {
            self.gusts[i].remaining -= 1;
        }
        let e = self.cells[i].element;
        if self.rules.elements.get(e).is_none() {
            return;
        }
        let element_settings = &self.rules.elements[e];
        if element_settings.carries_moisture {
            let humidity = self
                .atmosphere_at(i % self.width, i / self.width)
                .map_or(0.0, |a| a.humidity);
            let moisture = f32::from(self.cells[i].moisture);
            let equilibrated =
                moisture + (humidity * 2.55 - moisture) * element_settings.moisture_rate;
            let updated = equilibrated.clamp(0.0, 255.0) as u8;
            if self.cells[i].moisture != updated {
                self.cells[i].moisture = updated;
                self.mark_cell_dirty(i);
            }
        }
        if self.rules.elements[e].name == "fire" || self.rules.elements[e].name == "oil_fire" {
            let ax = (i % self.width) / ATMOSPHERE_CELL_SIZE;
            let ay = (i / self.width) / ATMOSPHERE_CELL_SIZE;
            let oxygen = &mut self.atmosphere[ay * self.atmosphere_width + ax].oxygen;
            if *oxygen < 2.0 {
                let extinguished = self.rules.elements[e]
                    .decay_to
                    .or_else(|| self.rules.id("air"));
                if let Some(target) = extinguished {
                    self.set_element(i, target);
                }
                return;
            }
            *oxygen = (*oxygen - self.rules.atmosphere.oxygen_consumption).max(0.0);
        }
        self.apply_temperature_phase(i, e);
        let e = self.cells[i].element;
        let Some(element) = self.rules.elements.get(e) else {
            return;
        };
        let lifetime = element.lifetime;
        let decay_to = element.decay_to;
        let emission = element.emission;
        let motion = element.motion;
        let wind_response = element.wind_response;
        let density = element.density;
        if lifetime > 0 {
            self.cells[i].age = self.cells[i].age.saturating_add(1);
            if self.cells[i].age >= lifetime {
                if self.rules.elements[e].name == "plant"
                    && decay_to.is_some_and(|to| self.rules.elements[to].name == "tree")
                {
                    self.grow_tree_canopy(i);
                } else if let Some(to) = decay_to {
                    self.set_element(i, to);
                }
                if let Some(emission) = emission {
                    let above = i.checked_sub(self.width);
                    if let Some(j) = above.filter(|j| {
                        self.cells[*j].element == self.rules.id("air").unwrap_or(usize::MAX)
                    }) {
                        self.set_element(j, emission);
                    }
                }
                return;
            }
        }
        let x = i % self.width;
        let y = i / self.width;
        for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
            let Some(nx) = x.checked_add_signed(dx).filter(|v| *v < self.width) else {
                continue;
            };
            let Some(ny) = y.checked_add_signed(dy).filter(|v| *v < self.height) else {
                continue;
            };
            let j = ny * self.width + nx;
            let other = self.cells[j].element;
            if self.rules.elements[e].name == "fire" && self.rules.elements[other].carries_moisture
            {
                self.cells[j].burn_progress = self.cells[j].burn_progress.saturating_add(4);
                self.mark_cell_dirty(j);
            }
            let rule = self
                .rules
                .interactions
                .iter()
                .find(|r| r.enabled && r.when == e && r.with == other)
                .map(|r| (r.chance, r.downwind_bonus, r.result, r.self_result));
            let seed_on_hot_ash = (self.rules.elements[e].name == "seed"
                && self.rules.elements[other].name == "ash"
                || self.rules.elements[e].name == "ash"
                    && self.rules.elements[other].name == "seed")
                && self
                    .atmosphere_at(x, y)
                    .is_some_and(|field| field.temperature >= self.rules.atmosphere.ignition_point);
            if !seed_on_hot_ash
                && let Some((chance, downwind_bonus, result, self_result)) = rule
                && self.random_f32()
                    < (chance * self.flammability_factor(j, other)
                        + if (nx as isize - x as isize).signum()
                            == wind.direction.horizontal() as isize
                            && wind.direction != crate::WindDirection::Calm
                        {
                            downwind_bonus * wind.strength as f32 / 4.0
                        } else {
                            0.0
                        })
                    .min(1.0)
            {
                let replaced = other;
                self.set_element(j, result);
                if let Some(to) = self_result {
                    self.set_element(i, to);
                }
                // Burning a living cell drops its seed so the ash bed can regrow.
                if let Some(emission) = self.rules.elements.get(replaced).and_then(|el| el.emission)
                {
                    self.emit_into_air_near(j, emission);
                }
                self.wake_decomposer(e, replaced, j);
                return;
            }
        }
        if wind.strength > 0
            && wind.direction != crate::WindDirection::Calm
            && wind_response > 0.0
            && self.random_f32() < wind_response * wind.strength as f32 / 4.0
        {
            let target_x = x.checked_add_signed(wind.direction.horizontal() as isize);
            if let Some(target_x) = target_x.filter(|target_x| *target_x < self.width)
                && self.swap_if_empty(i, y * self.width + target_x)
            {
                return;
            }
        }
        match motion {
            Motion::Fall if y + 1 < self.height => {
                self.swap_if_empty(i, i + self.width);
            }
            Motion::Gas if y > 0 => {
                self.swap_if_empty(i, i - self.width);
            }
            Motion::Liquid => {
                if y > 0 {
                    let above = i - self.width;
                    let above_element = self.cells[above].element;
                    if self.rules.elements[above_element].motion == Motion::Liquid
                        && density < self.rules.elements[above_element].density
                    {
                        self.cells.swap(i, above);
                        self.mark_cell_dirty(i);
                        self.mark_cell_dirty(above);
                        return;
                    }
                }
                if y + 1 < self.height {
                    let below = i + self.width;
                    let below_element = self.cells[below].element;
                    if Some(below_element) == self.rules.id("air")
                        || (self.rules.elements[below_element].motion == Motion::Liquid
                            && density > self.rules.elements[below_element].density)
                    {
                        self.cells.swap(i, below);
                        self.mark_cell_dirty(i);
                        self.mark_cell_dirty(below);
                        return;
                    }
                }
                let dir = if self.random_u64() & 1 == 0 {
                    -1isize
                } else {
                    1
                };
                if let Some(nx) = x.checked_add_signed(dir).filter(|v| *v < self.width) {
                    self.swap_if_empty(i, y * self.width + nx);
                }
            }
            _ => {}
        }
    }

    fn apply_temperature_phase(&mut self, index: usize, element: usize) {
        let air = self.rules.id("air").unwrap_or(usize::MAX);
        if self.cells[index].element == air {
            return;
        }
        let temp = self
            .atmosphere_at(index % self.width, index / self.width)
            .map(|field| field.temperature)
            .unwrap_or(self.ambient_temperature);
        let name = self.rules.elements[element].name.as_str();
        let atmosphere_rules = &self.rules.atmosphere;
        let wetness = f32::from(self.cells[index].moisture) / 255.0;
        let priming = f32::from(self.cells[index].burn_progress) / 255.0;
        let ignition_point = atmosphere_rules.ignition_point + wetness * 180.0 - priming * 30.0;
        let target = match name {
            "water" if temp <= atmosphere_rules.freezing_point => self.rules.id("ice"),
            "water" if temp >= atmosphere_rules.boiling_point => self.rules.id("steam"),
            "ice" if temp >= atmosphere_rules.melting_point => self.rules.id("water"),
            "tree" | "plant" | "oil" | "deadwood" | "leaf_litter" | "humus"
                if temp >= ignition_point =>
            {
                self.rules.id("fire")
            }
            _ => None,
        };
        if let Some(target) = target {
            let emission = self.rules.elements[element].emission;
            self.set_element(index, target);
            if let Some(emission) = emission {
                self.emit_into_air_near(index, emission);
            }
        }
    }

    fn update_atmosphere_cell(&mut self) {
        if self.atmosphere.is_empty() {
            return;
        }
        let index = self.atmosphere_cursor;
        self.atmosphere_cursor = (self.atmosphere_cursor + 1) % self.atmosphere.len();
        let ax = index % self.atmosphere_width;
        let ay = index / self.atmosphere_width;
        let x0 = ax * ATMOSPHERE_CELL_SIZE;
        let y0 = ay * ATMOSPHERE_CELL_SIZE;
        let cx = (x0 + ATMOSPHERE_CELL_SIZE / 2).min(self.width - 1);
        let cy = (y0 + ATMOSPHERE_CELL_SIZE / 2).min(self.height - 1);
        let sample_points = [
            (cx, cy),
            (x0.min(self.width - 1), y0.min(self.height - 1)),
            (
                (x0 + ATMOSPHERE_CELL_SIZE - 1).min(self.width - 1),
                y0.min(self.height - 1),
            ),
            (
                x0.min(self.width - 1),
                (y0 + ATMOSPHERE_CELL_SIZE - 1).min(self.height - 1),
            ),
            (
                (x0 + ATMOSPHERE_CELL_SIZE - 1).min(self.width - 1),
                (y0 + ATMOSPHERE_CELL_SIZE - 1).min(self.height - 1),
            ),
        ];
        let mut heat = 0.0;
        let mut moisture = 0.0;
        for (x, y) in sample_points {
            let name = &self.rules.elements[self.cells[y * self.width + x].element].name;
            match name.as_str() {
                "fire" | "oil_fire" => heat += 2_000.0,
                "lava" => heat += 2_600.0,
                "water" => {
                    moisture += 1.0;
                    heat -= 4.0;
                }
                "steam" => {
                    moisture += 0.5;
                    heat += 6.0;
                }
                "ice" => heat -= 3.0,
                _ => {}
            }
        }
        let west = if ax > 0 {
            self.atmosphere[index - 1].temperature
        } else {
            self.ambient_temperature
        };
        let east = if ax + 1 < self.atmosphere_width {
            self.atmosphere[index + 1].temperature
        } else {
            self.ambient_temperature
        };
        let north = if ay > 0 {
            self.atmosphere[index - self.atmosphere_width].temperature
        } else {
            self.ambient_temperature
        };
        let south = if ay + 1 < self.atmosphere_height {
            self.atmosphere[index + self.atmosphere_width].temperature
        } else {
            self.ambient_temperature
        };
        let prior = self.atmosphere[index];
        let settings = self.rules.atmosphere.clone();
        let temperature = (prior.temperature
            + (self.ambient_temperature - prior.temperature) * 0.015
            + ((west + east + north + south) * 0.25 - prior.temperature) * settings.heat_transfer
            + heat * 0.025)
            .clamp(-100.0, 1200.0);
        // Ambient humidity is the finite initial vapor reservoir, not a source that
        // refills every update. Vapor is added only when a liquid cell actually
        // transitions to steam below.
        let mut humidity = prior.humidity.clamp(0.0, 100.0);
        let oxygen = self.diffused_oxygen(index, prior.oxygen, &settings);
        let pressure = (temperature - self.ambient_temperature) * 0.2
            + (if ay > 0 {
                self.atmosphere[index - self.atmosphere_width].temperature - temperature
            } else {
                0.0
            }) * 0.1;
        self.atmosphere[index] = crate::AtmosphereCell {
            temperature,
            humidity,
            pressure,
            oxygen,
        };

        if humidity >= settings.condensation_humidity
            && temperature > settings.freezing_point
            && temperature <= settings.melting_point + 3.0
        {
            let ground_y = (y0 + ATMOSPHERE_CELL_SIZE).min(self.height - 1);
            let x = x0 + self.random_u64() as usize % (self.width - x0).min(ATMOSPHERE_CELL_SIZE);
            let ground = ground_y * self.width + x;
            let above = ground.saturating_sub(self.width);
            if self.rules.elements[self.cells[ground].element]
                .name
                .ends_with("soil")
                && self.cells[above].element == self.rules.id("air").unwrap_or(usize::MAX)
                && let Some(water) = self.rules.id("water")
            {
                self.set_element(above, water);
                humidity = (humidity - 12.0).max(0.0);
            }
        }
        if humidity >= settings.condensation_humidity && temperature < 25.0 {
            if let Some(cloud) = self.rules.id("cloud") {
                // Condensation samples within the atmospheric cell so visible clouds
                // do not expose the coarse simulation grid as a block pattern.
                let x =
                    x0 + self.random_u64() as usize % (self.width - x0).min(ATMOSPHERE_CELL_SIZE);
                let y =
                    y0 + self.random_u64() as usize % (self.height - y0).min(ATMOSPHERE_CELL_SIZE);
                let cell = y * self.width + x;
                if self.cells[cell].element == self.rules.id("air").unwrap_or(usize::MAX) {
                    self.set_element(cell, cloud);
                    humidity = (humidity - 50.0).max(0.0);
                }
            }
        }
        self.atmosphere[index].humidity = humidity;
        if moisture > 0.0 && temperature > self.ambient_temperature + 4.0 {
            let water = self.rules.id("water");
            let steam = self.rules.id("steam");
            if let (Some(water), Some(steam)) = (water, steam)
                && let Some((x, y)) = sample_points
                    .into_iter()
                    .find(|(x, y)| self.cells[y * self.width + x].element == water)
            {
                let warmth = ((temperature - self.ambient_temperature)
                    / settings.boiling_point.max(1.0))
                .clamp(0.0, 1.0);
                if self.random_f32() < settings.evaporation_rate * warmth {
                    self.set_element(y * self.width + x, steam);
                    self.atmosphere[index].humidity =
                        (self.atmosphere[index].humidity + 8.0).min(100.0);
                }
            }
        }
    }

    pub(crate) fn flammability_factor(&self, index: usize, element: usize) -> f32 {
        let settings = &self.rules.elements[element];
        if !settings.carries_moisture {
            return 1.0;
        }
        let wetness = f32::from(self.cells[index].moisture) / 255.0;
        let priming = f32::from(self.cells[index].burn_progress) / 255.0;
        (1.0 - wetness * (1.0 - settings.wet_flammability)) * (1.0 + priming * 0.5)
    }

    fn diffused_oxygen(
        &self,
        index: usize,
        current: f32,
        settings: &crate::AtmosphereRules,
    ) -> f32 {
        let x = index % self.atmosphere_width;
        let y = index / self.atmosphere_width;
        let west = if x > 0 {
            self.atmosphere[index - 1].oxygen
        } else {
            current
        };
        let east = if x + 1 < self.atmosphere_width {
            self.atmosphere[index + 1].oxygen
        } else {
            current
        };
        let north = if y > 0 {
            self.atmosphere[index - self.atmosphere_width].oxygen
        } else {
            current
        };
        let south = if y + 1 < self.atmosphere_height {
            self.atmosphere[index + self.atmosphere_width].oxygen
        } else {
            current
        };
        (current
            + (self.ambient_oxygen - current) * 0.01
            + ((west + east + north + south) * 0.25 - current) * settings.oxygen_diffusion)
            .clamp(0.0, 100.0)
    }

    fn grow_tree_canopy(&mut self, base: usize) {
        let Some(tree) = self.rules.id("tree") else {
            return;
        };
        self.set_element(base, tree);
        let x = base % self.width;
        let y = base / self.width;
        // Mature saplings become a small rooted silhouette: a five-cell trunk
        // with a three-cell-wide crown. Existing materials are never displaced.
        for dy in 1..=4 {
            if let Some(row) = y.checked_sub(dy) {
                let index = row * self.width + x;
                if self.rules.elements[self.cells[index].element].name == "air" {
                    self.set_element(index, tree);
                }
            }
        }
        for dy in 2..=4 {
            if let Some(row) = y.checked_sub(dy) {
                for dx in [-1isize, 1] {
                    let Some(column) = x.checked_add_signed(dx).filter(|x| *x < self.width) else {
                        continue;
                    };
                    let index = row * self.width + column;
                    if self.rules.elements[self.cells[index].element].name == "air" {
                        self.set_element(index, tree);
                    }
                }
            }
        }
    }

    fn swap_if_empty(&mut self, a: usize, b: usize) -> bool {
        if self
            .cells
            .get(b)
            .is_some_and(|c| Some(c.element) == self.rules.id("air"))
        {
            self.cells.swap(a, b);
            self.mark_cell_dirty(a);
            self.mark_cell_dirty(b);
            true
        } else {
            false
        }
    }

    fn emit_into_air_near(&mut self, index: usize, element: usize) {
        let air = self.rules.id("air").unwrap_or(usize::MAX);
        let x = index % self.width;
        let y = index / self.width;
        for (dx, dy) in [
            (0isize, -1isize),
            (1, 0),
            (-1, 0),
            (0, 1),
            (1, -1),
            (-1, -1),
        ] {
            let Some(nx) = x.checked_add_signed(dx).filter(|v| *v < self.width) else {
                continue;
            };
            let Some(ny) = y.checked_add_signed(dy).filter(|v| *v < self.height) else {
                continue;
            };
            let j = ny * self.width + nx;
            if self.cells[j].element == air {
                self.set_element(j, element);
                return;
            }
        }
        if self.rules.elements[element].name == "seed" {
            // Dense crowns and an active fire front can fill every adjacent cell.
            // Release a seed above that canopy instead of silently losing it.
            let canopy_clearance = self.height.min((self.height / 4).max(8));
            for dy in 2..=canopy_clearance {
                let Some(row) = y.checked_sub(dy) else {
                    break;
                };
                for dx in [0isize, -1, 1, -2, 2] {
                    let Some(column) = x.checked_add_signed(dx).filter(|x| *x < self.width) else {
                        continue;
                    };
                    let j = row * self.width + column;
                    if self.cells[j].element == air {
                        self.set_element(j, element);
                        return;
                    }
                }
            }
        }
    }

    fn wake_decomposer(&mut self, actor: usize, replaced: usize, index: usize) {
        let Some(ash) = self.rules.id("ash") else {
            return;
        };
        let Some(water) = self.rules.id("water") else {
            return;
        };
        let Some(fungus) = self.rules.id("fungus") else {
            return;
        };
        if (actor == ash && replaced == water) || (actor == water && replaced == ash) {
            self.emit_into_air_near(index, fungus);
        }
    }

    fn set_element(&mut self, index: usize, element: usize) {
        let changed = self.cells[index].element != element;
        self.cells[index] = crate::Cell {
            element,
            age: 0,
            moisture: 0,
            burn_progress: 0,
        };
        if changed {
            self.mark_cell_dirty(index);
        }
    }

    fn mark_cell_dirty(&mut self, index: usize) {
        self.dirty_regions
            .mark_cell(index % self.width, index / self.width);
    }

    fn random_u64(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    fn random_f32(&mut self) -> f32 {
        (self.random_u64() >> 40) as f32 / (1u32 << 24) as f32
    }
}
