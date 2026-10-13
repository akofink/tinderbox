use tinderbox_sim::{DEFAULT_RULES, Rules, World};

const PAIR: &str = r#"
[[elements]]
name = "air"
color = [0, 0, 0]
motion = "static"
[[elements]]
name = "spark"
color = [255, 0, 0]
motion = "static"
[[elements]]
name = "fuel"
color = [0, 255, 0]
motion = "static"
[[interactions]]
when = "spark"
with = "fuel"
result = "spark"
chance = 1.0
"#;

fn pair() -> World {
    let mut world = World::new(2, 1, 42, Rules::load(PAIR).unwrap()).unwrap();
    world.paint(0, 0, 1);
    world.paint(1, 0, 2);
    world
}

#[test]
fn disabled_rule_never_fires_and_can_be_reenabled() {
    let mut world = pair();
    world.set_interaction(0, false, 1.0).unwrap();
    for _ in 0..100 {
        world.step(2);
    }
    assert_eq!(world.cells()[1].element, 2);
    world.set_interaction(0, true, 1.0).unwrap();
    world.step(2);
    assert_eq!(world.cells()[1].element, 1);
}

#[test]
fn chance_changes_take_effect_without_resetting() {
    let mut world = pair();
    world.set_interaction(0, true, 0.0).unwrap();
    for _ in 0..100 {
        world.step(2);
    }
    assert_eq!(world.cells()[1].element, 2);
    world.set_interaction(0, true, 1.0).unwrap();
    world.step(2);
    assert_eq!(world.cells()[1].element, 1);
}

#[test]
fn fractional_tuning_is_seed_deterministic() {
    let mut left = pair();
    let mut right = pair();
    for chance in [0.05, 0.25, 0.0, 0.8] {
        left.set_interaction(0, true, chance).unwrap();
        right.set_interaction(0, true, chance).unwrap();
        for _ in 0..4 {
            left.step(1);
            right.step(1);
            assert_eq!(left.cells(), right.cells());
        }
    }
}

#[test]
fn invalid_load_and_edits_keep_rules_cells_and_rng() {
    let mut world = pair();
    let mut untouched = pair();
    let before = world.rules().to_toml().unwrap();
    let cells = world.cells().to_vec();
    for text in [
        "not TOML",
        &PAIR.replace("chance = 1.0", "chance = 2.0"),
        &PAIR.replace("name = \"air\"", "name = \"vacuum\""),
        &PAIR.replace("result = \"spark\"", "result = \"missing\""),
        &PAIR.replace("chance = 1.0", "chance = 1.0\nunsupported = true"),
    ] {
        assert!(world.load_rules(text).is_err());
        assert_eq!(world.rules().to_toml().unwrap(), before);
        assert_eq!(world.cells(), cells);
    }
    for chance in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(world.set_interaction(0, false, chance).is_err());
    }
    assert!(world.set_interaction(999, false, 0.5).is_err());
    world.step(2);
    untouched.step(2);
    assert_eq!(world.cells(), untouched.cells());
    assert_eq!(world.rules().to_toml().unwrap(), before);
}

#[test]
fn export_roundtrip_keeps_disabled_probability() {
    let mut world = pair();
    world.set_interaction(0, false, 0.37).unwrap();
    let exported = world.rules().to_toml().unwrap();
    let mut loaded = pair();
    loaded.load_rules(&exported).unwrap();
    assert!(!loaded.rules().interactions[0].enabled);
    assert_eq!(loaded.rules().interactions[0].chance, 0.37);
    loaded.step(2);
    assert_eq!(loaded.cells()[1].element, 2);
}

#[test]
fn defaults_restore_rules_not_world() {
    let mut world = World::new(2, 1, 42, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    let tree = world.rules().id("tree").unwrap();
    world.paint(1, 0, tree);
    world.set_interaction(0, false, 0.01).unwrap();
    world.reset_rules().unwrap();
    assert_eq!(
        world.rules().to_toml().unwrap(),
        Rules::load(DEFAULT_RULES).unwrap().to_toml().unwrap()
    );
    assert_eq!(world.cells()[1].element, tree);
}

#[test]
fn loads_remap_reordered_and_removed_elements_by_name() {
    let mut world = pair();
    let pending = world.pending_work();
    let reordered = PAIR.replace("name = \"spark\"", "name = \"fuel\"").replace(
        "name = \"fuel\"\ncolor = [0",
        "name = \"spark\"\ncolor = [0",
    );
    world.load_rules(&reordered).unwrap();
    assert_eq!(world.cells()[0].element, world.rules().id("spark").unwrap());
    assert_eq!(world.cells()[1].element, world.rules().id("fuel").unwrap());
    assert_eq!(world.pending_work(), pending);
    let air_only = "[[elements]]\nname = 'air'\ncolor = [0, 0, 0]\nmotion = 'static'";
    world.load_rules(air_only).unwrap();
    assert!(world.cells().iter().all(|c| c.element == 0));
}

#[test]
fn bundled_export_preserves_lifecycle_and_motion() {
    let rules = Rules::load(DEFAULT_RULES).unwrap();
    let loaded = Rules::load(&rules.to_toml().unwrap()).unwrap();
    assert_eq!(rules.to_toml().unwrap(), loaded.to_toml().unwrap());
    for (a, b) in rules.elements.iter().zip(loaded.elements.iter()) {
        assert_eq!(a.motion, b.motion);
        assert_eq!(a.lifetime, b.lifetime);
        assert_eq!(a.decay_to, b.decay_to);
        assert_eq!(a.emission, b.emission);
    }
}
