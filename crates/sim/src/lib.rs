//! Deterministic, headless falling-material simulation.
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

pub const DEFAULT_RULES: &str = include_str!("../rules/default.toml");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Motion {
    Static,
    Fall,
    Liquid,
    Gas,
}

#[derive(Clone, Debug)]
pub struct Element {
    pub name: String,
    pub color: [u8; 3],
    pub motion: Motion,
    pub lifetime: u16,
    pub decay_to: Option<usize>,
    pub emission: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Interaction {
    pub when: usize,
    pub with: usize,
    pub result: usize,
    pub self_result: Option<usize>,
    pub chance: f32,
}

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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RulesError(pub String);
impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for RulesError {}

impl Rules {
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
    pub fn id(&self, name: &str) -> Option<usize> {
        self.elements.iter().position(|e| e.name == name)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cell {
    pub element: usize,
    age: u16,
}

pub struct World {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
    pub rules: Rules,
    cursor: usize,
    rng: u64,
    priority: Vec<usize>,
    pub pending: usize,
}
impl World {
    pub fn new(width: usize, height: usize, seed: u64, rules: Rules) -> Result<Self, RulesError> {
        let count = width
            .checked_mul(height)
            .filter(|n| *n > 0 && *n <= 16_777_216)
            .ok_or_else(|| {
                RulesError("world dimensions must be non-zero and at most 16,777,216 cells".into())
            })?;
        let air = rules
            .id("air")
            .ok_or_else(|| RulesError("ruleset must define 'air'".into()))?;
        Ok(Self {
            width,
            height,
            cells: vec![
                Cell {
                    element: air,
                    age: 0
                };
                count
            ],
            rules,
            cursor: 0,
            rng: seed.max(1),
            priority: Vec::with_capacity(count.min(4096)),
            pending: 0,
        })
    }
    pub fn reset(&mut self, seed: u64) {
        let air = self.rules.id("air").unwrap_or(0);
        self.cells.fill(Cell {
            element: air,
            age: 0,
        });
        self.cursor = 0;
        self.rng = seed.max(1);
        self.priority.clear();
        self.pending = 0;
    }
    pub fn paint(&mut self, x: usize, y: usize, element: usize) {
        let Some(i) = y
            .checked_mul(self.width)
            .and_then(|i| i.checked_add(x))
            .filter(|_| x < self.width && y < self.height)
        else {
            return;
        };
        self.cells[i] = Cell { element, age: 0 };
        self.queue_focus(i);
        let x = i % self.width;
        let y = i / self.width;
        for (dx, dy) in [(0isize, -1isize), (1, 0), (0, 1), (-1, 0)] {
            if let Some(nx) = x.checked_add_signed(dx).filter(|v| *v < self.width)
                && let Some(ny) = y.checked_add_signed(dy).filter(|v| *v < self.height)
            {
                self.queue_focus(ny * self.width + nx);
            }
        }
        self.pending = self.priority.len();
    }
    pub fn step(&mut self, budget: usize) -> usize {
        let work = budget.min(self.cells.len());
        let mut completed = 0;
        while completed < work {
            if let Some(i) = self.priority.pop() {
                self.update_cell(i);
            } else {
                let i = self.cursor;
                self.cursor = (self.cursor + 1) % self.cells.len();
                self.update_cell(i);
            }
            completed += 1;
        }
        self.pending = self.priority.len();
        completed
    }
    fn update_cell(&mut self, i: usize) {
        let e = self.cells[i].element;
        let Some(element) = self.rules.elements.get(e) else {
            return;
        };
        let lifetime = element.lifetime;
        let decay_to = element.decay_to;
        let emission = element.emission;
        let motion = element.motion;
        if lifetime > 0 {
            self.cells[i].age = self.cells[i].age.saturating_add(1);
            if self.cells[i].age >= lifetime {
                if let Some(to) = decay_to {
                    self.cells[i] = Cell {
                        element: to,
                        age: 0,
                    };
                }
                if let Some(emission) = emission {
                    let above = i.checked_sub(self.width);
                    if let Some(j) = above.filter(|j| {
                        self.cells[*j].element == self.rules.id("air").unwrap_or(usize::MAX)
                    }) {
                        self.cells[j] = Cell {
                            element: emission,
                            age: 0,
                        };
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
            let rule = self
                .rules
                .interactions
                .iter()
                .find(|r| r.when == e && r.with == other)
                .map(|r| (r.chance, r.result, r.self_result));
            if let Some((chance, result, self_result)) = rule {
                if self.random_f32() < chance {
                    self.cells[j] = Cell {
                        element: result,
                        age: 0,
                    };
                    if let Some(to) = self_result {
                        self.cells[i] = Cell {
                            element: to,
                            age: 0,
                        };
                    }
                    return;
                }
            }
        }
        match motion {
            Motion::Fall if y + 1 < self.height => self.swap_if_empty(i, i + self.width),
            Motion::Gas if y > 0 => self.swap_if_empty(i, i - self.width),
            Motion::Liquid
                if y + 1 < self.height
                    && self.cells[i + self.width].element
                        == self.rules.id("air").unwrap_or(usize::MAX) =>
            {
                self.swap_if_empty(i, i + self.width)
            }
            Motion::Liquid => {
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
    fn queue_focus(&mut self, index: usize) {
        if self.priority.len() < self.priority.capacity() {
            self.priority.push(index);
        }
    }
    fn swap_if_empty(&mut self, a: usize, b: usize) {
        if self
            .cells
            .get(b)
            .is_some_and(|c| Some(c.element) == self.rules.id("air"))
        {
            self.cells.swap(a, b);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    fn world(seed: u64) -> World {
        World::new(32, 24, seed, Rules::load(DEFAULT_RULES).unwrap()).unwrap()
    }
    #[test]
    fn rules_load_and_missing_reference_has_clear_error() {
        let r = Rules::load(DEFAULT_RULES).unwrap();
        for n in [
            "fire", "tree", "ash", "smoke", "water", "steam", "sand", "stone",
        ] {
            assert!(r.id(n).is_some(), "missing {n}");
        }
        assert!(
            Rules::load("[[elements]]\nname='air'\ncolor=[0,0,0]\nmotion='invalid'")
                .unwrap_err()
                .to_string()
                .contains("unknown motion")
        );
        let invalid_reference = "[[elements]]\nname='air'\ncolor=[0,0,0]\nmotion='static'\n[[interactions]]\nwhen='air'\nwith='missing'\nresult='air'\nchance=1.0";
        assert!(
            Rules::load(invalid_reference)
                .unwrap_err()
                .to_string()
                .contains("unknown element 'missing'")
        );
        let invalid_probability = "[[elements]]\nname='air'\ncolor=[0,0,0]\nmotion='static'\n[[interactions]]\nwhen='air'\nwith='air'\nresult='air'\nchance=2.0";
        assert!(
            Rules::load(invalid_probability)
                .unwrap_err()
                .to_string()
                .contains("chance must be between 0 and 1")
        );
    }
    #[test]
    fn seeded_fire_tree_ash_smoke_water_steam_sand_stone_chain() {
        let mut a = world(77);
        let mut b = world(77);
        let fire = a.rules.id("fire").unwrap();
        let tree = a.rules.id("tree").unwrap();
        let water = a.rules.id("water").unwrap();
        let sand = a.rules.id("sand").unwrap();
        let stone = a.rules.id("stone").unwrap();
        a.paint(10, 10, fire);
        a.paint(11, 10, tree);
        a.paint(12, 10, tree);
        a.paint(10, 11, water);
        a.paint(6, 5, fire);
        a.paint(7, 5, water);
        a.paint(5, 5, sand);
        a.paint(20, 4, sand);
        a.paint(20, 20, stone);
        a.paint(31, 23, fire);
        b.cells.clone_from(&a.cells);
        b.priority.clone_from(&a.priority);
        b.pending = a.pending;
        for _ in 0..35 {
            a.step(usize::MAX);
            b.step(usize::MAX);
        }
        assert_eq!(a.cells, b.cells);
        for n in ["ash", "smoke", "steam", "stone"] {
            let e = a.rules.id(n).unwrap();
            assert!(
                a.cells.iter().any(|c| c.element == e),
                "chain did not produce {n}"
            );
        }
        assert!(
            a.cells
                .iter()
                .any(|c| c.element == a.rules.id("sand").unwrap())
        );
    }
    #[test]
    fn default_world_heavy_play_smoke_keeps_work_bounded() {
        let mut w = World::new(192, 120, 99, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
        let fire = w.rules.id("fire").unwrap();
        let tree = w.rules.id("tree").unwrap();
        let water = w.rules.id("water").unwrap();
        for y in 30..90 {
            for x in 40..150 {
                w.paint(x, y, tree);
            }
        }
        for x in 40..150 {
            w.paint(x, 60, fire);
        }
        for frame in 0..240 {
            for x in 48..64 {
                w.paint((x + frame % 96) % w.width, 55, water);
            }
            assert_eq!(w.step(12_000), 12_000);
        }
        assert!(
            w.cells
                .iter()
                .any(|c| c.element != w.rules.id("air").unwrap())
        );
    }

    #[test]
    fn each_update_obeys_work_budget_and_paint_is_bounded() {
        let mut w = world(1);
        assert_eq!(w.step(7), 7);
        let sand = w.rules.id("sand").unwrap();
        w.paint(100, 0, sand);
        assert_eq!(w.step(3), 3);
    }
}
