//! Bounded scheduler updates, interactions, and material movement.
use crate::{
    Motion, World,
    world::{ATMOSPHERE_CELL_SIZE, ATMOSPHERE_INTERVAL, MAX_EDGE_WATER_COLUMN},
};

impl World {
    /// Processes at most `budget` work units, servicing recent player actions first.
    /// A rigid-body translation is charged by its member-cell count. Returns work units used.
    pub fn step(&mut self, budget: usize) -> usize {
        let work = budget.min(self.cells.len());
        let mut completed = 0;
        while completed < work {
            let index = self
                .scheduler
                .pop_focus()
                .unwrap_or_else(|| self.scheduler.next_background(self.cells.len()));
            let cost = self.body_work_cost(index);
            let remaining = work - completed;
            if cost <= remaining {
                self.update_cell(index);
                completed += cost;
            } else {
                // Do not let a rigid-group translation overrun the caller's work budget.
                // Retain a runnable body at the front of the queue for the next call.
                if cost <= work && self.cells[index].body_id != 0 {
                    self.scheduler.enqueue_focus(index);
                }
                completed += 1;
            }
            for _ in 0..if cost <= remaining { cost } else { 1 } {
                self.atmosphere_credit += 1;
                if self.atmosphere_credit >= ATMOSPHERE_INTERVAL {
                    self.atmosphere_credit -= ATMOSPHERE_INTERVAL;
                    self.update_atmosphere_cell();
                }
            }
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
        let is_water = element_settings.name == "water";
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
        let water = self.rules.id("water");
        let steam = self.rules.id("steam");
        if Some(self.cells[i].element) == water
            && let (Some(steam), Some(air)) =
                (steam, self.atmosphere_at(i % self.width, i / self.width))
            && air.temperature > self.rules.atmosphere.freezing_point
            && self.ambient_humidity >= self.rules.atmosphere.condensation_humidity
            && self.random_weather_f32() < self.rules.atmosphere.evaporation_rate * 0.015
        {
            let ax = (i % self.width) / ATMOSPHERE_CELL_SIZE;
            let ay = (i / self.width) / ATMOSPHERE_CELL_SIZE;
            self.set_element(i, steam);
            let humidity = &mut self.atmosphere[ay * self.atmosphere_width + ax].humidity;
            *humidity = (*humidity + 8.0).min(100.0);
        }
        let e = self.cells[i].element;
        let Some(element) = self.rules.elements.get(e) else {
            return;
        };
        let lifetime = element.lifetime;
        let decay_to = element.decay_to;
        let emission = element.emission;
        let motion = element.motion;
        let wind_response = element.wind_response;
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
        let is_critter = matches!(self.rules.elements[e].name.as_str(), "beetle" | "termite");
        // Let critters move before a successful meal interaction returns early below.
        if is_critter && self.update_ecology(i) {
            return;
        }
        for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
            let Some(nx) = x.checked_add_signed(dx).filter(|v| *v < self.width) else {
                continue;
            };
            let Some(ny) = y.checked_add_signed(dy).filter(|v| *v < self.height) else {
                continue;
            };
            let j = ny * self.width + nx;
            let other = self.cells[j].element;
            if other == self.rules.id("air").unwrap_or(usize::MAX)
                && self.rules.elements[e].name == "fire"
                && wind.strength > 0
                && wind.direction.horizontal() == dx as i8
                && self.try_firebrand_across_gap(x, y, dx, wind)
            {
                return;
            }
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
        if !is_critter && self.update_ecology(i) {
            return;
        }
        if self.cells[i].body_id != 0 {
            self.update_body_at(i);
            return;
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
            Motion::Fall => {
                if y + 1 < self.height {
                    if self.try_density_move(i, i + self.width, true) {
                        return;
                    }
                    if self.rules.elements[e].name == "ash" && self.try_roll_down(i, x, y) {
                        return;
                    }
                    if self.rules.elements[e].name == "sand" && self.try_sand_roll(i, x, y) {
                        return;
                    }
                }
                if y > 0 {
                    self.try_density_move(i, i - self.width, false);
                }
            }
            Motion::Gas if y > 0 => {
                self.try_density_move(i, i - self.width, false);
            }
            Motion::Liquid => {
                if self.try_liquid_pressure_flow(i, x, y, e) {
                    return;
                }
                if is_water
                    && (x == 0 || x + 1 == self.width)
                    && (y..self.height)
                        .take(MAX_EDGE_WATER_COLUMN)
                        .take_while(|row| self.cells[row * self.width + x].element == e)
                        .count()
                        == MAX_EDGE_WATER_COLUMN
                {
                    let inward_x = if x == 0 { 1 } else { x - 1 };
                    if self.width > 1 {
                        self.swap_if_empty(i, y * self.width + inward_x);
                    }
                    return;
                }
                if y + 1 < self.height && self.try_density_move(i, i + self.width, true) {
                    return;
                }
                if y > 0 && self.try_density_move(i, i - self.width, false) {
                    return;
                }
                let first_direction = if self.random_u64() & 1 == 0 {
                    -1isize
                } else {
                    1
                };
                // Keep rainfall puddles on the original one-sided flow; deep lakes can check both banks.
                let deep_water = self.rules.elements[e].name == "water"
                    && (0..8).all(|offset| {
                        y + offset < self.height && self.cells[i + offset * self.width].element == e
                    });
                let directions = if deep_water {
                    [first_direction, -first_direction]
                } else {
                    [first_direction, 0]
                };
                for direction in directions.into_iter().filter(|direction| *direction != 0) {
                    if let Some(nx) = x.checked_add_signed(direction).filter(|v| *v < self.width)
                        && self.swap_if_empty(i, y * self.width + nx)
                    {
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    /// Wind-borne brands can ignite a flammable cell across one empty cell.
    fn try_firebrand_across_gap(
        &mut self,
        x: usize,
        y: usize,
        dx: isize,
        wind: crate::Wind,
    ) -> bool {
        let Some(target_x) = x.checked_add_signed(dx * 2).filter(|x| *x < self.width) else {
            return false;
        };
        let target = y * self.width + target_x;
        let element = self.cells[target].element;
        let fire = self.rules.id("fire").unwrap_or(usize::MAX);
        let rule = self
            .rules
            .interactions
            .iter()
            .find(|rule| rule.enabled && rule.when == fire && rule.with == element)
            .map(|rule| {
                (
                    rule.chance,
                    rule.downwind_bonus,
                    rule.result,
                    rule.self_result,
                )
            });
        let Some((base_chance, downwind_bonus, result, self_result)) = rule else {
            return false;
        };
        let chance = ((base_chance + downwind_bonus * wind.strength as f32)
            * self.flammability_factor(target, element))
        .min(1.0);
        if self.random_f32() >= chance {
            return false;
        }
        let replaced = element;
        self.set_element(target, result);
        let source = y * self.width + x;
        if let Some(result) = self_result {
            self.set_element(source, result);
        }
        self.wake_decomposer(fire, replaced, target);
        true
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
            "tree" | "plant" | "oil" | "wood" | "deadwood" | "leaf_litter" | "humus"
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
        let mut plume_seed = false;
        for (x, y) in sample_points {
            let name = &self.rules.elements[self.cells[y * self.width + x].element].name;
            match name.as_str() {
                "fire" | "oil_fire" => {
                    heat += 2_000.0;
                    plume_seed = true;
                }
                "lava" => {
                    heat += 2_600.0;
                    plume_seed = true;
                }
                "water" => {
                    moisture += 1.0;
                    // Water and ice cool nearby air gently; large bodies must not outpace
                    // ambient relaxation and freeze otherwise temperate lakes in seconds.
                    heat -= 0.1;
                }
                "steam" => {
                    moisture += 0.5;
                    heat += 6.0;
                    plume_seed = true;
                }
                "smoke" => plume_seed = true,
                "ice" => heat -= 0.1,
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
            let x = x0
                + self.random_weather_u64() as usize % (self.width - x0).min(ATMOSPHERE_CELL_SIZE);
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
        if humidity >= settings.condensation_humidity
            && (temperature < 25.0 || (plume_seed && temperature < 120.0))
        {
            if let Some(cloud) = self.rules.id("cloud") {
                // Condensation samples within the atmospheric cell so visible clouds
                // do not expose the coarse simulation grid as a block pattern.
                let x = x0
                    + self.random_weather_u64() as usize
                        % (self.width - x0).min(ATMOSPHERE_CELL_SIZE);
                let y = y0
                    + self.random_weather_u64() as usize
                        % (self.height - y0).min(ATMOSPHERE_CELL_SIZE);
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
                let warmth = (0.12
                    + (temperature - self.ambient_temperature) / settings.boiling_point.max(1.0))
                .clamp(0.0, 1.0);
                if temperature > settings.freezing_point
                    && self.random_weather_f32() < settings.evaporation_rate * warmth
                {
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

    /// Let a surface liquid flow across a bounded horizontal distance toward a lower head.
    fn try_liquid_pressure_flow(
        &mut self,
        index: usize,
        x: usize,
        y: usize,
        element: usize,
    ) -> bool {
        // Pressure transfers are for lake columns, not transient shallow rain puddles.
        if self.cells[index].body_id != 0
            || self.rules.elements[element].name != "water"
            || (y > 0 && self.cells[index - self.width].element == element)
            || !(0..6).all(|offset| {
                y + offset < self.height
                    && self.cells[index + offset * self.width].element == element
            })
        {
            return false;
        }
        let first_direction = if (x + y) % 2 == 0 { 1isize } else { -1 };
        let air = self.rules.id("air").unwrap_or(usize::MAX);
        for direction in [first_direction, -first_direction] {
            for distance in 1..=256isize {
                let Some(target_x) = x
                    .checked_add_signed(direction * distance)
                    .filter(|x| *x < self.width)
                else {
                    break;
                };
                let mut surface_y = None;
                for row in y + 1..(y + 33).min(self.height) {
                    let target_element = self.cells[row * self.width + target_x].element;
                    if target_element == element {
                        surface_y = Some(row);
                        break;
                    }
                    let target_settings = &self.rules.elements[target_element];
                    if target_element == air
                        || matches!(target_settings.motion, Motion::Liquid | Motion::Fall)
                    {
                        continue;
                    }
                    break;
                }
                let Some(surface_y) = surface_y else {
                    continue;
                };
                let target = surface_y * self.width + target_x;
                if surface_y <= y + 1 {
                    continue;
                }
                // A small bounded batch approximates pressure transfer without scanning the lake.
                let transfer_limit = ((surface_y - y - 1) / 2).min(2);
                let mut transferred = 0;
                for offset in 0..transfer_limit {
                    let source = index + offset * self.width;
                    let destination = target - (offset + 1) * self.width;
                    if self.cells[source].element != element
                        || self.cells[destination].element != air
                    {
                        break;
                    }
                    self.cells.swap(source, destination);
                    self.mark_cell_dirty(source);
                    self.mark_cell_dirty(destination);
                    transferred += 1;
                }
                if transferred > 0 {
                    return true;
                }
            }
        }
        false
    }

    /// Let loose sand roll down a diagonal when the cell directly below is blocked.
    fn try_sand_roll(&mut self, index: usize, x: usize, y: usize) -> bool {
        let first_direction = if (x + y) % 2 == 0 { -1isize } else { 1 };
        for direction in [first_direction, -first_direction] {
            let Some(target_x) = x.checked_add_signed(direction).filter(|x| *x < self.width) else {
                continue;
            };
            let target = (y + 1) * self.width + target_x;
            if self.try_density_move(index, target, true) {
                return true;
            }
        }
        false
    }

    /// Move into empty air or probabilistically displace a less-dense movable neighbor.
    fn try_density_move(&mut self, a: usize, b: usize, downward: bool) -> bool {
        let air = self.rules.id("air").unwrap_or(usize::MAX);
        let moving = self.cells[a].element;
        let other = self.cells[b].element;
        let can_displace = self.cells[b].body_id == 0
            && self.rules.elements[other].motion != Motion::Static
            && if downward {
                self.rules.elements[moving].density > self.rules.elements[other].density
            } else {
                self.rules.elements[moving].density < self.rules.elements[other].density
            };
        let density_delta = self.rules.elements[moving]
            .density
            .abs_diff(self.rules.elements[other].density) as f32
            / 255.0;
        let forceful_fluid_displacement = self.rules.elements[other].motion == Motion::Liquid
            && matches!(
                self.rules.elements[moving].name.as_str(),
                "sand" | "steam" | "ice" | "oil"
            );
        let chance = if forceful_fluid_displacement {
            0.8 + density_delta * 0.2
        } else {
            0.06 + density_delta * 0.12
        };
        let can_enter_air =
            other == air && (downward || self.rules.elements[moving].motion == Motion::Gas);
        if can_enter_air || (can_displace && self.random_density_f32() < chance) {
            self.cells.swap(a, b);
            self.mark_cell_dirty(a);
            self.mark_cell_dirty(b);
            true
        } else {
            false
        }
    }

    /// Ash away from active flames can roll one cell down a side, forming an angle-of-repose pile.
    fn try_roll_down(&mut self, index: usize, x: usize, y: usize) -> bool {
        let fire = self.rules.id("fire").unwrap_or(usize::MAX);
        let oil_fire = self.rules.id("oil_fire").unwrap_or(usize::MAX);
        for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
            let Some(nx) = x.checked_add_signed(dx).filter(|x| *x < self.width) else {
                continue;
            };
            let Some(ny) = y.checked_add_signed(dy).filter(|y| *y < self.height) else {
                continue;
            };
            let neighbor = self.cells[ny * self.width + nx].element;
            if neighbor == fire || neighbor == oil_fire {
                return false;
            }
        }
        let first_direction = if (x + y) % 2 == 0 { -1isize } else { 1 };
        for direction in [first_direction, -first_direction] {
            let Some(target_x) = x.checked_add_signed(direction).filter(|x| *x < self.width) else {
                continue;
            };
            let target = (y + 1) * self.width + target_x;
            if self.swap_if_empty(index, target) {
                return true;
            }
        }
        false
    }

    pub(super) fn swap_if_empty(&mut self, a: usize, b: usize) -> bool {
        self.cells
            .get(b)
            .is_some_and(|cell| Some(cell.element) == self.rules.id("air"))
            && self.try_density_move(a, b, true)
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

    pub(super) fn set_element(&mut self, index: usize, element: usize) {
        let changed = self.cells[index].element != element;
        self.detach_body_member(index);
        self.cells[index] = crate::Cell {
            element,
            age: 0,
            moisture: 0,
            burn_progress: 0,
            body_id: 0,
        };
        if changed {
            self.mark_cell_dirty(index);
        }
    }

    pub(super) fn mark_cell_dirty(&mut self, index: usize) {
        self.dirty_regions
            .mark_cell(index % self.width, index / self.width);
    }

    pub(super) fn random_u64(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    pub(super) fn random_f32(&mut self) -> f32 {
        (self.random_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// Density motion has an independent stream so new settling choices cannot retune reactions.
    fn random_density_f32(&mut self) -> f32 {
        let mut x = self.density_rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.density_rng = x;
        (x >> 40) as f32 / (1u32 << 24) as f32
    }

    fn random_weather_u64(&mut self) -> u64 {
        let mut x = self.weather_rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.weather_rng = x;
        x
    }

    fn random_weather_f32(&mut self) -> f32 {
        (self.random_weather_u64() >> 40) as f32 / (1u32 << 24) as f32
    }
}
