use crate::{
    DEFAULT_RULES, Rules, World,
    level::{Generator, Level},
};

fn world(seed: u64) -> World {
    World::new(32, 24, seed, Rules::load(DEFAULT_RULES).unwrap()).unwrap()
}

#[test]
fn decomposers_persist_on_a_well_fed_forest_floor_and_decline_without_food() {
    let rules = Rules::load(DEFAULT_RULES).unwrap();
    let recipe = Level {
        version: 1,
        width: 20,
        height: 16,
        fit_window: false,
        seed: 20_261_013,
        generator: Generator::Hills,
        mode: crate::level::LevelMode::Fixed,
        wind: Default::default(),
        ambient_temperature: 20,
        ambient_humidity: 100,
        ambient_oxygen: 21,
        amounts: [
            ("stone", 100),
            ("soil", 100),
            ("tree", 12),
            ("wet_soil", 45),
            ("fertile_soil", 10),
            ("deadwood", 45),
            ("leaf_litter", 45),
            ("humus", 45),
            ("fungus", 75),
            ("beetle", 55),
            ("termite", 45),
        ]
        .into_iter()
        .map(|(name, amount)| (name.to_owned(), amount))
        .collect(),
        spark: false,
        boat: false,
        boat_sand: 0,
        boat_burning: false,
    };
    let mut fed = recipe.generate(rules.clone()).unwrap();
    let mut starved = recipe.generate(rules).unwrap();
    let food_names = [
        "deadwood",
        "leaf_litter",
        "humus",
        "wet_soil",
        "fertile_soil",
    ];
    for world in [&mut starved] {
        for cell in &mut world.cells {
            if food_names.contains(&world.rules.elements[cell.element].name.as_str()) {
                cell.element = world.rules.id("stone").unwrap();
            }
        }
    }
    // Rain from the newly modeled humid atmosphere can restore edible wet soil.
    starved.set_ambient(20, 0, 21);
    let counts = |world: &World, name: &str| {
        let id = world.rules().id(name).unwrap();
        world
            .cells()
            .iter()
            .filter(|cell| cell.element == id)
            .count()
    };
    let advance_seconds = |world: &mut World, seconds: usize| {
        // The 120x75 showcase recipe gets about 80 full cell sweeps per simulated second
        // at the native 12,000-work, 60-frame budget. Preserve that per-cell update rate
        // in this smaller headless recipe so the test stays fast and deterministic.
        for _ in 0..seconds * 80 {
            world.step(usize::MAX);
        }
    };
    advance_seconds(&mut fed, 120);
    for (name, cap) in [("fungus", 150), ("beetle", 150), ("termite", 150)] {
        let count = counts(&fed, name);
        assert!(
            count > 0,
            "{name} should persist after 120 seconds; count={count}"
        );
        assert!(count < cap, "{name} should remain bounded; count={count}");
    }
    let initial = [
        counts(&starved, "fungus"),
        counts(&starved, "beetle"),
        counts(&starved, "termite"),
    ];
    assert!(initial.iter().all(|count| *count > 0));
    advance_seconds(&mut starved, 300);
    let remaining = [
        counts(&starved, "fungus"),
        counts(&starved, "beetle"),
        counts(&starved, "termite"),
    ];
    for (name, before, after) in [
        ("fungus", initial[0], remaining[0]),
        ("beetle", initial[1], remaining[1]),
        ("termite", initial[2], remaining[2]),
    ] {
        assert!(
            after < before,
            "starved {name} should decline: {before} -> {after}"
        );
    }
}

#[test]
fn forest_floor_critters_move_each_five_second_window() {
    let recipe = Level {
        version: 1,
        width: 120,
        height: 75,
        fit_window: false,
        seed: 20_261_013,
        generator: Generator::Hills,
        mode: crate::level::LevelMode::Fixed,
        wind: Default::default(),
        ambient_temperature: 20,
        ambient_humidity: 100,
        ambient_oxygen: 21,
        amounts: [
            ("stone", 100),
            ("soil", 100),
            ("tree", 12),
            ("wet_soil", 45),
            ("fertile_soil", 10),
            ("deadwood", 45),
            ("leaf_litter", 45),
            ("humus", 45),
            ("fungus", 75),
            ("beetle", 55),
            ("termite", 45),
        ]
        .into_iter()
        .map(|(name, amount)| (name.to_owned(), amount))
        .collect(),
        spark: false,
        boat: false,
        boat_sand: 0,
        boat_burning: false,
    };
    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    rules.ecology.breeding_chance = 0.0;
    let mut world = recipe.generate(rules).unwrap();
    let species = ["beetle", "termite"].map(|name| world.rules().id(name).unwrap());
    let positions = |world: &World, species| {
        world
            .cells()
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| (cell.element == species).then_some(index))
            .collect::<Vec<_>>()
    };
    let mut previous = species.map(|species| positions(&world, species));
    let fertile_soil = world.rules().id("fertile_soil").unwrap();
    let initial_fertile_soil = world
        .cells()
        .iter()
        .filter(|cell| cell.element == fertile_soil)
        .count();
    for window in 1..=3 {
        for _ in 0..5 * 80 {
            world.step(usize::MAX);
        }
        for (kind, species) in species.iter().enumerate() {
            let current = positions(&world, *species);
            assert!(
                !current.is_empty(),
                "living {} should remain",
                world.rules().elements[*species].name
            );
            for position in &current {
                let x = position % world.width();
                let y = position / world.width();
                let has_support = [
                    (x > 0).then_some(position - 1),
                    (x + 1 < world.width()).then_some(position + 1),
                    (y > 0).then_some(position - world.width()),
                    (y + 1 < world.height()).then_some(position + world.width()),
                ]
                .into_iter()
                .flatten()
                .any(|neighbor| {
                    world.cells()[neighbor].element != world.rules().id("air").unwrap()
                });
                assert!(
                    has_support,
                    "{} at ({x}, {y}) cannot be stranded in open air",
                    world.rules().elements[*species].name
                );
            }
            let moved = previous[kind]
                .iter()
                .filter(|position| !current.contains(position))
                .count();
            let fraction = moved as f64 / previous[kind].len() as f64;
            assert!(
                fraction > 0.5,
                "species {} moved only {:.1}% in five-second window {window} ({}/{})",
                world.rules().elements[*species].name,
                fraction * 100.0,
                moved,
                previous[kind].len()
            );
            previous[kind] = current;
        }
    }
    assert!(
        world
            .cells()
            .iter()
            .filter(|cell| cell.element == fertile_soil)
            .count()
            > initial_fertile_soil,
        "moving critters should eat organic matter and leave fertile soil"
    );
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
    let fire_id = rules.id("fire").unwrap();
    assert_eq!(
        loaded.elements[fire_id].alternate_decay_to,
        rules.elements[fire_id].alternate_decay_to
    );
    assert_eq!(
        loaded.elements[fire_id].alternate_decay_chance,
        rules.elements[fire_id].alternate_decay_chance
    );
    let legacy = DEFAULT_RULES
        .lines()
        .filter(|line| {
            !line.starts_with("carries_moisture =")
                && !line.starts_with("moisture_rate =")
                && !line.starts_with("wet_flammability =")
                && !line.starts_with("alternate_decay_to =")
                && !line.starts_with("alternate_decay_chance =")
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
fn alternate_decay_rules_load_validate_and_drive_lifecycle_outcomes() {
    let invalid_chance = DEFAULT_RULES.replace(
        "alternate_decay_chance = 0.1",
        "alternate_decay_chance = 1.5",
    );
    assert!(
        Rules::load(&invalid_chance)
            .unwrap_err()
            .to_string()
            .contains("alternate_decay_chance must be between 0 and 1")
    );
    let missing_target = DEFAULT_RULES.replace("alternate_decay_to = \"leaf_litter\"\n", "");
    assert!(
        Rules::load(&missing_target)
            .unwrap_err()
            .to_string()
            .contains("alternate_decay_chance requires alternate_decay_to")
    );

    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    let fire = rules.id("fire").unwrap();
    rules.elements[fire].lifetime = 1;
    rules.elements[fire].alternate_decay_chance = 1.0;
    let mut world = World::new(8, 8, 9, rules).unwrap();
    put(&mut world, 4, 4, "fire");
    world.step(usize::MAX);
    assert!(has(&world, 4, 4, "leaf_litter"));
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
fn dirty_texture_rectangles_mark_only_intersecting_regions() {
    let mut world = World::new(64, 32, 5, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    world.mark_dirty_rect(20, 2, 20, 10);
    assert_eq!(world.pending_dirty_regions(), 2);
    assert_eq!(world.pop_dirty_region().unwrap().x, 16);
    assert_eq!(world.pop_dirty_region().unwrap().x, 32);
    assert_eq!(world.pop_dirty_region(), None);

    world.mark_dirty_rect(usize::MAX, 0, 1, 1);
    assert_eq!(world.pending_dirty_regions(), 0);
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
        burial_age: 0,
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
fn default_forest_floor_life_colors_are_muted_and_distinct() {
    let rules = Rules::load(DEFAULT_RULES).unwrap();
    let color = |name: &str| rules.elements[rules.id(name).unwrap()].color;

    let beetle = color("beetle");
    let termite = color("termite");
    let fungus = color("fungus");
    assert_eq!(beetle, [117, 83, 56]);
    assert_eq!(termite, [151, 124, 78]);
    assert_eq!(fungus, [196, 184, 157]);
    assert!(fungus[0] > fungus[1] && fungus[1] > fungus[2]);
    assert!(fungus[0] - fungus[2] < 50, "fungus should be quiet cream");
    assert_ne!(beetle, termite);
    assert_ne!(beetle, fungus);
    assert_ne!(termite, fungus);
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
    let mut burning_slick = guaranteed_world(102, &[("oil_fire", "oil")]);
    put(&mut burning_slick, 0, 0, "oil_fire");
    put(&mut burning_slick, 1, 0, "oil");
    burning_slick.step(1);
    assert!(has(&burning_slick, 1, 0, "oil_fire"));
    let mut hot = guaranteed_world(101, &[("oil_fire", "tree")]);
    put(&mut hot, 0, 0, "oil_fire");
    put(&mut hot, 1, 0, "tree");
    hot.step(1);
    assert!(has(&hot, 1, 0, "fire"));
}

#[test]
fn sand_sinks_through_water_to_the_bottom_without_losing_cells() {
    let mut world = World::new(1, 8, 11, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    put(&mut world, 0, 0, "sand");
    for y in 1..7 {
        put(&mut world, 0, y, "water");
    }
    put(&mut world, 0, 7, "stone");
    let counts = element_counts(&world);
    for _ in 0..4_000 {
        world.step(8);
    }
    assert!(has(&world, 0, 6, "sand"));
    assert_eq!(element_counts(&world), counts);
}

#[test]
fn sand_block_settles_into_basin_within_ten_simulated_seconds() {
    let mut world = World::new(400, 250, 610, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    for x in 150..250 {
        put(&mut world, x, 200, "stone");
    }
    for y in 10..200 {
        put(&mut world, 150, y, "stone");
        put(&mut world, 249, y, "stone");
    }
    for x in 151..249 {
        for y in 150..200 {
            put(&mut world, x, y, "water");
        }
    }
    for x in 192..208 {
        for y in 135..150 {
            put(&mut world, x, y, "sand");
        }
    }
    let sand = world.rules().id("sand").unwrap();
    for _ in 0..600 {
        world.step(12_000);
    }
    let sand_count = world
        .cells()
        .iter()
        .filter(|cell| cell.element == sand)
        .count();
    let bottom_sand = world
        .cells()
        .iter()
        .enumerate()
        .filter(|(index, cell)| cell.element == sand && *index / world.width() >= 190)
        .count();
    assert_eq!(sand_count, 16 * 15);
    assert!(
        bottom_sand as f32 / sand_count as f32 >= 0.5,
        "only {bottom_sand}/{sand_count} sand cells reached the basin bottom"
    );
}

#[test]
fn uneven_connected_water_surface_levels_within_ten_simulated_seconds() {
    let mut world = World::new(400, 250, 611, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    for x in 50..350 {
        put(&mut world, x, 200, "stone");
    }
    for y in 10..200 {
        put(&mut world, 50, y, "stone");
        put(&mut world, 349, y, "stone");
    }
    for x in 51..349 {
        let surface = if x < 200 { 150 } else { 168 };
        for y in surface..200 {
            put(&mut world, x, y, "water");
        }
    }
    for x in 190..210 {
        for y in 140..160 {
            put(&mut world, x, y, "sand");
        }
    }
    let water = world.rules().id("water").unwrap();
    for _ in 0..600 {
        world.step(12_000);
    }
    let surfaces: Vec<_> = (51..349)
        .map(|x| {
            (10..200)
                .find(|y| world.cells()[y * world.width() + x].element == water)
                .expect("connected basin column should contain water")
        })
        .collect();
    let spread = surfaces.iter().max().unwrap() - surfaces.iter().min().unwrap();
    assert!(
        spread <= 2,
        "water surface varies by {spread} rows: {surfaces:?}"
    );
}

#[test]
fn density_lab_surface_levels_after_sand_displacement_within_ten_seconds() {
    let rules = Rules::load(DEFAULT_RULES).unwrap();
    let mut world = crate::level::Preset::DensityLab
        .level(&rules, 400, 250)
        .generate(rules)
        .unwrap();
    let water = world.rules().id("water").unwrap();
    let left = world.width() / 10 + 1;
    let right = world.width() - world.width() / 10 - 1;
    for _ in 0..600 {
        world.step(12_000);
    }
    let surfaces: Vec<_> = (left..right)
        .filter_map(|x| {
            (world.height() / 3..world.height() * 4 / 5)
                .find(|y| world.cells()[y * world.width() + x].element == water)
        })
        .collect();
    let spread = surfaces.iter().max().unwrap() - surfaces.iter().min().unwrap();
    assert!(
        spread <= 8,
        "Density lab surface varies by {spread} rows: {surfaces:?}"
    );
}

#[test]
fn density_lab_steam_vent_reaches_above_surface_within_ten_seconds() {
    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    rules.atmosphere.evaporation_rate = 0.0;
    let mut world = World::new(400, 250, 612, rules).unwrap();
    let water = world.rules().id("water").unwrap();
    let steam = world.rules().id("steam").unwrap();
    for x in 150..250 {
        put(&mut world, x, 200, "stone");
    }
    for y in 100..200 {
        put(&mut world, 150, y, "stone");
        put(&mut world, 249, y, "stone");
    }
    for x in 151..249 {
        for y in 120..200 {
            put(&mut world, x, y, "water");
        }
    }
    put(&mut world, 200, 160, "fire");
    for _ in 0..600 {
        world.step(12_000);
    }
    let reached_surface = world
        .cells()
        .iter()
        .enumerate()
        .any(|(index, cell)| cell.element == steam && index / world.width() < 120);
    assert!(
        reached_surface,
        "steam vent produced no steam above the water surface after ten seconds"
    );
    assert!(world.cells().iter().any(|cell| cell.element == water));
}

#[test]
fn gases_bubble_up_through_water_without_losing_cells() {
    for gas in ["steam", "smoke"] {
        let mut rules = Rules::load(DEFAULT_RULES).unwrap();
        let gas_id = rules.id(gas).unwrap();
        rules.elements[gas_id].lifetime = 0;
        let mut world = World::new(1, 8, 21, rules).unwrap();
        for y in 0..6 {
            put(&mut world, 0, y, "water");
        }
        put(&mut world, 0, 6, gas);
        put(&mut world, 0, 7, "stone");
        let counts = element_counts(&world);
        for _ in 0..4_000 {
            world.step(8);
        }
        assert!(has(&world, 0, 0, gas), "{gas} did not reach the surface");
        assert_eq!(element_counts(&world), counts, "{gas} cell count changed");
    }
}

#[test]
fn buoyant_materials_rise_and_dense_lava_sinks() {
    for buoyant in ["ice", "oil", "wood"] {
        let mut world = World::new(1, 8, 31, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
        if buoyant == "ice" {
            world.rules.atmosphere.freezing_point = -20.0;
            world.rules.atmosphere.melting_point = 0.0;
            world.set_ambient(-10, 45, 21);
        }
        for y in 0..6 {
            put(&mut world, 0, y, "water");
        }
        put(&mut world, 0, 6, buoyant);
        put(&mut world, 0, 7, "stone");
        let counts = element_counts(&world);
        for _ in 0..4_000 {
            world.step(8);
        }
        assert_eq!(
            element_counts(&world),
            counts,
            "{buoyant} cell count changed"
        );
        assert!(
            has(&world, 0, 0, buoyant),
            "{buoyant} did not float: {:?}",
            world
                .cells()
                .iter()
                .map(|cell| &world.rules().elements[cell.element].name)
                .collect::<Vec<_>>()
        );
    }

    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    let lava_id = rules.id("lava").unwrap();
    rules.elements[lava_id].lifetime = 0;
    for interaction in &mut rules.interactions {
        if rules.elements[interaction.when].name == "lava"
            && rules.elements[interaction.with].name == "water"
            || rules.elements[interaction.when].name == "water"
                && rules.elements[interaction.with].name == "lava"
        {
            interaction.chance = 0.0;
        }
    }
    let mut lava = World::new(1, 8, 41, rules).unwrap();
    put(&mut lava, 0, 0, "lava");
    for y in 1..7 {
        put(&mut lava, 0, y, "water");
    }
    put(&mut lava, 0, 7, "stone");
    let counts = element_counts(&lava);
    for _ in 0..4_000 {
        lava.step(8);
    }
    assert!(has(&lava, 0, 6, "lava"));
    assert_eq!(element_counts(&lava), counts);
}

#[test]
fn static_solids_never_move_under_density_updates() {
    let mut world = World::new(3, 3, 51, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    put(&mut world, 1, 1, "stone");
    put(&mut world, 2, 2, "soil");
    for _ in 0..100 {
        world.step(9);
    }
    assert!(has(&world, 1, 1, "stone"));
    assert!(has(&world, 2, 2, "soil"));
}

fn element_counts(world: &World) -> Vec<usize> {
    let mut counts = vec![0; world.rules().elements.len()];
    for cell in world.cells() {
        counts[cell.element] += 1;
    }
    counts
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
    put(&mut world, 0, 1, "soil");
    put(&mut world, 0, 0, "tree");
    put(&mut world, 2, 2, "soil");
    put(&mut world, 2, 1, "plant");
    world.step(19);
    assert!(has(&world, 0, 0, "deadwood"));
    assert!(has(&world, 2, 1, "tree"));
    assert!(has(&world, 2, 0, "tree"));
}

#[test]
fn burial_kills_plants_before_maturation_and_trees_into_deadwood() {
    let mut plant = World::new(12, 10, 501, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    put(&mut plant, 5, 5, "plant");
    put(&mut plant, 5, 4, "sand");
    put(&mut plant, 4, 5, "stone");
    put(&mut plant, 6, 5, "stone");
    for _ in 0..119 {
        plant.step(usize::MAX);
    }
    assert!(has(&plant, 5, 5, "plant"));
    assert!(
        !plant
            .cells()
            .iter()
            .any(|cell| { cell.element == plant.rules().id("tree").unwrap() })
    );
    plant.step(usize::MAX);
    assert!(
        ["deadwood", "leaf_litter", "humus"].iter().any(|name| {
            plant
                .cells()
                .iter()
                .any(|cell| cell.element == plant.rules().id(name).unwrap())
        }),
        "a buried plant should leave existing dead organic matter"
    );
    assert!(
        !plant
            .cells()
            .iter()
            .any(|cell| { cell.element == plant.rules().id("tree").unwrap() })
    );

    let mut tree = World::new(12, 10, 502, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    put(&mut tree, 5, 6, "soil");
    put(&mut tree, 5, 5, "tree");
    put(&mut tree, 5, 4, "sand");
    put(&mut tree, 4, 5, "stone");
    put(&mut tree, 6, 5, "stone");
    for _ in 0..48 {
        tree.step(usize::MAX);
    }
    assert!(
        tree.cells()
            .iter()
            .any(|cell| { cell.element == tree.rules().id("deadwood").unwrap() })
    );
}

#[test]
fn soil_and_stone_are_configured_burial_covers() {
    for (seed, cover) in [(510, "soil"), (511, "stone")] {
        let mut world = World::new(12, 10, seed, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
        put(&mut world, 5, 6, "soil");
        put(&mut world, 5, 5, "tree");
        put(&mut world, 5, 4, cover);
        for _ in 0..48 {
            world.step(usize::MAX);
        }
        assert!(
            world
                .cells()
                .iter()
                .any(|cell| { cell.element == world.rules().id("deadwood").unwrap() }),
            "{cover} should trigger the configured overhead burial rule"
        );
    }
}

#[test]
fn lateral_living_and_exposed_vegetation_do_not_count_as_burial() {
    let mut side = World::new(12, 10, 520, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    put(&mut side, 5, 6, "soil");
    put(&mut side, 5, 5, "tree");
    put(&mut side, 4, 5, "stone");
    for _ in 0..48 {
        side.step(usize::MAX);
    }
    assert!(has(&side, 5, 5, "tree"));
    assert!(
        !side
            .cells()
            .iter()
            .any(|cell| { cell.element == side.rules().id("deadwood").unwrap() })
    );

    for (seed, fluid_or_gas) in [(523, "water"), (524, "smoke")] {
        let mut world = World::new(12, 10, seed, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
        put(&mut world, 5, 6, "soil");
        put(&mut world, 5, 5, "tree");
        put(&mut world, 5, 4, fluid_or_gas);
        for _ in 0..48 {
            world.step(usize::MAX);
        }
        assert!(
            has(&world, 5, 5, "tree"),
            "{fluid_or_gas} is not solid cover"
        );
    }

    let mut canopy = World::new(12, 10, 521, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    put(&mut canopy, 5, 6, "soil");
    put(&mut canopy, 5, 5, "tree");
    put(&mut canopy, 5, 4, "tree");
    for _ in 0..48 {
        canopy.step(usize::MAX);
    }
    assert!(
        canopy
            .cells()
            .iter()
            .filter(|cell| { cell.element == canopy.rules().id("tree").unwrap() })
            .count()
            >= 2,
        "a living canopy cell is not burial cover"
    );

    let mut exposed = World::new(12, 10, 522, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    put(&mut exposed, 5, 6, "soil");
    put(&mut exposed, 5, 5, "plant");
    for _ in 0..120 {
        exposed.step(usize::MAX);
    }
    assert!(
        has(&exposed, 5, 5, "tree"),
        "uncovered plant maturation is unchanged"
    );
}

#[test]
fn burial_timer_resets_when_cover_is_removed() {
    let mut world = World::new(12, 10, 529, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    put(&mut world, 5, 6, "soil");
    put(&mut world, 5, 5, "tree");
    let tree = 5 + 5 * world.width();
    world.cells[tree].burial_age = 47;
    world.step(usize::MAX);
    assert_eq!(world.cells()[tree].burial_age, 0);

    put(&mut world, 5, 4, "stone");
    world.step(usize::MAX);
    assert_eq!(
        world.cells()[tree].element,
        world.rules().id("tree").unwrap()
    );
}

#[test]
fn burial_products_remain_connected_to_existing_decomposition() {
    let mut world = guaranteed_world(
        530,
        &[("wet_soil", "deadwood"), ("wet_soil", "leaf_litter")],
    );
    put(&mut world, 1, 1, "wet_soil");
    put(&mut world, 2, 1, "deadwood");
    put(&mut world, 3, 1, "leaf_litter");
    world.step(usize::MAX);
    assert!(has(&world, 1, 1, "fertile_soil"));
    assert!(world.cells().iter().any(|cell| {
        cell.element == world.rules().id("fungus").unwrap()
            || cell.element == world.rules().id("beetle").unwrap()
    }));
}

#[test]
fn burial_rules_round_trip_with_the_data_driven_ruleset() {
    let rules = Rules::load(DEFAULT_RULES).unwrap();
    let exported = rules.to_toml().unwrap();
    let loaded = Rules::load(&exported).unwrap();
    assert_eq!(loaded.burial_rules.len(), 2);
    assert_eq!(loaded.burial_rules[0].updates, 120);
}

#[test]
fn burning_trees_release_a_seed_into_the_canopy_gap() {
    let mut world = World::new(12, 12, 42, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    put(&mut world, 5, 10, "soil");
    put(&mut world, 5, 9, "tree");
    let tree = world.rules().id("tree").unwrap();
    let fire = world.rules().id("fire").unwrap();
    let ash = world.rules().id("ash").unwrap();
    let trunk = 9 * world.width() + 5;

    world.set_element(trunk, fire);
    assert!(has(&world, 5, 9, "fire"));
    assert_eq!(world.cells()[trunk].element, fire);
    world.set_element(trunk, ash);
    assert_ne!(world.cells()[trunk].element, tree);
    assert_ne!(world.cells()[trunk].burn_progress, 0);
    world.step(usize::MAX);

    let seed = world.rules().id("seed").unwrap();
    let plant = world.rules().id("plant").unwrap();
    assert!(
        (7..=9).any(|y| {
            let element = world.cells()[y * world.width() + 5].element;
            element == seed || element == plant
        }),
        "an extinguished tree should leave a viable seed above its burned trunk"
    );
}

#[test]
fn burned_ash_finds_seed_space_above_a_crowded_canopy() {
    let mut world = World::new(12, 12, 43, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    for (x, y) in [(5, 7), (4, 8), (6, 8), (5, 9)] {
        put(&mut world, x, y, "stone");
    }
    put(&mut world, 5, 8, "ash");
    let ash = 8 * world.width() + 5;
    world.cells[ash].burn_progress = 2;

    world.step(usize::MAX);

    let seed = world.rules().id("seed").unwrap();
    assert!(world.cells().iter().any(|cell| cell.element == seed));
    assert_eq!(world.cells()[ash].burn_progress, 0);
}

#[test]
fn unsupported_tree_cells_wither_within_64_updates() {
    let mut world = World::new(12, 12, 43, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    put(&mut world, 5, 3, "tree");
    for _ in 0..64 {
        world.step(usize::MAX);
    }
    assert!(
        !world
            .cells()
            .iter()
            .any(|cell| cell.element == world.rules().id("tree").unwrap())
    );
    assert!(
        world
            .cells()
            .iter()
            .any(|cell| cell.element == world.rules().id("deadwood").unwrap())
    );
}

#[test]
fn unsupported_canopy_falls_after_its_living_trunk_dies() {
    let mut world = World::new(12, 12, 44, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    for x in 3..=7 {
        put(&mut world, x, 9, "soil");
    }
    let tree = world.rules().id("tree").unwrap();
    for y in 5..=8 {
        put(&mut world, 5, y, "tree");
    }
    for x in 4..=6 {
        put(&mut world, x, 4, "tree");
    }
    let trunk = 6 * world.width() + 5;
    world.cells[trunk].age = world.rules.elements[tree].lifetime - 1;

    // Support checks are phase-staggered across 64 updates to keep frame work bounded.
    for _ in 0..64 {
        world.step(usize::MAX);
    }
    assert!(
        (0..=6)
            .all(|y| (0..world.width())
                .all(|x| world.cells()[y * world.width() + x].element != tree)),
        "no unsupported crown or upper trunk cells should remain above the surviving grounded trunk"
    );
    assert!(world.cells().iter().any(|cell| cell.element == tree));
    assert_ne!(
        world.cells()[4 * world.width() + 5].element,
        tree,
        "the unsupported crown should leave its original position"
    );
    assert!(
        world
            .cells()
            .iter()
            .filter(|cell| cell.element == world.rules().id("deadwood").unwrap())
            .count()
            > 1,
        "fallen canopy should become deadwood"
    );
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
        world.rules.ecology.critter_move_chance = 0.0;
        put(&mut world, 0, 0, decomposer);
        put(&mut world, 1, 0, matter);
        world.step(1);
        assert!(has(&world, 0, 0, decomposer));
        assert!(has(&world, 1, 0, "fertile_soil"));
    }
}

#[test]
fn critters_walk_supported_surfaces_but_not_open_air() {
    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    rules.ecology.critter_move_chance = 1.0;
    rules.ecology.flee_chance = 0.0;
    let mut world = World::new(8, 8, 91, rules).unwrap();
    for x in 1..=4 {
        put(&mut world, x, 5, "stone");
    }
    put(&mut world, 2, 4, "beetle");
    world.scheduler.enqueue_focus(2 + 4 * world.width());
    world.step(1);
    assert_eq!(
        world
            .cells()
            .iter()
            .filter(|c| world.rules().elements[c.element].name == "beetle")
            .count(),
        1
    );
    assert_ne!(
        world.cells()[2 + 4 * world.width()].element,
        world.rules().id("beetle").unwrap()
    );
    for _ in 0..10 {
        let position = world
            .cells()
            .iter()
            .position(|cell| cell.element == world.rules().id("beetle").unwrap())
            .unwrap();
        world.scheduler.enqueue_focus(position);
        world.step(1);
    }
    assert_eq!(
        world
            .cells()
            .iter()
            .filter(|cell| cell.element == world.rules().id("beetle").unwrap())
            .count(),
        1
    );
    assert_eq!(
        world
            .cells()
            .iter()
            .position(|cell| cell.element == world.rules().id("beetle").unwrap())
            .unwrap()
            / world.width(),
        4
    );

    let mut open = World::new(8, 8, 91, {
        let mut rules = Rules::load(DEFAULT_RULES).unwrap();
        rules.ecology.critter_move_chance = 1.0;
        rules
    })
    .unwrap();
    put(&mut open, 3, 2, "termite");
    let index = 3 + 2 * open.width();
    open.scheduler.enqueue_focus(index);
    open.step(1);
    assert_eq!(
        open.cells()[index + open.width()].element,
        open.rules().id("termite").unwrap()
    );
}

#[test]
fn critters_climb_tree_and_deadwood_surfaces() {
    for (offset, surface) in ["tree", "deadwood"].into_iter().enumerate() {
        let mut rules = Rules::load(DEFAULT_RULES).unwrap();
        rules.ecology.critter_move_chance = 1.0;
        rules.ecology.flee_chance = 0.0;
        let mut world = World::new(8, 8, 200 + offset as u64, rules).unwrap();
        put(&mut world, 3, 4, "beetle");
        put(&mut world, 3, 5, "stone");
        put(&mut world, 2, 4, "stone");
        put(&mut world, 4, 4, "stone");
        put(&mut world, 4, 3, surface);
        let start = 3 + 4 * world.width();
        world.scheduler.enqueue_focus(start);
        world.step(1);
        assert_eq!(
            world.cells()[3 + 3 * world.width()].element,
            world.rules().id("beetle").unwrap(),
            "beetle should climb beside {surface}"
        );
    }
}

#[test]
fn fungus_colonizes_damp_habitat_and_heat_kills_it() {
    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    rules.ecology.fungus_spread_chance = 1.0;
    rules.ecology.fungus_starvation_chance = 0.0;
    let mut world = World::new(8, 8, 92, rules).unwrap();
    world.set_ambient(20, 90, 21);
    put(&mut world, 1, 1, "fungus");
    put(&mut world, 3, 1, "deadwood");
    let deadwood = 3 + world.width();
    world.cells[deadwood].moisture = 80;
    put(&mut world, 2, 2, "stone");
    let index = 1 + world.width();
    world.scheduler.enqueue_focus(index);
    world.step(1);
    assert_eq!(
        world.cells()[2 + world.width()].element,
        world.rules().id("fungus").unwrap()
    );

    let mut hot = World::new(8, 8, 93, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    hot.set_ambient(60, 90, 21);
    put(&mut hot, 2, 2, "fungus");
    let index = 2 + 2 * hot.width();
    hot.scheduler.enqueue_focus(index);
    hot.step(1);
    assert_eq!(hot.cells()[index].element, hot.rules().id("soil").unwrap());

    let mut starving_rules = Rules::load(DEFAULT_RULES).unwrap();
    starving_rules.ecology.fungus_starvation_chance = 1.0;
    let mut starving = World::new(8, 8, 97, starving_rules).unwrap();
    put(&mut starving, 2, 2, "fungus");
    let index = 2 + 2 * starving.width();
    starving.scheduler.enqueue_focus(index);
    starving.step(1);
    assert_eq!(
        starving.cells()[index].element,
        starving.rules().id("soil").unwrap()
    );
}

#[test]
fn critters_flee_fire_and_turn_meals_into_fertile_soil() {
    let mut fleeing = World::new(8, 8, 94, {
        let mut rules = Rules::load(DEFAULT_RULES).unwrap();
        rules.ecology.critter_move_chance = 1.0;
        rules.ecology.flee_chance = 1.0;
        rules
    })
    .unwrap();
    for x in 1..=5 {
        put(&mut fleeing, x, 5, "stone");
    }
    put(&mut fleeing, 3, 3, "fire");
    put(&mut fleeing, 3, 4, "beetle");
    let beetle = fleeing.rules().id("beetle").unwrap();
    let stone = fleeing.rules().id("stone").unwrap();
    let fire = 3 + 3 * fleeing.width();
    let start = 3 + 4 * fleeing.width();
    fleeing.scheduler.enqueue_focus(start);
    fleeing.step(1);
    let positions: Vec<_> = fleeing
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.element == beetle).then_some(index))
        .collect();
    assert_eq!(positions.len(), 1);
    assert_ne!(positions[0], start);
    let beetle_x = positions[0] % fleeing.width();
    let beetle_y = positions[0] / fleeing.width();
    let fire_x = fire % fleeing.width();
    let fire_y = fire / fleeing.width();
    assert!(beetle_x.abs_diff(fire_x) + beetle_y.abs_diff(fire_y) > 1);
    assert_ne!(positions[0] / fleeing.width(), 5);
    assert_eq!(fleeing.cells()[positions[0]].element, beetle);
    assert_ne!(fleeing.cells()[positions[0]].element, stone);

    let mut foraging = guaranteed_world(95, &[("beetle", "leaf_litter")]);
    foraging.rules.ecology.critter_move_chance = 0.0;
    put(&mut foraging, 2, 4, "beetle");
    put(&mut foraging, 3, 4, "leaf_litter");
    foraging.scheduler.enqueue_focus(4 * foraging.width() + 2);
    foraging.step(1);
    assert!(has(&foraging, 2, 4, "beetle"));
    assert!(has(&foraging, 3, 4, "fertile_soil"));
}

#[test]
fn fungus_spreads_in_humid_air_but_stays_out_of_dry_air_and_is_bounded() {
    let make_world = |humidity| {
        let mut rules = Rules::load(DEFAULT_RULES).unwrap();
        rules.ecology.fungus_spread_chance = 1.0;
        rules.ecology.fungus_starvation_chance = 0.0;
        for interaction in &mut rules.interactions {
            if rules.elements[interaction.when].name == "fungus" {
                interaction.enabled = false;
            }
        }
        let mut world = World::new(12, 8, 96, rules).unwrap();
        world.set_ambient(20, humidity, 21);
        put(&mut world, 1, 4, "fungus");
        for x in 1..=10 {
            put(&mut world, x, 5, "stone");
        }
        for x in 2..=9 {
            put(&mut world, x, 3, "deadwood");
            let index = 3 * world.width() + x;
            world.cells[index].moisture = 80;
        }
        world.scheduler.enqueue_focus(4 * world.width() + 1);
        world
    };
    let mut humid = make_world(100);
    let mut dry = make_world(0);
    let budget = humid.cells().len() * 8;
    for _ in 0..budget {
        humid.step(1);
        dry.step(1);
    }
    let fungus = humid.rules().id("fungus").unwrap();
    let humid_count = humid
        .cells()
        .iter()
        .filter(|cell| cell.element == fungus)
        .count();
    let dry_count = dry
        .cells()
        .iter()
        .filter(|cell| cell.element == fungus)
        .count();
    assert!(humid_count > dry_count);
    assert!(
        humid_count <= 12,
        "fungus growth stays bounded: {humid_count}"
    );
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
fn seeds_do_not_germinate_on_ash_when_hot_or_cooled() {
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
    assert!(has(&world, 0, 0, "seed"));
    assert!(has(&world, 0, 1, "ash"));
}

#[test]
fn seeds_do_not_germinate_during_sustained_contact_with_an_ash_only_bed() {
    let mut world = guaranteed_world(43, &[("seed", "ash"), ("ash", "seed")]);
    let width = world.width();
    let height = world.height();
    let floor_y = height - 1;

    // A stone floor naturally keeps the falling seed and ash side by side.
    for x in 0..width {
        put(&mut world, x, floor_y, "stone");
    }
    let seed_x = width / 2;
    let seed_y = floor_y - 1;
    put(&mut world, seed_x, seed_y, "seed");
    put(&mut world, seed_x + 1, seed_y, "ash");

    let assert_ash_only_contact = |world: &World, checkpoint: usize| {
        assert!(
            has(world, seed_x, seed_y, "seed"),
            "seed moved at checkpoint {checkpoint}"
        );
        assert!(
            has(world, seed_x + 1, seed_y, "ash"),
            "ash moved at checkpoint {checkpoint}"
        );
        assert!(
            world.cells().iter().all(|cell| {
                ["air", "stone", "seed", "ash"]
                    .iter()
                    .any(|name| world.rules.elements[cell.element].name == *name)
            }),
            "non-ash substrate appeared at checkpoint {checkpoint}"
        );
    };

    assert_ash_only_contact(&world, 0);
    for sweep in 1..=128 {
        assert_eq!(world.step(width * height), width * height);
        if [1, 8, 32, 64, 128].contains(&sweep) {
            assert_ash_only_contact(&world, sweep);
        }
    }
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
    assert!(
        phases
            .cells()
            .iter()
            .any(|cell| { cell.element == phases.rules().id("ice").unwrap() })
    );
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
fn humid_air_produces_cloud_and_rain_but_dry_air_stays_clear_and_water_is_bounded() {
    let mut humid = World::new(64, 48, 630, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    humid.set_ambient(20, 100, 21);
    let ids =
        ["ice", "cloud", "rain", "water", "steam"].map(|name| humid.rules().id(name).unwrap());
    let count = |world: &World, id| {
        world
            .cells()
            .iter()
            .filter(|cell| cell.element == id)
            .count()
    };
    let mut saw_cloud = false;
    let mut saw_rain = false;
    let mut weather_at_30_seconds = false;
    for sweep in 1..=1800 {
        humid.step(usize::MAX);
        let clouds = count(&humid, ids[1]);
        let rain = count(&humid, ids[2]);
        saw_cloud |= clouds > 0;
        saw_rain |= rain > 0;
        if sweep == 1800 {
            weather_at_30_seconds = clouds > 0 || rain > 0;
        }
        assert!(
            ids.iter().map(|id| count(&humid, *id)).sum::<usize>() <= humid.atmosphere.len() * 3,
            "water-bearing cells should stay bounded by the initial vapor reservoir"
        );
        for x in [0, humid.width() - 1] {
            let edge_water = (0..humid.height())
                .filter(|y| humid.cells()[y * humid.width() + x].element == ids[3])
                .count();
            assert!(
                edge_water <= crate::world::MAX_EDGE_WATER_COLUMN,
                "water column at world edge x={x} grew to {edge_water} cells"
            );
        }
    }
    assert_eq!(
        count(&humid, ids[0]),
        0,
        "20 C humid air must not create ice"
    );
    assert!(saw_cloud, "humid air should form visible clouds");
    assert!(saw_rain, "cloud droplets should become rain");
    assert!(
        weather_at_30_seconds,
        "humid air should still show clouds or rain at 30 simulated seconds"
    );

    let mut dry = World::new(64, 48, 630, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    dry.set_ambient(20, 0, 21);
    for _ in 0..180 {
        dry.step(usize::MAX);
    }
    assert_eq!(count(&dry, ids[1]), 0, "dry air should stay cloud-free");
    assert_eq!(count(&dry, ids[2]), 0, "dry air should not produce rain");
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
fn falling_rain_damps_fire_and_turns_into_water() {
    let mut rain = guaranteed_world(66, &[("fire", "rain"), ("rain", "fire")]);
    put(&mut rain, 4, 5, "rain");
    put(&mut rain, 5, 5, "fire");
    rain.step(usize::MAX);
    assert!(
        !rain
            .cells()
            .iter()
            .any(|cell| cell.element == rain.rules().id("fire").unwrap()),
        "rain should extinguish an adjacent fire"
    );
    assert!(
        rain.cells()
            .iter()
            .any(|cell| cell.element == rain.rules().id("water").unwrap())
            || rain
                .cells()
                .iter()
                .any(|cell| cell.element == rain.rules().id("steam").unwrap()),
        "rain should remain water or become steam when quenching fire"
    );
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
            fit_window: false,
            seed: 731,
            generator: Generator::Hills,
            mode: crate::level::LevelMode::Fixed,
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
            boat: false,
            boat_sand: 0,
            boat_burning: false,
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
        dry_following > 0.40,
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
        dry_following > moist_following + 0.05,
        "dry {:.1}%, moist {:.1}%",
        dry_following * 100.0,
        moist_following * 100.0
    );
    assert_eq!(airless, 0.0, "zero oxygen must not burn trees");
}

#[test]
fn unburned_forest_keeps_at_least_75_percent_canopy_after_300_seconds() {
    let rules = Rules::load(DEFAULT_RULES).unwrap();
    let mut level = crate::level::Preset::DryForest.level(&rules, 32, 24);
    level.seed = 3;
    level.spark = false;
    level.wind = Default::default();
    let mut world = level.generate(rules).unwrap();
    let count = |world: &World, name: &str| {
        let id = world.rules().id(name).unwrap();
        world
            .cells()
            .iter()
            .filter(|cell| cell.element == id)
            .count()
    };
    let initial_canopy = count(&world, "tree");
    let tree = world.rules().id("tree").unwrap();
    let age_phases: std::collections::BTreeSet<_> = world
        .cells()
        .iter()
        .filter(|cell| cell.element == tree)
        .map(|cell| cell.age)
        .collect();
    assert!(age_phases.len() >= 4, "tree cohorts need varied ages");
    assert!(initial_canopy > 0);

    // 12,000 work/frame on 192x120 at 60 fps visits each cell 31.25 times/s.
    let full_sweeps = 300 * 12_000 * 60 / (192 * 120);
    for _ in 0..full_sweeps {
        world.step(usize::MAX);
    }

    let green_canopy = count(&world, "tree");
    assert!(
        green_canopy * 100 >= initial_canopy * 75,
        "unburned canopy must retain at least 75% after 300 simulated seconds: {green_canopy}/{initial_canopy}"
    );
}

#[test]
fn mature_tree_seeds_a_replacement_on_its_deadwood_stump() {
    let mut world = world(92_814);
    let tree = world.rules().id("tree").unwrap();
    for x in 7..=13 {
        put(&mut world, x, 23, "stone");
    }
    put(&mut world, 10, 18, "tree");
    let old_tree = 18 * world.width() + 10;
    world.cells[old_tree].age = world.rules.elements[tree].lifetime - 1;

    for _ in 0..64 {
        world.step(usize::MAX);
    }

    let plant = world.rules().id("plant").unwrap();
    assert!(
        world.cells().iter().any(|cell| cell.element == plant),
        "an old tree should emit a seed that germinates on its deadwood stump"
    );
    assert!(
        world
            .cells()
            .iter()
            .any(|cell| cell.element == world.rules().id("humus").unwrap()),
        "the seed should turn its deadwood perch into humus"
    );
}

#[test]
fn dry_forest_fire_front_waits_for_soil_enrichment_before_regrowth() {
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
        recovered_trees < trees_after_fire + 8,
        "dry ash without wet or fertile soil should not regrow a trunk-and-crown canopy"
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
    world.paint(4, 5, world.rules().id("soil").unwrap());
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
fn fertile_soil_patch_regrows_eight_canopy_cells_within_120_seconds() {
    let mut world = World::new(192, 120, 42, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
    let soil = world.rules().id("soil").unwrap();
    let fertile_soil = world.rules().id("fertile_soil").unwrap();
    let stone = world.rules().id("stone").unwrap();
    let seed = world.rules().id("seed").unwrap();
    for x in 85..100 {
        world.paint(x, 100, soil);
        world.paint(x, 101, stone);
    }
    world.paint(90, 99, fertile_soil);
    world.paint(90, 98, seed);
    let plant = world.rules().id("plant").unwrap();
    let tree = world.rules().id("tree").unwrap();
    let deadline_frames = 120 * 60;
    let mut saw_sapling = false;
    let mut canopy_frame = None;
    for frame in 1..=deadline_frames {
        world.step(12_000);
        saw_sapling |= world.cells().iter().any(|cell| cell.element == plant);
        let canopy_cells = world
            .cells()
            .iter()
            .filter(|cell| cell.element == tree)
            .count();
        if canopy_cells >= 8 {
            canopy_frame = Some(frame);
            break;
        }
    }
    assert!(
        saw_sapling,
        "fertile soil and a seed should produce a sapling"
    );
    assert!(
        canopy_frame.is_some_and(|frame| frame <= deadline_frames),
        "burned patch should regrow eight canopy cells within 120 seconds"
    );
}

#[test]
fn boat_density_adapter_uses_the_per_element_density() {
    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    let wood = rules.id("wood").unwrap();
    let water = rules.id("water").unwrap();
    assert!(rules.elements[wood].density < rules.elements[water].density);
    rules.elements[wood].density = 45;
    let world = World::new(32, 24, 1, rules).unwrap();
    assert_eq!(world.body_material_density(wood), 0.5);
    assert_eq!(world.body_material_density(water), 1.0);
}

#[test]
fn airborne_boat_falls_and_then_floats_on_water_deterministically() {
    let mut first = world(41);
    let mut second = world(41);
    for current in [&mut first, &mut second] {
        for x in 1..current.width() - 1 {
            put(current, x, 15, "water");
            put(current, x, 16, "stone");
        }
        assert!(current.place_boat(16, 4));
    }
    let wood = first.rules().id("wood").unwrap();
    let initial_top = first
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.element == wood).then_some(index / first.width()))
        .min()
        .unwrap();
    assert_eq!(initial_top, 2);

    for _ in 0..40 {
        first.step(768);
        second.step(768);
    }
    let boat_rows = |world: &World| {
        world
            .cells()
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| (cell.element == wood).then_some(index / world.width()))
            .collect::<Vec<_>>()
    };
    let first_rows = boat_rows(&first);
    let second_rows = boat_rows(&second);
    assert_eq!(
        first_rows, second_rows,
        "identical seeds keep boat motion deterministic"
    );
    assert!(
        *first_rows.iter().min().unwrap() > initial_top,
        "the empty boat falls from its spawn point"
    );
    assert_eq!(
        *first_rows.iter().min().unwrap(),
        12,
        "the hull comes to rest floating above the lake bed"
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
fn boat_buoyancy_uses_wood_density_from_the_ruleset() {
    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    let wood = rules.id("wood").unwrap();
    rules.elements[wood].density = 220;
    let mut world = World::new(32, 24, 8, rules).unwrap();
    for x in 1..world.width() - 1 {
        put(&mut world, x, 15, "water");
        put(&mut world, x, 16, "water");
        put(&mut world, x, 17, "stone");
    }
    assert!(world.place_boat(16, 14));
    for _ in 0..8 {
        world.step(768);
    }
    let top = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.body_id != 0).then_some(index / world.width()))
        .min()
        .unwrap();
    assert!(top > 12, "denser wood should make the body sink, top={top}");
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
    assert_eq!(
        translation, 1,
        "east wind nudges one cell over four body updates"
    );
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
fn ash_slumps_off_static_supports_and_settles_below_them() {
    let mut world = world(905);
    let ash = world.rules().id("ash").unwrap();
    let stone = world.rules().id("stone").unwrap();
    let tree = world.rules().id("tree").unwrap();
    for x in 0..world.width() {
        put(&mut world, x, 20, "stone");
    }
    for x in 10..=14 {
        put(&mut world, x, 10, "tree");
        put(&mut world, x, 11, "soil");
    }
    for y in 6..=9 {
        for x in 10..=14 {
            put(&mut world, x, y, "ash");
        }
    }
    let initial_ash = world
        .cells()
        .iter()
        .filter(|cell| cell.element == ash)
        .count();
    assert_eq!(initial_ash, 20);

    for _ in 0..80 {
        world.step(usize::MAX);
    }

    let ash_cells: Vec<_> = world
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| (cell.element == ash).then_some(index))
        .collect();
    assert_eq!(ash_cells.len(), initial_ash, "slumping preserves ash");
    assert!(
        ash_cells.iter().any(|index| {
            let x = index % world.width();
            let y = index / world.width();
            y > 10 && !(10..=14).contains(&x)
        }),
        "loose ash should slide off the static tree ledge and settle below it"
    );
    assert!(world.cells().iter().any(|cell| cell.element == stone));
    assert!(world.cells().iter().any(|cell| cell.element == tree));
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
