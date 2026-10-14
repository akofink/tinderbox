//! Human-editable TOML ruleset parsing and validation.
use crate::element::{Element, Motion};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Bundled default rules, compiled from `rules/default.toml`.
pub const DEFAULT_RULES: &str = include_str!("../rules/default.toml");

/// One ordered, data-driven neighbor interaction.
#[derive(Clone, Debug)]
pub struct Interaction {
    pub when: usize,
    pub with: usize,
    pub result: usize,
    pub self_result: Option<usize>,
    pub chance: f32,
    pub downwind_bonus: f32,
    /// Disabled interactions retain their tuned chance but never fire.
    pub enabled: bool,
}

/// Validated element and interaction definitions.
#[derive(Clone, Debug)]
pub struct Rules {
    pub elements: Vec<Element>,
    pub interactions: Vec<Interaction>,
    pub atmosphere: AtmosphereRules,
    pub ecology: EcologyRules,
}

/// Tunable atmospheric physics. Temperatures are in Celsius; rates are per atmosphere update.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AtmosphereRules {
    pub freezing_point: f32,
    pub melting_point: f32,
    pub boiling_point: f32,
    pub ignition_point: f32,
    pub heat_transfer: f32,
    pub evaporation_rate: f32,
    pub condensation_humidity: f32,
    pub pressure_wind: f32,
    /// Oxygen consumed by one fire-cell update, in percentage points.
    pub oxygen_consumption: f32,
    /// Fraction of local oxygen difference diffused per atmosphere update.
    pub oxygen_diffusion: f32,
}
impl Default for AtmosphereRules {
    fn default() -> Self {
        Self {
            freezing_point: 0.0,
            melting_point: 2.0,
            boiling_point: 100.0,
            ignition_point: 180.0,
            heat_transfer: 0.08,
            evaporation_rate: 0.08,
            condensation_humidity: 78.0,
            pressure_wind: 0.15,
            oxygen_consumption: 0.35,
            oxygen_diffusion: 0.08,
        }
    }
}

/// Data-driven rates for decomposers and fungus.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct EcologyRules {
    /// Chance per update that a beetle or termite attempts a one-cell move.
    pub critter_move_chance: f32,
    /// Chance of moving away from nearby fire or water.
    pub flee_chance: f32,
    /// Chance of breeding when adjacent to a same-species adult.
    pub breeding_chance: f32,
    /// Chance per fungus update of colonizing a suitable adjacent cell.
    pub fungus_spread_chance: f32,
    /// Chance per update of fading into soil when no adjacent food remains.
    pub fungus_starvation_chance: f32,
    /// Humidity below which fungus cannot colonize.
    pub fungus_min_humidity: f32,
    /// Temperature above which fungus is killed.
    pub fungus_kill_temperature: f32,
    /// Temperature above which fungus colonization slows.
    pub fungus_slow_temperature: f32,
}
impl Default for EcologyRules {
    fn default() -> Self {
        Self {
            critter_move_chance: 0.22,
            flee_chance: 0.8,
            breeding_chance: 0.01,
            fungus_spread_chance: 0.12,
            fungus_starvation_chance: 0.03,
            fungus_min_humidity: 35.0,
            fungus_kill_temperature: 55.0,
            fungus_slow_temperature: 30.0,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawRules {
    elements: Vec<RawElement>,
    #[serde(default)]
    interactions: Vec<RawInteraction>,
    #[serde(default)]
    atmosphere: AtmosphereRules,
    #[serde(default)]
    ecology: EcologyRules,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawElement {
    name: String,
    color: [u8; 3],
    motion: String,
    #[serde(default)]
    wind_response: f32,
    #[serde(default)]
    density: u8,
    #[serde(default)]
    lifetime: u16,
    decay_to: Option<String>,
    emission: Option<String>,
    #[serde(default)]
    carries_moisture: bool,
    #[serde(default)]
    moisture_rate: f32,
    #[serde(default)]
    wet_flammability: f32,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawInteraction {
    when: String,
    with: String,
    result: String,
    self_result: Option<String>,
    chance: f64,
    #[serde(default)]
    downwind_bonus: f32,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

pub(crate) fn validate_ecology(settings: &EcologyRules) -> Result<(), RulesError> {
    let rates = [
        settings.critter_move_chance,
        settings.flee_chance,
        settings.breeding_chance,
        settings.fungus_spread_chance,
        settings.fungus_starvation_chance,
    ];
    if rates
        .iter()
        .any(|rate| !rate.is_finite() || !(0.0..=1.0).contains(rate))
        || !settings.fungus_min_humidity.is_finite()
        || !(0.0..=100.0).contains(&settings.fungus_min_humidity)
        || !settings.fungus_kill_temperature.is_finite()
        || !settings.fungus_slow_temperature.is_finite()
        || settings.fungus_slow_temperature >= settings.fungus_kill_temperature
    {
        return Err(RulesError("invalid ecology settings".into()));
    }
    Ok(())
}

pub(crate) fn validate_atmosphere(settings: &AtmosphereRules) -> Result<(), RulesError> {
    let ordered = settings.freezing_point.is_finite()
        && settings.melting_point.is_finite()
        && settings.boiling_point.is_finite()
        && settings.ignition_point.is_finite()
        && settings.freezing_point < settings.melting_point
        && settings.melting_point < settings.boiling_point
        && settings.boiling_point < settings.ignition_point;
    let rates_valid = settings.heat_transfer.is_finite()
        && (0.0..=1.0).contains(&settings.heat_transfer)
        && settings.evaporation_rate.is_finite()
        && (0.0..=1.0).contains(&settings.evaporation_rate)
        && settings.condensation_humidity.is_finite()
        && (0.0..=100.0).contains(&settings.condensation_humidity)
        && settings.pressure_wind.is_finite()
        && (0.0..=1.0).contains(&settings.pressure_wind)
        && settings.oxygen_consumption.is_finite()
        && (0.0..=10.0).contains(&settings.oxygen_consumption)
        && settings.oxygen_diffusion.is_finite()
        && (0.0..=1.0).contains(&settings.oxygen_diffusion);
    if !ordered || !rates_valid {
        return Err(RulesError(
            "atmosphere thresholds must be ordered and rates must be within their valid ranges"
                .into(),
        ));
    }
    Ok(())
}

/// Validation or parse failure with a human-readable explanation.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RulesError(pub String);
impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for RulesError {}

impl Rules {
    /// Parses TOML and validates element names, motions, references, and probabilities.
    pub fn load(text: &str) -> Result<Self, RulesError> {
        let raw: RawRules =
            toml::from_str(text).map_err(|e| RulesError(format!("invalid rules TOML: {e}")))?;
        if raw.elements.is_empty() {
            return Err(RulesError(
                "ruleset must define at least one element".into(),
            ));
        }
        let mut ids = HashMap::new();
        for (i, element) in raw.elements.iter().enumerate() {
            if element.name.trim().is_empty() {
                return Err(RulesError(format!("element {i} has an empty name")));
            }
            if ids.insert(element.name.clone(), i).is_some() {
                return Err(RulesError(format!("duplicate element '{}'", element.name)));
            }
        }
        let id = |name: &str, path: &str| {
            ids.get(name)
                .copied()
                .ok_or_else(|| RulesError(format!("{path} refers to unknown element '{name}'")))
        };
        let mut elements = Vec::with_capacity(raw.elements.len());
        for e in raw.elements {
            if !e.wind_response.is_finite() || !(0.0..=1.0).contains(&e.wind_response) {
                return Err(RulesError(format!(
                    "element '{}' wind_response must be between 0 and 1",
                    e.name
                )));
            }
            let motion = match e.motion.as_str() {
                "static" => Motion::Static,
                "fall" => Motion::Fall,
                "liquid" => Motion::Liquid,
                "gas" => Motion::Gas,
                other => {
                    return Err(RulesError(format!(
                        "element '{}' has unknown motion '{other}'",
                        e.name
                    )));
                }
            };
            if !e.moisture_rate.is_finite()
                || !(0.0..=1.0).contains(&e.moisture_rate)
                || !e.wet_flammability.is_finite()
                || !(0.0..=1.0).contains(&e.wet_flammability)
            {
                return Err(RulesError(format!(
                    "element '{}' moisture settings must be between 0 and 1",
                    e.name
                )));
            }
            elements.push(Element {
                name: e.name,
                color: e.color,
                motion,
                wind_response: e.wind_response,
                density: e.density,
                lifetime: e.lifetime,
                decay_to: e
                    .decay_to
                    .as_deref()
                    .map(|v| id(v, "decay_to"))
                    .transpose()?,
                emission: e
                    .emission
                    .as_deref()
                    .map(|v| id(v, "emission"))
                    .transpose()?,
                carries_moisture: e.carries_moisture,
                moisture_rate: e.moisture_rate,
                wet_flammability: e.wet_flammability,
            });
        }
        let mut interactions = Vec::with_capacity(raw.interactions.len());
        for (i, r) in raw.interactions.into_iter().enumerate() {
            if !r.chance.is_finite() || !(0.0..=1.0).contains(&r.chance) {
                return Err(RulesError(format!(
                    "interaction {i} chance must be between 0 and 1"
                )));
            }
            if !r.downwind_bonus.is_finite() || !(0.0..=1.0).contains(&r.downwind_bonus) {
                return Err(RulesError(format!(
                    "interaction {i} downwind_bonus must be between 0 and 1"
                )));
            }
            interactions.push(Interaction {
                when: id(&r.when, &format!("interaction {i} when"))?,
                with: id(&r.with, &format!("interaction {i} with"))?,
                result: id(&r.result, &format!("interaction {i} result"))?,
                self_result: r
                    .self_result
                    .as_deref()
                    .map(|v| id(v, &format!("interaction {i} self_result")))
                    .transpose()?,
                chance: r.chance as f32,
                downwind_bonus: r.downwind_bonus,
                enabled: r.enabled,
            });
        }
        validate_atmosphere(&raw.atmosphere)?;
        validate_ecology(&raw.ecology)?;
        Ok(Self {
            elements,
            interactions,
            atmosphere: raw.atmosphere,
            ecology: raw.ecology,
        })
    }

    /// Exports a human-editable ruleset including tuned and disabled interactions.
    pub fn to_toml(&self) -> Result<String, RulesError> {
        let name = |id: usize| {
            self.elements
                .get(id)
                .map(|e| e.name.clone())
                .ok_or_else(|| RulesError(format!("unknown element index {id}")))
        };
        let elements = self
            .elements
            .iter()
            .map(|e| {
                Ok(RawElement {
                    name: e.name.clone(),
                    color: e.color,
                    motion: match e.motion {
                        Motion::Static => "static",
                        Motion::Fall => "fall",
                        Motion::Liquid => "liquid",
                        Motion::Gas => "gas",
                    }
                    .into(),
                    wind_response: e.wind_response,
                    density: e.density,
                    lifetime: e.lifetime,
                    decay_to: e.decay_to.map(name).transpose()?,
                    emission: e.emission.map(name).transpose()?,
                    carries_moisture: e.carries_moisture,
                    moisture_rate: e.moisture_rate,
                    wet_flammability: e.wet_flammability,
                })
            })
            .collect::<Result<Vec<_>, RulesError>>()?;
        let interactions = self
            .interactions
            .iter()
            .map(|r| {
                Ok(RawInteraction {
                    when: name(r.when)?,
                    with: name(r.with)?,
                    result: name(r.result)?,
                    self_result: r.self_result.map(name).transpose()?,
                    // Use the shortest decimal that roundtrips to this f32,
                    // rather than exposing its binary expansion in editable TOML.
                    chance: r
                        .chance
                        .to_string()
                        .parse::<f64>()
                        .map_err(|e| RulesError(format!("cannot serialize chance: {e}")))?,
                    downwind_bonus: r.downwind_bonus,
                    enabled: r.enabled,
                })
            })
            .collect::<Result<Vec<_>, RulesError>>()?;
        let text = toml::to_string_pretty(&RawRules {
            elements,
            interactions,
            atmosphere: self.atmosphere.clone(),
            ecology: self.ecology.clone(),
        })
        .map_err(|e| RulesError(format!("cannot serialize rules: {e}")))?;
        // Public Rules values may have been manually constructed or modified.
        Self::load(&text)?;
        Ok(text)
    }

    /// Looks up an element's stable index in this ruleset.
    pub fn id(&self, name: &str) -> Option<usize> {
        self.elements.iter().position(|e| e.name == name)
    }
}
