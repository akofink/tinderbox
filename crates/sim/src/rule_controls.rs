//! Atomic live rule edits. Loading never restarts the world or advances its RNG.
use crate::{AtmosphereRules, Cell, DEFAULT_RULES, EcologyRules, Rules, RulesError, World};

impl World {
    /// Change one material's lifetime in scheduled updates without advancing simulation time.
    pub fn set_material_lifetime(&mut self, index: usize, lifetime: u16) -> Result<(), RulesError> {
        let element = self
            .rules
            .elements
            .get_mut(index)
            .ok_or_else(|| RulesError(format!("unknown element {index}")))?;
        element.lifetime = lifetime;
        Ok(())
    }

    /// Change one material's relative density without advancing simulation time.
    pub fn set_material_density(&mut self, index: usize, density: u8) -> Result<(), RulesError> {
        let element = self
            .rules
            .elements
            .get_mut(index)
            .ok_or_else(|| RulesError(format!("unknown element {index}")))?;
        element.density = density;
        Ok(())
    }

    /// Atomically replace the atmospheric tuning after validating thresholds and rates.
    pub fn set_atmosphere_rules(&mut self, settings: AtmosphereRules) -> Result<(), RulesError> {
        crate::rules::validate_atmosphere(&settings)?;
        self.rules.atmosphere = settings;
        Ok(())
    }

    /// Atomically replace decomposer and fungus tuning after validation.
    pub fn set_ecology_rules(&mut self, settings: EcologyRules) -> Result<(), RulesError> {
        crate::rules::validate_ecology(&settings)?;
        self.rules.ecology = settings;
        Ok(())
    }

    /// Change moisture response for a material without advancing simulation time.
    pub fn set_material_moisture(
        &mut self,
        index: usize,
        enabled: bool,
        rate: f32,
        wet_flammability: f32,
    ) -> Result<(), RulesError> {
        if !rate.is_finite()
            || !(0.0..=1.0).contains(&rate)
            || !wet_flammability.is_finite()
            || !(0.0..=1.0).contains(&wet_flammability)
        {
            return Err(RulesError(
                "material moisture settings must be between 0 and 1".into(),
            ));
        }
        let element = self
            .rules
            .elements
            .get_mut(index)
            .ok_or_else(|| RulesError(format!("unknown element {index}")))?;
        element.carries_moisture = enabled;
        element.moisture_rate = rate;
        element.wet_flammability = wet_flammability;
        Ok(())
    }

    /// Change one interaction without advancing simulation time.
    pub fn set_interaction(
        &mut self,
        index: usize,
        enabled: bool,
        chance: f32,
    ) -> Result<(), RulesError> {
        if !chance.is_finite() || !(0.0..=1.0).contains(&chance) {
            return Err(RulesError("chance must be between 0 and 1".into()));
        }
        let rule = self
            .rules
            .interactions
            .get_mut(index)
            .ok_or_else(|| RulesError(format!("unknown interaction {index}")))?;
        rule.enabled = enabled;
        rule.chance = chance;
        self.rebuild_interaction_lookup();
        Ok(())
    }

    /// Parse before mutating. Existing elements are remapped by name; removed
    /// elements become air. Cell ages are retained for elements that still exist.
    pub fn load_rules(&mut self, text: &str) -> Result<(), RulesError> {
        let rules = Rules::load(text)?;
        let air = rules
            .id("air")
            .ok_or_else(|| RulesError("ruleset must define 'air'".into()))?;
        let mapping: Vec<_> = self
            .rules
            .elements
            .iter()
            .map(|e| rules.id(&e.name))
            .collect();
        for index in 0..self.cells.len() {
            if let Some(id) = mapping.get(self.cells[index].element).copied().flatten() {
                self.cells[index].element = id;
            } else {
                self.detach_body_member(index);
                self.cells[index] = Cell::default();
                self.cells[index].element = air;
            }
        }
        self.dirty_regions.mark_all();
        self.rules = rules;
        self.rebuild_interaction_lookup();
        Ok(())
    }

    /// Restore the shipped rules without resetting the running world.
    pub fn reset_rules(&mut self) -> Result<(), RulesError> {
        self.load_rules(DEFAULT_RULES)
    }
}
