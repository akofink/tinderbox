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
fn atmosphere_rules_validate_and_round_trip_with_legacy_defaults() {
    let rules = Rules::load(DEFAULT_RULES).unwrap();
    let text = rules.to_toml().unwrap();
    let loaded = Rules::load(&text).unwrap();
    assert_eq!(
        loaded.atmosphere.freezing_point,
        rules.atmosphere.freezing_point
    );
    assert_eq!(
        loaded.atmosphere.condensation_humidity,
        rules.atmosphere.condensation_humidity
    );
    let tree_id = rules.id("tree").unwrap();
    assert_eq!(
        loaded.elements[tree_id].carries_moisture,
        rules.elements[tree_id].carries_moisture
    );
    assert_eq!(
        loaded.elements[tree_id].wet_flammability,
        rules.elements[tree_id].wet_flammability
    );
    let legacy = DEFAULT_RULES
        .lines()
        .filter(|line| {
            !line.starts_with("carries_moisture =")
                && !line.starts_with("moisture_rate =")
                && !line.starts_with("wet_flammability =")
                && !line.starts_with("oxygen_consumption =")
                && !line.starts_with("oxygen_diffusion =")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        Rules::load(&legacy).unwrap().atmosphere.oxygen_consumption,
        0.35
    );
    let broken = text.replace("melting_point = 2.0", "melting_point = -1.0");
    assert!(
        Rules::load(&broken)
            .unwrap_err()
            .to_string()
            .contains("thresholds must be ordered")
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
fn dirty_texture_regions_coalesce_changes_and_reset_marks_the_world() {
    let mut world = world(5);
    let sand = world.rules().id("sand").unwrap();
    assert_eq!(world.pending_dirty_regions(), 0);
    world.paint(1, 1, sand);
    world.paint(2, 2, sand);
    assert_eq!(world.pending_dirty_regions(), 1);
    assert_eq!(
        world.pop_dirty_region(),
        Some(crate::DirtyRegion {
            x: 0,
            y: 0,
            width: 16,
            height: 16,
        })
    );
    let air = world.rules().id("air").unwrap();
    world.paint(1, 1, air);
    world.reset(6);
    assert_eq!(world.pending_dirty_regions(), 4);
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
        moisture: 0,
        burn_progress: 0,
        body_id: 0,
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
fn trees_die_into_organic_matter_and_plants_mature_into_trees() {
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
    assert!(has(&world, 2, 1, "tree"));
    assert!(has(&world, 2, 0, "tree"));
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
fn burning_a_tree_drops_a_seed_and_wet_ash_wakes_fungus() {
    let mut burned = guaranteed_world(3, &[("fire", "tree")]);
    put(&mut burned, 0, 0, "fire");
    put(&mut burned, 1, 0, "tree");
    burned.step(1);
    assert!(has(&burned, 1, 0, "fire"));
    let seed = burned.rules().id("seed").unwrap();
    assert!(burned.cells().iter().any(|cell| cell.element == seed));

    let mut wet = guaranteed_world(4, &[("ash", "water")]);
    put(&mut wet, 0, 0, "ash");
    put(&mut wet, 1, 0, "water");
    wet.step(1);
    assert!(has(&wet, 0, 0, "fertile_soil"));
    assert!(has(&wet, 1, 0, "fertile_soil"));
    assert!(has(&wet, 0, 1, "fungus") || has(&wet, 2, 0, "fungus") || has(&wet, 1, 1, "fungus"));
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
fn seeds_wait_for_hot_ash_to_cool_before_sprouting() {
    let mut world = guaranteed_world(42, &[("seed", "ash"), ("ash", "seed")]);
    let ignition_point = world.rules().atmosphere.ignition_point;
    for field in &mut world.atmosphere {
        field.temperature = ignition_point;
    }
    put(&mut world, 0, 0, "seed");
    put(&mut world, 0, 1, "ash");
    world.scheduler.enqueue_focus(0);
    world.step(1);
    assert!(has(&world, 0, 0, "seed"));
    assert!(has(&world, 0, 1, "ash"));

    for field in &mut world.atmosphere {
        field.temperature = 20.0;
    }
    world.scheduler.enqueue_focus(0);
    world.step(1);
    assert!(has(&world, 0, 0, "plant"));
    assert!(has(&world, 0, 1, "fertile_soil"));
}

#[test]
fn atmosphere_transfers_heat_and_phase_thresholds_are_data_driven() {
    let mut a = world(61);
    let mut b = world(61);
    put(&mut a, 4, 4, "lava");
    put(&mut b, 4, 4, "lava");
    a.step(32);
    b.step(32);
    assert_eq!(a.atmosphere_at(4, 4), b.atmosphere_at(4, 4));
    assert!(a.atmosphere_at(4, 4).unwrap().temperature > 20.0);
    assert!(a.atmosphere_at(12, 4).unwrap().temperature > 20.0);

    let mut phases = world(62);
    phases.atmosphere[0].temperature = -5.0;
    put(&mut phases, 0, 0, "water");
    phases.step(1);
    assert!(has(&phases, 0, 0, "ice"));
    phases.atmosphere[0].temperature = 10.0;
    put(&mut phases, 0, 0, "ice");
    phases.scheduler.enqueue_focus(0);
    phases.step(1);
    assert!(
        phases
            .cells()
            .iter()
            .any(|cell| cell.element == phases.rules().id("water").unwrap())
    );

    phases.rules.atmosphere.boiling_point = 30.0;
    phases.atmosphere[0].temperature = 40.0;
    put(&mut phases, 0, 0, "water");
    phases.scheduler.enqueue_focus(0);
    phases.step(1);
    assert!(has(&phases, 0, 0, "steam"));

    let mut ignited = world(65);
    ignited.atmosphere[0].temperature = 250.0;
    put(&mut ignited, 0, 0, "tree");
    ignited.step(1);
    assert!(has(&ignited, 0, 0, "fire"));
}

#[test]
fn humid_air_water_production_is_bounded_and_does_not_freeze_above_threshold() {
    let mut world = World::new(64, 48, 630, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    world.set_ambient(20, 100, 21);
    let ice = world.rules().id("ice").unwrap();
    let water_materials =
        ["water", "ice", "steam", "cloud"].map(|name| world.rules().id(name).unwrap());
    for _ in 0..300 {
        world.step(usize::MAX);
    }
    let ice_cells = world
        .cells()
        .iter()
        .filter(|cell| cell.element == ice)
        .count();
    let water_mass_cells = world
        .cells()
        .iter()
        .filter(|cell| water_materials.contains(&cell.element))
        .count();
    assert_eq!(ice_cells, 0, "20 C humid air must not create ice");
    assert!(
        water_mass_cells <= world.atmosphere.len() * 3,
        "condensed water should be bounded by the initial vapor reservoir: {water_mass_cells} water-bearing cells across {} atmosphere cells",
        world.atmosphere.len()
    );
}

#[test]
fn humid_cool_air_condenses_cloud_and_dew_and_pressure_drives_wind() {
    let mut wet_air = world(63);
    wet_air.atmosphere[0].temperature = 1.0;
    wet_air.atmosphere[0].humidity = 100.0;
    for x in 0..8 {
        put(&mut wet_air, x, 8, "soil");
    }
    wet_air.step(16);
    let cloud = wet_air.rules().id("cloud").unwrap();
    let water = wet_air.rules().id("water").unwrap();
    assert!(wet_air.cells().iter().any(|cell| cell.element == cloud));
    assert!((0..8).any(|x| wet_air.cells()[7 * wet_air.width() + x].element == water));

    let mut pressure = world(64);
    pressure.atmosphere[0].pressure = 0.0;
    pressure.atmosphere[2].pressure = 10.0;
    put(&mut pressure, 8, 1, "smoke");
    pressure.scheduler.enqueue_focus(8 + pressure.width);
    pressure.step(1);
    assert!(has(&pressure, 7, 1, "smoke"));
}

#[test]
fn lava_heats_a_lake_and_cloud_rain_wets_soil_for_regrowth() {
    let mut lake = world(66);
    put(&mut lake, 4, 4, "lava");
    put(&mut lake, 7, 7, "water");
    for (x, y) in [(4, 5), (3, 4), (5, 4), (7, 8), (6, 7), (8, 7)] {
        put(&mut lake, x, y, "stone");
    }
    for _ in 0..16 {
        lake.step(768);
    }
    assert!(lake.atmosphere_at(4, 4).unwrap().temperature > 100.0);
    assert!(
        lake.cells()
            .iter()
            .any(|cell| cell.element == lake.rules().id("steam").unwrap())
    );

    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    let cloud = rules.id("cloud").unwrap();
    rules.elements[cloud].lifetime = 1;
    for rule in &mut rules.interactions {
        let when = &rules.elements[rule.when].name;
        let with = &rules.elements[rule.with].name;
        if matches!(
            (when.as_str(), with.as_str()),
            ("water", "soil") | ("soil", "water") | ("seed", "wet_soil") | ("wet_soil", "seed")
        ) {
            rule.chance = 1.0;
        }
    }
    let mut rain = World::new(32, 24, 67, rules).unwrap();
    put(&mut rain, 4, 0, "cloud");
    put(&mut rain, 4, 9, "soil");
    put(&mut rain, 3, 9, "seed");
    put(&mut rain, 3, 10, "stone");
    put(&mut rain, 4, 10, "stone");
    for _ in 0..32 {
        rain.step(768);
    }
    assert!(
        rain.cells()
            .iter()
            .any(|cell| cell.element == rain.rules().id("plant").unwrap())
    );
}

#[test]
fn fire_front_spreads_downwind_and_respects_humidity_and_oxygen() {
    use crate::level::{Generator, Level};
    use std::collections::BTreeMap;
    let rules = Rules::load(DEFAULT_RULES).unwrap();
    let burned_fraction = |humidity, oxygen, direction| {
        let level = Level {
            version: 1,
            width: 192,
            height: 120,
            seed: 731,
            generator: Generator::Hills,
            wind: crate::Wind {
                direction,
                strength: 1,
            },
            ambient_temperature: 20,
            ambient_humidity: humidity,
            ambient_oxygen: oxygen,
            amounts: BTreeMap::from([
                ("stone".into(), 100),
                ("soil".into(), 100),
                ("tree".into(), 75),
            ]),
            spark: true,
        };
        let mut world = level.generate(rules.clone()).unwrap();
        let tree = world.rules().id("tree").unwrap();
        let initial = world
            .cells()
            .iter()
            .filter(|cell| cell.element == tree)
            .count();
        for _ in 0..60 {
            world.step(usize::MAX);
        }
        let remaining = world
            .cells()
            .iter()
            .filter(|cell| cell.element == tree)
            .count();
        (initial - remaining) as f32 / initial as f32
    };
    let dry_following = burned_fraction(0, 21, crate::WindDirection::East);
    let dry_headwind = burned_fraction(0, 21, crate::WindDirection::West);
    let moist_following = burned_fraction(80, 21, crate::WindDirection::East);
    let airless = burned_fraction(0, 0, crate::WindDirection::East);
    assert!(
        dry_following > 0.5,
        "dry following-wind forest burned {:.1}%",
        dry_following * 100.0
    );
    assert!(
        dry_following > dry_headwind + 0.15,
        "following wind {:.1}%, headwind {:.1}%",
        dry_following * 100.0,
        dry_headwind * 100.0
    );
    assert!(
        dry_following > moist_following + 0.15,
        "dry {:.1}%, moist {:.1}%",
        dry_following * 100.0,
        moist_following * 100.0
    );
    assert_eq!(airless, 0.0, "zero oxygen must not burn trees");
}

#[test]
fn dry_forest_fire_front_leaves_a_regrowing_canopy() {
    let rules = Rules::load(DEFAULT_RULES).unwrap();
    let level = crate::level::Preset::DryForest.level(&rules, 96, 64);
    let mut world = level.generate(rules).unwrap();
    let count = |world: &World, name: &str| {
        let id = world.rules().id(name).unwrap();
        world
            .cells()
            .iter()
            .filter(|cell| cell.element == id)
            .count()
    };
    let initial_trees = count(&world, "tree");
    let mut fire_clear_frame = None;
    for frame in 1..=600 {
        world.step(usize::MAX);
        if frame > 24 && count(&world, "fire") == 0 {
            fire_clear_frame = Some(frame);
            break;
        }
    }
    assert!(
        fire_clear_frame.is_some(),
        "the Dry forest fire front should finish within 600 simulation sweeps"
    );
    let trees_after_fire = count(&world, "tree");
    assert!(
        trees_after_fire < initial_trees,
        "the preset spark should burn a patch"
    );

    for _ in 0..300 {
        world.step(usize::MAX);
    }
    let recovered_trees = count(&world, "tree");
    assert!(
        recovered_trees >= trees_after_fire + 8,
        "a burned patch should grow a trunk-and-crown canopy after the front passes"
    );
    for _ in 0..600 {
        world.step(usize::MAX);
    }
    let settled_trees = count(&world, "tree");
    assert!(
        settled_trees <= initial_trees * 2,
        "a forest left alone should not overgrow: {settled_trees} tree cells from {initial_trees}"
    );
}

#[test]
fn humidity_equilibrates_material_moisture_and_reduces_flammability() {
    let mut world = world(901);
    let tree = world.rules().id("tree").unwrap();
    world.paint(4, 4, world.rules().id("tree").unwrap());
    let atmosphere_index = (4 / crate::world::ATMOSPHERE_CELL_SIZE) * world.atmosphere_width
        + (4 / crate::world::ATMOSPHERE_CELL_SIZE);
    world.atmosphere[atmosphere_index].humidity = 100.0;
    for _ in 0..20 {
        world.step(1);
    }
    assert!(world.cells()[4 * world.width() + 4].moisture > 0);
    let cell_index = 4 * world.width() + 4;
    let dry_factor = world.flammability_factor(cell_index, tree);
    world.cells[cell_index].moisture = 255;
    let wet_factor = world.flammability_factor(cell_index, tree);
    assert!(wet_factor < dry_factor);
    assert!(wet_factor < 0.2);
}

#[test]
fn material_attributes_survive_rules_reload_and_reset_to_clean_defaults() {
    let mut world = world(903);
    let tree = world.rules().id("tree").unwrap();
    world.paint(8, 8, tree);
    let index = 8 * world.width() + 8;
    world.cells[index].moisture = 187;
    world.cells[index].burn_progress = 63;
    let rules = world.rules().to_toml().unwrap();
    world.load_rules(&rules).unwrap();
    assert_eq!(world.cells()[index].moisture, 187);
    assert_eq!(world.cells()[index].burn_progress, 63);
    world.reset(903);
    assert_eq!(world.cells()[index].moisture, 0);
    assert_eq!(world.cells()[index].burn_progress, 0);
}

#[test]
fn zero_oxygen_smothering_is_deterministic_and_reset_restores_level_oxygen() {
    let mut world = world(902);
    let fire = world.rules().id("fire").unwrap();
    world.set_ambient(20, 45, 0);
    put(&mut world, 16, 16, "fire");
    for _ in 0..3 {
        world.step(usize::MAX);
    }
    assert!(!world.cells().iter().any(|cell| cell.element == fire));
    world.set_ambient(20, 45, 0);
    world.reset(902);
    assert_eq!(world.atmosphere[0].oxygen, 0.0);
}

#[test]
fn burned_patch_regrows_a_recognizable_canopy_within_a_short_play_session() {
    let mut world = World::new(192, 120, 42, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    let soil = world.rules().id("soil").unwrap();
    let stone = world.rules().id("stone").unwrap();
    let ash = world.rules().id("ash").unwrap();
    let seed = world.rules().id("seed").unwrap();
    for x in 85..100 {
        world.paint(x, 100, soil);
        world.paint(x, 101, stone);
    }
    world.paint(90, 99, ash);
    world.paint(90, 98, seed);
    for _ in 0..300 {
        world.step(12_000);
    }
    let plant = world.rules().id("plant").unwrap();
    assert!(
        world.cells().iter().any(|cell| cell.element == plant),
        "an ash bed and seed should produce a sapling"
    );
    for _ in 300..450 {
        world.step(12_000);
    }
    let tree = world.rules().id("tree").unwrap();
    let canopy_cells = world
        .cells()
        .iter()
        .filter(|cell| cell.element == tree)
        .count();
    assert!(
        canopy_cells >= 8,
        "burned patch should regrow trunks and crowns, got {canopy_cells} tree cells"
    );
}

#[test]
fn empty_boat_floats_as_one_body_and_cargo_pushes_it_down() {
    let mut world = World::new(32, 24, 1, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    for x in 1..world.width() - 1 {
        put(&mut world, x, 15, "water");
        put(&mut world, x, 16, "water");
        put(&mut world, x, 17, "stone");
    }
    let right_edge = world.width() - 1;
    for y in 15..=16 {
        put(&mut world, 0, y, "stone");
        put(&mut world, right_edge, y, "stone");
    }
    assert!(world.place_boat(16, 14));
    let wood = world.rules().id("wood").unwrap();
    let initial: Vec<_> = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.element == wood).then_some(index))
        .collect();
    assert_eq!(initial.len(), 23);
    for _ in 0..6 {
        world.step(768);
    }
    let floating: Vec<_> = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.element == wood).then_some(index))
        .collect();
    assert_eq!(floating.len(), initial.len());
    assert_eq!(
        floating.iter().map(|index| index / world.width()).min(),
        initial.iter().map(|index| index / world.width()).min(),
        "empty body remains at its waterline"
    );
    let translation = floating[0] as isize - initial[0] as isize;
    assert!(
        initial
            .iter()
            .zip(&floating)
            .all(|(before, after)| *after as isize - *before as isize == translation),
        "the hull translates as one piece"
    );
    assert_eq!(
        world
            .cells()
            .iter()
            .filter(|cell| cell.body_id != 0)
            .count(),
        23
    );
    let mut bobbed = false;
    for _ in 0..64 {
        let anchor = world
            .cells()
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| (cell.body_id != 0).then_some(index))
            .min()
            .unwrap();
        world.scheduler.enqueue_focus(anchor);
        world.step(23);
        let top = world
            .cells()
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| (cell.body_id != 0).then_some(index / world.width()))
            .min()
            .unwrap();
        bobbed |= top
            != initial
                .iter()
                .map(|index| index / world.width())
                .min()
                .unwrap();
    }
    assert!(bobbed, "the fixed-step buoyancy phase makes the hull bob");

    let sand = world.rules().id("sand").unwrap();
    let boat_y = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.body_id != 0).then_some(index / world.width()))
        .min()
        .unwrap();
    let boat_x = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| {
            (cell.body_id != 0 && index / world.width() == boat_y).then_some(index % world.width())
        })
        .nth(3)
        .unwrap();
    for row in 1..=4 {
        world.paint(boat_x, boat_y - row, sand);
    }
    for _ in 0..8 {
        world.step(768);
    }
    let loaded: Vec<_> = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.element == wood).then_some(index))
        .collect();
    assert_eq!(
        loaded.len(),
        23,
        "group movement neither drops nor duplicates hull cells"
    );
    assert!(
        loaded
            .iter()
            .map(|index| index / world.width())
            .min()
            .unwrap()
            > initial
                .iter()
                .map(|index| index / world.width())
                .min()
                .unwrap(),
        "heavy cargo should lower the boat"
    );
    assert_eq!(
        world
            .cells()
            .iter()
            .filter(|cell| cell.body_id != 0)
            .count(),
        27
    );
}

#[test]
fn a_hole_reduces_displacement_and_sinks_a_loaded_boat() {
    let mut world = World::new(32, 24, 4, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    for x in 1..world.width() - 1 {
        put(&mut world, x, 15, "water");
        put(&mut world, x, 16, "water");
        put(&mut world, x, 17, "stone");
    }
    let right_edge = world.width() - 1;
    for y in 15..=16 {
        put(&mut world, 0, y, "stone");
        put(&mut world, right_edge, y, "stone");
    }
    assert!(world.place_boat(16, 14));
    let sand = world.rules().id("sand").unwrap();
    world.paint(16, 11, sand);
    for _ in 0..4 {
        world.step(768);
    }
    let wood = world.rules().id("wood").unwrap();
    let hull: Vec<_> = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.element == wood && cell.body_id != 0).then_some(index))
        .collect();
    let baseline_y = hull
        .iter()
        .map(|index| index / world.width())
        .min()
        .unwrap();
    let keel_y = hull
        .iter()
        .map(|index| index / world.width())
        .max()
        .unwrap();
    let keel_x = hull
        .iter()
        .filter(|index| **index / world.width() == keel_y)
        .map(|index| *index % world.width())
        .nth(3)
        .unwrap();
    let air = world.rules().id("air").unwrap();
    world.paint(keel_x, keel_y, air);
    for _ in 0..8 {
        world.step(768);
    }
    let remaining: Vec<_> = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.element == wood && cell.body_id != 0).then_some(index))
        .collect();
    assert!(remaining.len() < 23, "erased hull plank makes a hole");
    assert!(
        remaining
            .iter()
            .map(|index| index / world.width())
            .min()
            .unwrap()
            > baseline_y,
        "a loaded leaky hull sinks"
    );
}

#[test]
fn wind_moves_the_hull_as_a_single_tagged_group() {
    let mut world = World::new(48, 24, 3, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    for x in 1..world.width() - 1 {
        put(&mut world, x, 15, "water");
        put(&mut world, x, 16, "water");
        put(&mut world, x, 17, "stone");
    }
    assert!(world.place_boat(20, 14));
    let before: Vec<_> = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.body_id != 0).then_some(index))
        .collect();
    world.set_wind(crate::WindDirection::East, 2);
    for _ in 0..4 {
        world.step(1_152);
    }
    let after: Vec<_> = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.body_id != 0).then_some(index))
        .collect();
    assert_eq!(after.len(), before.len());
    let translation = after[0] as isize - before[0] as isize;
    assert!(translation > 0, "east wind should drift the body east");
    assert!(
        before
            .iter()
            .zip(&after)
            .all(|(source, target)| *target as isize - *source as isize == translation),
        "body members keep their exact relative positions"
    );
}

#[test]
fn asymmetric_water_current_drifts_the_body() {
    let mut world = World::new(32, 24, 5, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    for x in 12..=20 {
        put(&mut world, x, 15, "water");
    }
    assert!(world.place_boat(16, 14));
    put(&mut world, 12, 12, "water");
    let anchor = 12 * world.width() + 13;
    world.scheduler.enqueue_focus(anchor);
    assert_eq!(world.step(23), 23);
    let leftmost = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.body_id != 0).then_some(index % world.width()))
        .min()
        .unwrap();
    assert_eq!(leftmost, 11, "one-sided water flow nudges the boat left");
}

#[test]
fn burning_body_detaches_changed_wood_into_loose_cells() {
    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    for rule in &mut rules.interactions {
        if rules.elements[rule.when].name == "fire" && rules.elements[rule.with].name == "wood" {
            rule.chance = 1.0;
        }
    }
    let mut world = World::new(32, 24, 2, rules).unwrap();
    for x in 0..world.width() {
        put(&mut world, x, 15, "water");
    }
    assert!(world.place_boat(16, 14));
    put(&mut world, 16, 11, "fire");
    for _ in 0..12 {
        world.step(768);
    }
    let loose_burnt = world.cells().iter().any(|cell| {
        cell.body_id == 0
            && matches!(
                world.rules().elements[cell.element].name.as_str(),
                "fire" | "ash"
            )
    });
    let attached_wood = world
        .cells()
        .iter()
        .filter(|cell| cell.body_id != 0 && world.rules().elements[cell.element].name == "wood")
        .count();
    assert!(loose_burnt, "burned hull cells leave the rigid body");
    assert!(attached_wood < 23, "fire breaks the boat apart");
}
