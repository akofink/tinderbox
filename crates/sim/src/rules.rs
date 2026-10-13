//! Human-editable TOML ruleset parsing and validation.
use crate::element::{Element, Motion};
use serde::Deserialize;
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
}

/// Validated element and interaction definitions.
#[derive(Clone, Debug)]
pub struct Rules {
    pub elements: Vec<Element>,
    pub interactions: Vec<Interaction>,
}

#[derive(Deserialize)]
struct RawRules {
    elements: Vec<RawElement>,
    #[serde(default)]
    interactions: Vec<RawInteraction>,
}
#[derive(Deserialize)]
struct RawElement {
    name: String,
    color: [u8; 3],
    motion: String,
    #[serde(default)]
    lifetime: u16,
    decay_to: Option<String>,
    emission: Option<String>,
}
#[derive(Deserialize)]
struct RawInteraction {
    when: String,
    with: String,
    result: String,
    self_result: Option<String>,
    chance: f32,
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
            elements.push(Element {
                name: e.name,
                color: e.color,
                motion,
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
            });
        }
        let mut interactions = Vec::with_capacity(raw.interactions.len());
        for (i, r) in raw.interactions.into_iter().enumerate() {
            if !r.chance.is_finite() || !(0.0..=1.0).contains(&r.chance) {
                return Err(RulesError(format!(
                    "interaction {i} chance must be between 0 and 1"
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
                chance: r.chance,
            });
        }
        Ok(Self {
            elements,
            interactions,
        })
    }

    /// Looks up an element's stable index in this ruleset.
    pub fn id(&self, name: &str) -> Option<usize> {
        self.elements.iter().position(|e| e.name == name)
    }
}
