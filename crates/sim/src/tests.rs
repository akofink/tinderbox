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
