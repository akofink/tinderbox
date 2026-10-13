//! Headless tests for setup transitions; no native window or GPU required.
use super::*;

#[test]
fn startup_waits_for_setup_and_cancel_preserves_a_running_game() {
    let mut app = Tinderbox::new_state(false);
    assert!(app.level_setup.open);
    assert!(!app.running);
    app.setup_action(SetupAction::Play(app.level.clone()));
    app.world.step(500);
    let cells = app.world.cells().to_vec();
    app.open_level_setup();
    assert!(!app.running);
    app.level_setup.draft.seed += 1;
    app.setup_action(SetupAction::Cancel);
    assert!(app.running);
    assert!(!app.level_setup.open);
    assert_eq!(cells, app.world.cells());
    assert_ne!(app.level.seed, app.level_setup.draft.seed);
}

#[test]
fn play_applies_size_and_reset_restores_cells_and_seeded_evolution() {
    let mut app = Tinderbox::new_state(false);
    let level = Preset::Island.level(app.world.rules(), 80, 48);
    app.setup_action(SetupAction::Play(level.clone()));
    assert_eq!((app.world.width(), app.world.height()), (80, 48));
    let start = app.world.cells().to_vec();
    app.world.paint(5, 5, app.world.rules().id("fire").unwrap());
    app.world.step(1000);
    app.world.set_wind(tinderbox_sim::WindDirection::West, 4);
    app.reset();
    assert_eq!(app.level, level);
    assert_eq!(app.world.cells(), start);
    assert_eq!(app.world.pending_work(), 0);
    assert_eq!(app.world.wind(), level.wind);
    let mut reference = level.generate(app.world.rules().clone()).unwrap();
    for _ in 0..20 {
        app.world.step(512);
        reference.step(512);
    }
    assert_eq!(app.world.cells(), reference.cells());
}

#[test]
fn level_reset_preserves_live_rule_tuning() {
    let mut app = Tinderbox::new_state(false);
    app.world.set_interaction(0, false, 0.25).unwrap();
    let tuned = app.world.rules().to_toml().unwrap();
    app.reset();
    assert_eq!(app.world.rules().to_toml().unwrap(), tuned);
}

#[test]
fn large_levels_fit_the_canvas_instead_of_being_cropped() {
    let app = Tinderbox::new_state(false);
    let world = Preset::Island
        .level(app.world.rules(), 1024, 640)
        .generate(app.world.rules().clone())
        .unwrap();
    let available = egui::Vec2::new(800.0, 500.0);
    let size = render::fitted_size(&world, available);
    assert!(size.x <= available.x && size.y <= available.y);
    assert!((size.x / size.y - 1.6).abs() < 0.001);
}

#[test]
fn invalid_apply_preserves_world_and_applied_recipe() {
    let mut app = Tinderbox::new_state(false);
    let original = app.level.clone();
    let cells = app.world.cells().to_vec();
    let mut invalid = original.clone();
    invalid.width = 0;
    app.setup_action(SetupAction::Play(invalid));
    assert_eq!(app.level, original);
    assert_eq!(app.world.cells(), cells);
    assert!(app.level_setup.open);
    assert!(app.level_setup.error.is_some());
    assert!(!app.running);
}

#[test]
fn setup_blocks_keyboard_play_and_step() {
    let mut app = Tinderbox::new_state(false);
    let cells = app.world.cells().to_vec();
    let ctx = egui::Context::default();
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| input::handle_keyboard(&mut app, ctx),
    );
    assert!(!app.running);
    assert_eq!(app.world.cells(), cells);
}

#[test]
fn shortcuts_work_with_button_focus_but_not_in_text_fields() {
    for text_focus in [false, true] {
        let mut app = Tinderbox::new_state(false);
        app.setup_action(SetupAction::Play(app.level.clone()));
        let ctx = egui::Context::default();
        let id = egui::Id::new("focused_control");
        let _ = ctx.run(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Space,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| {
                ctx.memory_mut(|m| m.request_focus(id));
                if text_focus {
                    egui::text_edit::TextEditState::default().store(ctx, id);
                }
                input::handle_keyboard(&mut app, ctx);
            },
        );
        assert_eq!(app.running, text_focus);
    }
}

#[test]
fn smooth_play_skips_setup_and_handles_small_worlds() {
    let mut app = Tinderbox::new_state(true);
    assert!(app.running);
    assert!(!app.level_setup.open);
    let level = Preset::DryForest.level(app.world.rules(), 16, 16);
    app.set_level(level).unwrap();
    app.simulate_frame(&egui::Context::default(), 1.0 / 60.0);
}
