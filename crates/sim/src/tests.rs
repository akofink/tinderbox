use crate::{DEFAULT_RULES, Rules, World};

fn world(seed: u64) -> World {
    World::new(32, 24, seed, Rules::load(DEFAULT_RULES).unwrap()).unwrap()
}

#[test]
fn rules_load_and_missing_reference_has_clear_error() {
    let rules = Rules::load(DEFAULT_RULES).unwrap();
    for name in [
        "fire", "tree", "ash", "smoke", "water", "steam", "sand", "stone",
    ] {
        assert!(rules.id(name).is_some(), "missing {name}");
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
    let invalid_wind =
        "[[elements]]\nname='air'\ncolor=[0,0,0]\nmotion='static'\nwind_response=1.5";
    assert!(
        Rules::load(invalid_wind)
            .unwrap_err()
            .to_string()
            .contains("wind_response must be between 0 and 1")
    );
}

#[test]
fn seeded_fire_tree_ash_smoke_water_steam_sand_stone_chain() {
    let mut a = world(77);
    let mut b = world(77);
    let fire = a.rules().id("fire").unwrap();
    let tree = a.rules().id("tree").unwrap();
    let water = a.rules().id("water").unwrap();
    let sand = a.rules().id("sand").unwrap();
    let stone = a.rules().id("stone").unwrap();
    for world in [&mut a, &mut b] {
        world.paint(10, 10, fire);
        world.paint(11, 10, tree);
        world.paint(12, 10, tree);
        world.paint(10, 11, water);
        world.paint(6, 5, fire);
        world.paint(7, 5, water);
        world.paint(5, 5, sand);
        world.paint(20, 4, sand);
        world.paint(20, 20, stone);
        world.paint(31, 23, fire);
    }
    for _ in 0..35 {
        a.step(usize::MAX);
        b.step(usize::MAX);
    }
    assert_eq!(a.cells(), b.cells());
    for name in ["ash", "smoke", "steam", "stone"] {
        let element = a.rules().id(name).unwrap();
        assert!(
            a.cells().iter().any(|c| c.element == element),
            "chain did not produce {name}"
        );
    }
    let sand = a.rules().id("sand").unwrap();
    assert!(a.cells().iter().any(|c| c.element == sand));
}

#[test]
fn default_world_heavy_play_smoke_keeps_work_bounded() {
    let mut world = World::new(192, 120, 99, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    world.set_wind(crate::WindDirection::East, 1);
    let fire = world.rules().id("fire").unwrap();
    let tree = world.rules().id("tree").unwrap();
    let water = world.rules().id("water").unwrap();
    for y in 30..90 {
        for x in 40..150 {
            world.paint(x, y, tree);
        }
    }
    for x in 40..150 {
        world.paint(x, 60, fire);
    }
    for frame in 0..240 {
        for x in 48..64 {
            world.paint((x + frame % 96) % world.width(), 55, water);
        }
        assert_eq!(world.step(12_000), 12_000);
    }
    let air = world.rules().id("air").unwrap();
    assert!(world.cells().iter().any(|c| c.element != air));
}

#[test]
fn each_update_obeys_work_budget_and_paint_is_bounded() {
    let mut world = world(1);
    assert_eq!(world.step(7), 7);
    let sand = world.rules().id("sand").unwrap();
    world.paint(100, 0, sand);
    assert_eq!(world.step(3), 3);
    world.paint(0, 0, usize::MAX);
    assert_eq!(world.cells()[0].element, world.rules().id("air").unwrap());
}

fn guaranteed_world(seed: u64, pairs: &[(&str, &str)]) -> World {
    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    for interaction in &mut rules.interactions {
        if pairs.iter().any(|(when, with)| {
            rules.elements[interaction.when].name == *when
                && rules.elements[interaction.with].name == *with
        }) {
            interaction.chance = 1.0;
            interaction.downwind_bonus = 0.0;
        }
    }
    World::new(16, 12, seed, rules).unwrap()
}

fn put(world: &mut World, x: usize, y: usize, element: &str) {
    let id = world.rules().id(element).unwrap();
    let index = y * world.width() + x;
    world.cells[index] = crate::Cell {
        element: id,
        age: 0,
    };
}

fn has(world: &World, x: usize, y: usize, element: &str) -> bool {
    world.cells()[y * world.width() + x].element == world.rules().id(element).unwrap()
}

#[test]
fn default_rules_include_the_remaining_starting_elements() {
    let rules = Rules::load(DEFAULT_RULES).unwrap();
    for name in [
        "soil",
        "ice",
        "plant",
        "seed",
        "lava",
        "oil",
        "deadwood",
        "leaf_litter",
        "humus",
        "fungus",
        "beetle",
        "termite",
    ] {
        assert!(rules.id(name).is_some(), "missing {name}");
    }
}

#[test]
fn soil_ash_water_and_seed_interactions_form_regrowth_chain() {
    let mut hydrated = guaranteed_world(5, &[("soil", "water")]);
    put(&mut hydrated, 0, 0, "soil");
    put(&mut hydrated, 1, 0, "water");
    hydrated.step(1);
    assert!(has(&hydrated, 0, 0, "wet_soil"));
    assert!(has(&hydrated, 1, 0, "wet_soil"));

    let mut enriched = guaranteed_world(6, &[("ash", "water")]);
    put(&mut enriched, 0, 0, "ash");
    put(&mut enriched, 1, 0, "water");
    enriched.step(1);
    assert!(has(&enriched, 0, 0, "fertile_soil"));
    assert!(has(&enriched, 1, 0, "fertile_soil"));

    let mut sprouted = guaranteed_world(7, &[("seed", "wet_soil"), ("seed", "fertile_soil")]);
    put(&mut sprouted, 0, 0, "seed");
    put(&mut sprouted, 1, 0, "wet_soil");
    sprouted.step(1);
    assert!(has(&sprouted, 0, 0, "plant"));
    assert!(has(&sprouted, 1, 0, "soil"));

    let mut fertile_sprout = guaranteed_world(71, &[("seed", "fertile_soil")]);
    put(&mut fertile_sprout, 0, 0, "seed");
    put(&mut fertile_sprout, 1, 0, "fertile_soil");
    fertile_sprout.step(1);
    assert!(has(&fertile_sprout, 0, 0, "plant"));
    assert!(has(&fertile_sprout, 1, 0, "soil"));
}

#[test]
fn lava_water_ice_and_oil_interactions_are_data_driven() {
    let mut lava = guaranteed_world(8, &[("lava", "water")]);
    put(&mut lava, 0, 0, "lava");
    put(&mut lava, 1, 0, "water");
    lava.step(1);
    assert!(has(&lava, 0, 0, "stone"));
    assert!(has(&lava, 1, 0, "steam"));

    let mut ice = guaranteed_world(9, &[("fire", "ice")]);
    put(&mut ice, 0, 0, "fire");
    put(&mut ice, 1, 0, "ice");
    ice.step(1);
    assert!(has(&ice, 1, 0, "water"));

    let mut lava_ice = guaranteed_world(91, &[("lava", "ice")]);
    put(&mut lava_ice, 0, 0, "lava");
    put(&mut lava_ice, 1, 0, "ice");
    lava_ice.step(1);
    assert!(has(&lava_ice, 0, 0, "stone"));
    assert!(has(&lava_ice, 1, 0, "steam"));

    let mut ignited = guaranteed_world(10, &[("fire", "oil"), ("oil_fire", "tree")]);
    put(&mut ignited, 0, 0, "fire");
    put(&mut ignited, 1, 0, "oil");
    ignited.step(1);
    assert!(has(&ignited, 1, 0, "oil_fire"));
    let mut hot = guaranteed_world(101, &[("oil_fire", "tree")]);
    put(&mut hot, 0, 0, "oil_fire");
    put(&mut hot, 1, 0, "tree");
    hot.step(1);
    assert!(has(&hot, 1, 0, "fire"));
}

#[test]
fn oil_floats_above_water_by_data_driven_density() {
    let mut world = World::new(3, 2, 11, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    for x in [0, 2] {
        put(&mut world, x, 0, "stone");
        put(&mut world, x, 1, "stone");
    }
    put(&mut world, 1, 0, "water");
    put(&mut world, 1, 1, "oil");
    world.step(6);
    assert!(has(&world, 1, 0, "oil"));
    assert!(has(&world, 1, 1, "water"));
}

#[test]
fn wind_drifts_light_materials_and_boosts_downwind_fire_deterministically() {
    for name in ["ash", "smoke", "steam"] {
        let mut rules = Rules::load(DEFAULT_RULES).unwrap();
        let id = rules.id(name).unwrap();
        rules.elements[id].wind_response = 1.0;
        let mut drift = World::new(16, 12, 12, rules).unwrap();
        drift.set_wind(crate::WindDirection::East, 4);
        put(&mut drift, 0, 0, name);
        drift.step(1);
        assert!(has(&drift, 1, 0, name), "{name} did not drift east");
    }

    let mut downwind = guaranteed_world(13, &[("fire", "tree")]);
    for interaction in &mut downwind.rules.interactions {
        if downwind.rules.elements[interaction.when].name == "fire"
            && downwind.rules.elements[interaction.with].name == "tree"
        {
            interaction.chance = 0.0;
            interaction.downwind_bonus = 1.0;
        }
    }
    downwind.set_wind(crate::WindDirection::East, 4);
    put(&mut downwind, 1, 1, "fire");
    put(&mut downwind, 2, 1, "tree");
    put(&mut downwind, 0, 1, "tree");
    downwind.step(18);
    assert!(has(&downwind, 2, 1, "fire"));
    assert!(has(&downwind, 0, 1, "tree"));
}

#[test]
fn local_gust_is_bounded_and_respects_world_edges() {
    let mut world = world(14);
    world.gust(0, 0, 2, crate::WindDirection::East, 99);
    put(&mut world, 0, 0, "smoke");
    world.step(1);
    assert!(has(&world, 1, 0, "smoke"));
}

#[test]
fn trees_and_plants_die_into_seed_bearing_organic_matter() {
    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    let tree = rules.id("tree").unwrap();
    let plant = rules.id("plant").unwrap();
    rules.elements[tree].lifetime = 1;
    rules.elements[plant].lifetime = 1;
    let mut world = World::new(16, 12, 15, rules).unwrap();
    put(&mut world, 0, 0, "tree");
    put(&mut world, 2, 1, "plant");
    world.step(19);
    assert!(has(&world, 0, 0, "deadwood"));
    assert!(has(&world, 2, 1, "leaf_litter"));
    assert!(has(&world, 2, 0, "seed"));
}

#[test]
fn damp_organic_matter_wakes_decomposers_and_returns_fertile_soil() {
    let wake_pairs = [
        ("wet_soil", "deadwood", "fungus"),
        ("wet_soil", "leaf_litter", "beetle"),
        ("wet_soil", "humus", "termite"),
    ];
    for (index, (soil, matter, decomposer)) in wake_pairs.into_iter().enumerate() {
        let mut world = guaranteed_world(20 + index as u64, &[(soil, matter)]);
        put(&mut world, 0, 0, soil);
        put(&mut world, 1, 0, matter);
        world.step(1);
        assert!(has(&world, 0, 0, "fertile_soil"));
        assert!(has(&world, 1, 0, decomposer));
    }

    let meals = [
        ("fungus", "deadwood"),
        ("beetle", "leaf_litter"),
        ("termite", "humus"),
    ];
    for (index, (decomposer, matter)) in meals.into_iter().enumerate() {
        let mut world = guaranteed_world(30 + index as u64, &[(decomposer, matter)]);
        put(&mut world, 0, 0, decomposer);
        put(&mut world, 1, 0, matter);
        world.step(1);
        assert!(has(&world, 0, 0, "fertile_soil"));
        assert!(has(&world, 1, 0, "fertile_soil"));
    }
}

#[test]
fn dry_soil_does_not_sprout_seeds() {
    let mut world = world(42);
    let y = world.height() - 1;
    let width = world.width();
    put(&mut world, 0, y, "seed");
    put(&mut world, 1, y, "soil");
    world.step(width * y + 1);
    assert!(has(&world, 0, y, "seed"));
    assert!(has(&world, 1, y, "soil"));
}

#[test]
fn ash_and_deadwood_make_burnable_humus() {
    let mut world = guaranteed_world(40, &[("deadwood", "ash")]);
    put(&mut world, 0, 0, "deadwood");
    put(&mut world, 1, 0, "ash");
    world.step(1);
    assert!(has(&world, 0, 0, "humus"));
    assert!(has(&world, 1, 0, "humus"));
}

#[test]
fn seeded_burned_bed_recovers_through_water_and_fertile_soil() {
    let mut world = guaranteed_world(41, &[("ash", "water"), ("fertile_soil", "seed")]);
    put(&mut world, 0, 0, "ash");
    put(&mut world, 1, 0, "water");
    put(&mut world, 2, 0, "seed");
    world.step(3);
    assert!(has(&world, 0, 0, "fertile_soil"));
    assert!(has(&world, 1, 0, "soil"));
    assert!(has(&world, 2, 0, "plant"));
}

#[test]
fn burned_patch_regrows_during_a_short_play_session_at_default_rates() {
    let mut world = World::new(192, 120, 42, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    let soil = world.rules().id("soil").unwrap();
    let stone = world.rules().id("stone").unwrap();
    let ash = world.rules().id("ash").unwrap();
    let water = world.rules().id("water").unwrap();
    let seed = world.rules().id("seed").unwrap();
    for x in 85..100 {
        world.paint(x, 100, soil);
        world.paint(x, 101, stone);
    }
    world.paint(90, 99, ash);
    world.paint(91, 99, water);
    world.paint(92, 99, seed);
    for _ in 0..300 {
        world.step(12_000);
    }
    let plant = world.rules().id("plant").unwrap();
    assert!(world.cells().iter().any(|cell| cell.element == plant));
}
