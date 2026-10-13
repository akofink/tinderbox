//! Atomic live rule edits. Loading never restarts the world or advances its RNG.
use crate::{Cell, DEFAULT_RULES, Rules, RulesError, World};

impl World {
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
        self.rules = rules;
        Ok(())
    }

    /// Restore the shipped rules without resetting the running world.
    pub fn reset_rules(&mut self) -> Result<(), RulesError> {
        self.load_rules(DEFAULT_RULES)
    }
}
