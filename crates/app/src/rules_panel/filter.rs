//! Pure browsing logic. Element names, not numeric ids, survive ruleset imports.
use tinderbox_sim::{AtmosphereRules, Interaction, Rules};

#[derive(Default)]
pub(super) struct Filter {
    pub query: String,
    pub element: Option<String>,
    pub changed_only: bool,
}

fn signature<'a>(rules: &'a Rules, rule: &Interaction) -> [&'a str; 4] {
    let name = |id: usize| rules.elements[id].name.as_str();
    [
        name(rule.when),
        name(rule.with),
        name(rule.result),
        rule.self_result.map(name).unwrap_or(""),
    ]
}

pub(super) fn changed(rules: &Rules, defaults: &Rules, index: usize) -> bool {
    let rule = &rules.interactions[index];
    let key = signature(rules, rule);
    // Repeated interactions remain distinct, even when their participants match.
    let occurrence = rules.interactions[..index]
        .iter()
        .filter(|other| signature(rules, other) == key)
        .count();
    defaults
        .interactions
        .iter()
        .filter(|other| signature(defaults, other) == key)
        .nth(occurrence)
        .is_none_or(|default| {
            rule.enabled != default.enabled
                || rule.chance != default.chance
                || rule.downwind_bonus != default.downwind_bonus
        })
}

impl Filter {
    pub fn matches(&self, rules: &Rules, defaults: &Rules, index: usize) -> bool {
        let names = signature(rules, &rules.interactions[index]);
        if self
            .element
            .as_ref()
            .is_some_and(|element| !names.contains(&element.as_str()))
            || (self.changed_only && !changed(rules, defaults, index))
        {
            return false;
        }
        let text = names.join(" ").replace('_', " ").to_lowercase();
        self.query
            .split_whitespace()
            .all(|word| text.contains(&word.replace('_', " ").to_lowercase()))
    }

    pub fn atmosphere(&self, current: &AtmosphereRules, defaults: &AtmosphereRules) -> bool {
        let text = "atmosphere thresholds rates freeze freezing melt melting boil boiling ignition heat transfer evaporation condensation humidity pressure wind temperature";
        self.element.is_none()
            && (!self.changed_only || atmosphere_values(current) != atmosphere_values(defaults))
            && self
                .query
                .split_whitespace()
                .all(|word| text.contains(&word.to_lowercase()))
    }

    pub fn active(&self) -> bool {
        !self.query.trim().is_empty() || self.element.is_some() || self.changed_only
    }
}

fn atmosphere_values(a: &AtmosphereRules) -> [f32; 8] {
    [
        a.freezing_point,
        a.melting_point,
        a.boiling_point,
        a.ignition_point,
        a.heat_transfer,
        a.evaporation_rate,
        a.condensation_humidity,
        a.pressure_wind,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinderbox_sim::DEFAULT_RULES;

    fn defaults() -> Rules {
        Rules::load(DEFAULT_RULES).unwrap()
    }

    #[test]
    fn search_and_element_cover_all_participants_and_combine() {
        let rules = defaults();
        let rule = &rules.interactions[0];
        for name in signature(&rules, rule)
            .into_iter()
            .filter(|s| !s.is_empty())
        {
            let filter = Filter {
                element: Some(name.into()),
                ..Default::default()
            };
            assert!(filter.matches(&rules, &rules, 0));
            let filter = Filter {
                query: name.to_uppercase(),
                ..Default::default()
            };
            assert!(filter.matches(&rules, &rules, 0));
        }
        let filter = Filter {
            query: "no such rule".into(),
            element: Some(rules.elements[rule.when].name.clone()),
            ..Default::default()
        };
        assert!(!filter.matches(&rules, &rules, 0));
        assert!(
            !Filter {
                element: Some("removed".into()),
                ..Default::default()
            }
            .matches(&rules, &rules, 0)
        );
        assert!(
            (0..rules.interactions.len()).all(|i| Filter::default().matches(&rules, &rules, i))
        );
    }

    #[test]
    fn changed_view_tracks_edits_reset_new_and_reordered_rules() {
        let defaults = defaults();
        let mut rules = defaults.clone();
        let filter = Filter {
            changed_only: true,
            ..Default::default()
        };
        assert!((0..rules.interactions.len()).all(|i| !filter.matches(&rules, &defaults, i)));
        rules.interactions[0].enabled = false;
        assert!(filter.matches(&rules, &defaults, 0));
        rules.interactions[0] = defaults.interactions[0].clone();
        rules.interactions[0].chance *= 0.5;
        assert!(filter.matches(&rules, &defaults, 0));
        rules = defaults.clone();
        rules.interactions.reverse();
        assert!((0..rules.interactions.len()).all(|i| !changed(&rules, &defaults, i)));
        rules.interactions.push(rules.interactions[0].clone());
        assert!(changed(&rules, &defaults, rules.interactions.len() - 1));
        rules = defaults.clone();
        rules.elements.reverse();
        let n = rules.elements.len();
        for rule in &mut rules.interactions {
            rule.when = n - 1 - rule.when;
            rule.with = n - 1 - rule.with;
            rule.result = n - 1 - rule.result;
            rule.self_result = rule.self_result.map(|id| n - 1 - id);
        }
        assert!((0..rules.interactions.len()).all(|i| !changed(&rules, &defaults, i)));
    }

    #[test]
    fn atmosphere_respects_search_changed_and_element_filters() {
        let rules = defaults();
        let mut current = rules.atmosphere.clone();
        let filter = Filter {
            changed_only: true,
            query: "HEAT".into(),
            ..Default::default()
        };
        assert!(!filter.atmosphere(&current, &rules.atmosphere));
        current.heat_transfer = 0.5;
        assert!(filter.atmosphere(&current, &rules.atmosphere));
        assert!(
            !Filter {
                element: Some("fire".into()),
                ..Default::default()
            }
            .atmosphere(&current, &rules.atmosphere)
        );
    }
}
