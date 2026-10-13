//! Headless tests for setup transitions; no native window or GPU required.
use super::*;

#[test]
fn startup_waits_for_setup_and_cancel_preserves_a_running_game() {
    let mut app = Tinderbox::new_state(false);
    assert!(app.level_setup.open);
    assert!(!app.running);
    assert!(app.world.pending_dirty_regions() > 0);
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
fn level_recipe_round_trip_and_reset_reproduce_per_cell_attributes() {
    let mut app = Tinderbox::new_state(false);
    let mut level = Preset::DryForest.level(app.world.rules(), 192, 120);
    level.ambient_humidity = 100;
    level.ambient_oxygen = 0;
    let encoded = level.to_toml().unwrap();
    let loaded = Level::load(&encoded).unwrap();
    assert_eq!(loaded, level);
    app.setup_action(SetupAction::Play(loaded.clone()));
    let initial = app.world.cells().to_vec();
    app.world.set_ambient(20, 100, 21);
    for _ in 0..20 {
        app.world.step(usize::MAX);
    }
    assert!(app.world.cells().iter().any(|cell| cell.moisture > 0));
    assert!(app.world.cells().iter().any(|cell| cell.burn_progress > 0));
    assert_ne!(app.world.cells(), initial);
    app.reset();
    assert_eq!(app.world.cells(), initial);
    app.setup_action(SetupAction::Play(Level::load(&encoded).unwrap()));
    assert_eq!(app.world.cells(), initial);
    assert_eq!(app.world.atmosphere_at(0, 0).unwrap().oxygen, 0.0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_recipe_capture_arguments_validate_as_a_pair() {
    let args = native_args_from([
        "--level".into(),
        "dry.toml".into(),
        "--capture-after".into(),
        "120".into(),
        "--screenshot".into(),
        "dry.png".into(),
    ])
    .unwrap();
    assert_eq!(args.level_path, Some(PathBuf::from("dry.toml")));
    assert_eq!(args.capture.after_frames, Some(120));
    assert_eq!(args.capture.output, Some(PathBuf::from("dry.png")));
    let initial = native_args_from([
        "--level".into(),
        "sealed.toml".into(),
        "--capture-after".into(),
        "0".into(),
        "--screenshot".into(),
        "initial.png".into(),
    ])
    .unwrap();
    assert_eq!(initial.capture.after_frames, Some(0));
    assert!(native_args_from(["--capture-after".into(), "-1".into()]).is_err());
    assert!(
        native_args_from([
            "--level".into(),
            "dry.toml".into(),
            "--capture-after".into(),
            "12".into()
        ])
        .is_err()
    );
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

#[test]
fn work_accumulator_issues_default_budget_and_preserves_fractional_work() {
    let mut work = WorkAccumulator::default();
    work.accrue(1.0 / 60.0, 1.0);
    assert_eq!(work.take(), WORK_PER_FRAME);
    assert!(work.pending < 0.01);
    work.accrue(1.0 / 120.0, 1.0);
    assert_eq!(work.take(), WORK_PER_FRAME / 2);
    assert!(work.pending < 0.01);
}

#[test]
fn frame_cap_carries_backlog_but_bounds_long_hitches() {
    let mut work = WorkAccumulator::default();
    work.accrue(0.1, 1.0);
    assert_eq!(work.take(), MAX_WORK_PER_FRAME);
    assert!(work.pending > 0.0);
    work.accrue(1.0 / 60.0, 1.0);
    let next = work.take();
    assert!((MAX_WORK_PER_FRAME / 2..=MAX_WORK_PER_FRAME).contains(&next));
    assert!(work.pending < 0.01);
    let carry_cap = (MAX_WORK_PER_FRAME * MAX_CARRY_FRAMES) as f64;
    work.accrue(10.0, 4.0);
    assert_eq!(work.take(), MAX_WORK_PER_FRAME);
    assert!(work.pending <= carry_cap - MAX_WORK_PER_FRAME as f64);
    assert!(work.pending > 0.0);
}

#[test]
fn manual_steps_share_the_frame_cap_and_can_run_while_paused() {
    let mut work = WorkAccumulator::default();
    work.add(WORK_PER_FRAME as f64);
    assert_eq!(work.take(), WORK_PER_FRAME);
    assert_eq!(work.take(), 0);

    work.add((MAX_WORK_PER_FRAME * 2) as f64);
    assert_eq!(work.take(), MAX_WORK_PER_FRAME);
    assert_eq!(work.take(), MAX_WORK_PER_FRAME);
    assert_eq!(work.pending, 0.0);
}
