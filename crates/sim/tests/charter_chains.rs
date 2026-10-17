use tinderbox_sim::{DEFAULT_RULES, Rules, WindDirection, World};

fn world(width: usize, height: usize, seed: u64) -> World {
    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    rules.daylight.enabled = false;
    World::new(width, height, seed, rules).unwrap()
}

fn id(world: &World, name: &str) -> usize {
    world
        .rules()
        .id(name)
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn count(world: &World, name: &str) -> usize {
    let element = id(world, name);
    world
        .cells()
        .iter()
        .filter(|cell| cell.element == element)
        .count()
}

fn min_row(world: &World, name: &str) -> Option<usize> {
    let element = id(world, name);
    world
        .cells()
        .iter()
        .enumerate()
        .find_map(|(index, cell)| (cell.element == element).then_some(index / world.width()))
}

fn average_moisture(world: &World, name: &str) -> f32 {
    let element = id(world, name);
    let cells: Vec<_> = world
        .cells()
        .iter()
        .filter(|cell| cell.element == element)
        .collect();
    if cells.is_empty() {
        0.0
    } else {
        cells
            .iter()
            .map(|cell| f32::from(cell.moisture))
            .sum::<f32>()
            / cells.len() as f32
    }
}

fn tick(world: &mut World) {
    world.step(world.width() * world.height());
}

fn sample(world: &World, t: usize) {
    println!(
        "ecology,{t},{},{},{},{},{},{},{},{},{},{},{},{},{:.1}",
        count(world, "tree"),
        count(world, "fire"),
        count(world, "ash"),
        count(world, "deadwood"),
        count(world, "leaf_litter"),
        count(world, "humus"),
        count(world, "fungus"),
        count(world, "beetle"),
        count(world, "termite"),
        count(world, "fertile_soil"),
        count(world, "plant"),
        count(world, "seed"),
        average_moisture(world, "deadwood"),
    );
}

#[test]
fn direct_water_contact_extinguishes_fire_and_keeps_it_out_for_multiple_sweeps() {
    let mut rules = Rules::load(DEFAULT_RULES).unwrap();
    let fire = rules.id("fire").unwrap();
    let water = rules.id("water").unwrap();
    let dousing = rules
        .interactions
        .iter_mut()
        .find(|rule| rule.when == fire && rule.with == water)
        .expect("default rules should include direct fire/water dousing");
    dousing.chance = 1.0;

    let mut world = World::new(8, 8, 0xD0_051E, rules).unwrap();
    world.paint(3, 6, fire);
    world.paint(4, 6, water);
    assert_eq!(count(&world, "fire"), 1);
    assert_eq!(count(&world, "water"), 1);
    assert_eq!(count(&world, "steam"), 0);

    tick(&mut world);
    assert_eq!(
        count(&world, "fire"),
        0,
        "water must extinguish the only fire"
    );
    assert_eq!(
        count(&world, "water"),
        0,
        "contacted water must be consumed"
    );
    assert!(
        count(&world, "steam") > 0,
        "dousing must turn water into steam"
    );

    for sweep in 1..=5 {
        tick(&mut world);
        assert_eq!(
            count(&world, "fire"),
            0,
            "fire must not reappear during controlled sweep {sweep}"
        );
        assert!(
            count(&world, "water") + count(&world, "steam") > 0,
            "the dousing product must remain water or steam during sweep {sweep}"
        );
    }
}

#[test]
fn charter_chains_are_measurable_and_regression_covered() {
    // A soaked front must stop while its seeded, unwatered control continues spreading.
    let mut wet = world(48, 32, 7);
    let mut dry = world(48, 32, 7);
    for w in [&mut wet, &mut dry] {
        for x in 10..38 {
            w.paint(x, 24, id(w, "soil"));
            w.paint(x, 23, id(w, "tree"));
        }
        w.paint(24, 23, id(w, "fire"));
    }
    wet.paint(24, 22, id(&wet, "water"));
    tick(&mut wet);
    tick(&mut dry);
    println!(
        "dousing,1,wet_fire={},steam={},wet_trees={},dry_fire={},dry_trees={}",
        count(&wet, "fire"),
        count(&wet, "steam"),
        count(&wet, "tree"),
        count(&dry, "fire"),
        count(&dry, "tree")
    );
    assert_eq!(count(&wet, "fire"), 0);
    assert!(count(&wet, "steam") >= 1);
    assert!(count(&dry, "fire") >= 5);
    assert!(count(&wet, "tree") > count(&dry, "tree"));

    // Seed a small burned forest over a damp organic floor and measure its recovery.
    let mut ecology = world(48, 32, 81);
    ecology.set_ambient(20, 80, 21);
    for x in 6..42 {
        for y in 16..24 {
            ecology.paint(x, y, id(&ecology, "tree"));
        }
        ecology.paint(x, 24, id(&ecology, "deadwood"));
        ecology.paint(x, 25, id(&ecology, "wet_soil"));
        for y in 26..32 {
            ecology.paint(x, y, id(&ecology, "soil"));
        }
    }
    for x in 6..42 {
        ecology.paint(x, 16, id(&ecology, "fire"));
    }
    println!(
        "ecology,tick,tree,fire,ash,deadwood,leaf_litter,humus,fungus,beetle,termite,fertile_soil,plant,seed,mean_deadwood_moisture"
    );
    sample(&ecology, 0);
    let mut elapsed = 0;
    for target in [1, 12, 30, 60, 120, 180] {
        for _ in elapsed..target {
            tick(&mut ecology);
        }
        elapsed = target;
        sample(&ecology, target);
        match target {
            1 => assert!(count(&ecology, "ash") > 0 && count(&ecology, "fire") > 0),
            30 => assert!(count(&ecology, "beetle") > 0),
            180 => {
                // Organic matter can be absent at this coarse stochastic checkpoint;
                // alternate fire decay has a dedicated forced-outcome unit test.
                assert!(count(&ecology, "termite") > 0);
                assert!(count(&ecology, "humus") > 0);
                assert!(count(&ecology, "fertile_soil") > 0);
                assert!(count(&ecology, "plant") > 0 && count(&ecology, "tree") > 0);
                assert!(count(&ecology, "beetle") < 100 && count(&ecology, "termite") < 100);
                assert!(count(&ecology, "plant") < 200 && count(&ecology, "tree") < 200);
            }
            _ => {}
        }
    }

    // Strong east wind advects light materials and accelerates spread downwind.
    let mut drift = world(48, 24, 3);
    drift.set_wind(WindDirection::East, 4);
    drift.paint(20, 10, id(&drift, "ash"));
    drift.paint(20, 11, id(&drift, "smoke"));
    for _ in 0..20 {
        tick(&mut drift);
    }
    let east_ash = drift
        .cells()
        .iter()
        .enumerate()
        .any(|(i, c)| c.element == id(&drift, "ash") && i % drift.width() > 20);
    let east_smoke = drift
        .cells()
        .iter()
        .enumerate()
        .any(|(i, c)| c.element == id(&drift, "smoke") && i % drift.width() > 20);
    println!("wind_drift,20,ash_east={east_ash},smoke_east={east_smoke}");
    assert!(east_ash && east_smoke);
    let mut spread = Vec::new();
    for (direction, label) in [
        (WindDirection::East, "east"),
        (WindDirection::West, "west"),
        (WindDirection::Calm, "calm"),
    ] {
        let mut w = world(48, 24, 31);
        w.set_wind(direction, 4);
        for x in 15..34 {
            w.paint(x, 12, id(&w, "tree"));
        }
        w.paint(15, 12, id(&w, "fire"));
        for _ in 0..20 {
            tick(&mut w);
        }
        let fires = count(&w, "fire");
        println!("fire_spread,20,{label},{fires}");
        spread.push(fires);
    }
    assert!(spread[0] >= spread[1] * 2);
    assert!(spread[0] >= spread[2] * 2);

    // Lava/water, fire/ice, and an ignited oil slick floating on water.
    let mut lava = world(24, 24, 1);
    lava.paint(10, 10, id(&lava, "lava"));
    lava.paint(11, 10, id(&lava, "water"));
    let mut ice = world(24, 24, 1);
    ice.paint(10, 10, id(&ice, "fire"));
    ice.paint(11, 10, id(&ice, "ice"));
    let mut oil = world(24, 24, 1);
    let mut oil_float = world(24, 24, 1);
    for w in [&mut oil, &mut oil_float] {
        for x in 5..19 {
            w.paint(x, 17, id(w, "water"));
        }
        for x in 7..17 {
            w.paint(x, 16, id(w, "oil"));
        }
    }
    oil.paint(6, 16, id(&oil, "fire"));
    tick(&mut lava);
    tick(&mut ice);
    tick(&mut oil);
    tick(&mut oil_float);
    let oil_floats = min_row(&oil_float, "oil") < min_row(&oil_float, "water");
    println!(
        "materials,1,lava_stone={},lava_steam={},ice_water={},ice_left={},oil_floats={},oil_float_rows={:?},oil_fire={},oil_left={}",
        count(&lava, "stone"),
        count(&lava, "steam"),
        count(&ice, "water"),
        count(&ice, "ice"),
        oil_floats,
        (min_row(&oil_float, "oil"), min_row(&oil_float, "water")),
        count(&oil, "oil_fire"),
        count(&oil, "oil")
    );
    assert!(count(&lava, "stone") > 0 && count(&lava, "steam") > 0);
    assert!(count(&ice, "water") > 0 && count(&ice, "ice") == 0);
    assert!(oil_floats && count(&oil, "oil_fire") > 0);

    // A live rules toggle prevents an active front from consuming neighboring trees.
    let mut toggle = world(24, 12, 23);
    let fire_tree = toggle
        .rules()
        .interactions
        .iter()
        .position(|r| {
            toggle.rules().elements[r.when].name == "fire"
                && toggle.rules().elements[r.with].name == "tree"
        })
        .unwrap();
    toggle.set_interaction(fire_tree, false, 1.0).unwrap();
    for x in 6..18 {
        toggle.paint(x, 7, id(&toggle, "soil"));
        toggle.paint(x, 6, id(&toggle, "tree"));
    }
    toggle.paint(6, 6, id(&toggle, "fire"));
    for _ in 0..30 {
        tick(&mut toggle);
    }
    println!(
        "rule_toggle,30,fire={},tree={}",
        count(&toggle, "fire"),
        count(&toggle, "tree")
    );
    assert_eq!(count(&toggle, "fire"), 0);
    assert_eq!(count(&toggle, "tree"), 11);
}
