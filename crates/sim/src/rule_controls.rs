//! Atomic live rule edits. Loading never restarts the world or advances its RNG.
use crate::{AtmosphereRules, Cell, DEFAULT_RULES, Rules, RulesError, World};

impl World {
    /// Atomically replace the atmospheric tuning after validating thresholds and rates.
    pub fn set_atmosphere_rules(&mut self, settings: AtmosphereRules) -> Result<(), RulesError> {
        crate::rules::validate_atmosphere(&settings)?;
        self.rules.atmosphere = settings;
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
        for cell in &mut self.cells {
            if let Some(id) = mapping.get(cell.element).copied().flatten() {
                cell.element = id;
            } else {
                *cell = Cell::default();
                cell.element = air;
            }
        }
        self.dirty_regions.mark_all();
        self.rules = rules;
        Ok(())
    }

    /// Restore the shipped rules without resetting the running world.
    pub fn reset_rules(&mut self) -> Result<(), RulesError> {
        self.load_rules(DEFAULT_RULES)
    }
}
