//! Headless tests for setup transitions; no native window or GPU required.
use super::*;

fn play_level(app: &mut Tinderbox, level: Level) {
    if app.level_setup.draft != level {
        app.level_setup.draft = level.clone();
        app.level_setup.revision = app.level_setup.revision.wrapping_add(1);
    }
    app.setup_action(SetupAction::Play(level.clone()));
    if app.play_when_preview_ready {
        let preview = level.generate(app.world.rules().clone()).unwrap();
        app.apply_setup_preview(preview, app.level_setup.revision);
    }
}

#[test]
fn default_world_dimensions_follow_canvas_size_and_cell_size() {
    let a = fitted_world_dimensions(egui::vec2(960.0, 700.0), 3.5);
    let b = fitted_world_dimensions(egui::vec2(1280.0, 700.0), 3.5);
    let zoomed = fitted_world_dimensions(egui::vec2(960.0, 700.0), 4.0);
    assert_eq!(a, (274, 200));
    assert_eq!(b, (365, 200));
    assert_eq!(zoomed, (240, 175));
}

#[test]
fn debounced_canvas_resize_preserves_cells_and_updates_the_texture_grid() {
    let mut app = Tinderbox::new_state(false);
    let water = app.world.rules().id("water").unwrap();
    app.world.paint(3, 240, water);
    let previous = app.world.cells().to_vec();
    let size = egui::vec2(224.0, 168.0);
    app.resize_world_to_fit(size);
    assert_eq!((app.world.width(), app.world.height()), (400, 250));
    app.resize_target_since = Instant::now() - std::time::Duration::from_millis(121);
    app.resize_world_to_fit(size);
    assert_eq!((app.world.width(), app.world.height()), (64, 48));
    assert_eq!(app.world.cells()[38 * 64 + 3], previous[240 * 400 + 3]);
    assert_eq!(app.world_texture.size(), [64, 48]);
}

#[test]
fn endless_arrow_keys_scroll_the_camera() {
    let mut app = Tinderbox::new_state(false);
    let mut level = app.level.clone();
    level.mode = LevelMode::Endless;
    level.width = 128;
    level.height = 64;
    app.set_level(level).unwrap();
    app.center_camera_on_origin(128.0);
    app.level_setup.open = false;
    let before = app.world_start_chunk * ENDLESS_CHUNK_WIDTH as i64 + app.camera_x as i64;
    let ctx = egui::Context::default();
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| input::handle_keyboard(&mut app, ctx),
    );
    let after = app.world_start_chunk * ENDLESS_CHUNK_WIDTH as i64 + app.camera_x as i64;
    assert_eq!(after, before + 8);
}

#[test]
fn endless_camera_centers_the_world_origin_in_the_actual_viewport() {
    let mut app = Tinderbox::new_state(false);
    let mut level = app.level.clone();
    level.mode = LevelMode::Endless;
    level.width = 1024;
    level.height = 240;
    app.set_level(level).unwrap();
    let origin_x = -(app.world_start_chunk as f32) * ENDLESS_CHUNK_WIDTH as f32;
    app.center_camera_on_origin(360.0);
    assert_eq!(app.camera_x, origin_x - 180.0);
}

#[test]
fn endless_capture_offset_generates_chunks_until_the_camera_is_in_bounds() {
    let mut app = Tinderbox::new_state(false);
    let mut level = app.level.clone();
    level.mode = LevelMode::Endless;
    level.width = 128;
    level.height = 64;
    app.set_level(level).unwrap();
    app.set_camera_x(900);
    assert_eq!(app.camera_x, 900.0);
    assert!(app.world.width() >= 900 + 128 + 2 * ENDLESS_CHUNK_WIDTH);
    assert_eq!(app.world_start_chunk, -2);
}

#[test]
fn endless_camera_expands_deterministic_chunks_in_both_directions() {
    let mut app = Tinderbox::new_state(false);
    let mut level = app.level.clone();
    level.mode = LevelMode::Endless;
    level.width = 128;
    level.height = 64;
    level.fit_window = false;
    app.set_level(level).unwrap();
    app.center_camera_on_origin(128.0);
    let stone = app.world.rules().id("stone").unwrap();
    let original_global_x = app.world_start_chunk * ENDLESS_CHUNK_WIDTH as i64 + 260;
    app.world.paint(260, 3, stone);
    app.pan_camera(-320.0, 128.0);
    assert_eq!(app.world_start_chunk, -3);
    assert_eq!(app.camera_x, 128.0);
    for _ in 0..48 {
        app.pan_camera(32.0, 128.0);
    }
    let east_width = app.world.width();
    assert!(east_width > 5 * ENDLESS_CHUNK_WIDTH);
    for _ in 0..96 {
        app.pan_camera(-32.0, 128.0);
    }
    let local_x = (original_global_x - app.world_start_chunk * ENDLESS_CHUNK_WIDTH as i64) as usize;
    assert_eq!(
        app.world.cells()[3 * app.world.width() + local_x].element,
        stone
    );
    assert!(app.world_start_chunk < -3);
    assert!(app.camera_x >= 0.0);
}

#[test]
fn startup_waits_for_setup_and_cancel_preserves_a_running_game() {
    let mut app = Tinderbox::new_state(false);
    assert!(app.level_setup.open);
    assert!(!app.running);
    assert!(app.world.pending_dirty_regions() > 0);
    let level = app.level.clone();
    play_level(&mut app, level);
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
fn live_setup_preview_regenerates_without_play_and_cancel_restores_the_running_world() {
    let mut app = Tinderbox::new_state(false);
    let level = app.level.clone();
    play_level(&mut app, level);
    app.world.step(500);
    let cells = app.world.cells().to_vec();
    let original_size = (app.world.width(), app.world.height());
    let original_level = app.level.clone();
    app.open_level_setup();

    let level = Preset::Island.level(app.world.rules(), 80, 48);
    app.level_setup.draft = level.clone();
    app.level_setup.revision = app.level_setup.revision.wrapping_add(1);
    let revision = app.level_setup.revision;
    app.last_setup_revision = revision;
    app.last_setup_change = Instant::now() - std::time::Duration::from_millis(201);
    app.update_setup_preview();
    #[cfg(not(target_arch = "wasm32"))]
    for _ in 0..500 {
        if app.preview_revision == Some(revision) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
        app.update_setup_preview();
    }
    #[cfg(target_arch = "wasm32")]
    app.update_setup_preview();
    assert_eq!(app.preview_revision, Some(revision));

    assert_eq!((app.world.width(), app.world.height()), (80, 48));
    assert_ne!(app.world.cells(), cells);
    assert_eq!(app.level, original_level);
    app.setup_action(SetupAction::Cancel);
    assert_eq!((app.world.width(), app.world.height()), original_size);
    assert_eq!(app.world.cells(), cells);
    assert_eq!(app.level, original_level);
    assert!(app.running);
}

#[test]
fn play_waits_for_the_debounced_preview_then_keeps_that_world() {
    let mut app = Tinderbox::new_state(false);
    let level = Preset::Island.level(app.world.rules(), 80, 48);
    app.level_setup.draft = level.clone();
    app.level_setup.revision = app.level_setup.revision.wrapping_add(1);
    let revision = app.level_setup.revision;
    app.setup_action(SetupAction::Play(level.clone()));
    assert!(app.level_setup.open);
    assert!(!app.running);
    assert!(app.play_when_preview_ready);

    let preview = level.generate(app.world.rules().clone()).unwrap();
    app.apply_setup_preview(preview, revision);
    assert!(!app.level_setup.open);
    assert!(app.running);
    assert_eq!(app.level, level);
    assert_eq!((app.world.width(), app.world.height()), (80, 48));
    #[cfg(not(target_arch = "wasm32"))]
    assert!(app.setup_backup.is_none());
}

#[test]
fn play_applies_size_and_reset_restores_cells_and_seeded_evolution() {
    let mut app = Tinderbox::new_state(false);
    let level = Preset::Island.level(app.world.rules(), 80, 48);
    play_level(&mut app, level.clone());
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
    assert!(size.x > 0.0 && size.y > 0.0);
    assert!((size.x / size.y - 1.6).abs() < 0.001);
    assert!((size.x / world.width() as f32 - size.y / world.height() as f32).abs() < 0.001);
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
        let level = app.level.clone();
        play_level(&mut app, level);
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
    play_level(&mut app, loaded.clone());
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
    play_level(&mut app, Level::load(&encoded).unwrap());
    assert_eq!(app.world.cells(), initial);
    assert_eq!(app.world.atmosphere_at(0, 0).unwrap().oxygen, 0.0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn smooth_play_is_vsynced_while_capture_windows_are_background() {
    assert!(!background_window_requested(Vec::<String>::new()));
    assert!(!background_window_requested(["--smooth-play".into()]));
    assert!(background_window_requested([
        "--level".into(),
        "dry.toml".into(),
        "--capture-at-seconds".into(),
        "5".into(),
        "--screenshot".into(),
        "dry.png".into(),
    ]));
    assert!(background_window_requested([
        "--level".into(),
        "dry.toml".into(),
        "--capture-after".into(),
        "300".into(),
        "--screenshot".into(),
        "dry.png".into(),
    ]));
    assert!(!background_window_requested([
        "--level".into(),
        "dry.toml".into(),
    ]));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_recipe_capture_arguments_validate_as_a_pair() {
    let args = native_args_from([
        "--level".into(),
        "dry.toml".into(),
        "--camera-x".into(),
        "700".into(),
        "--capture-after".into(),
        "120".into(),
        "--screenshot".into(),
        "dry.png".into(),
        "--inspect-at".into(),
        "24,18".into(),
    ])
    .unwrap();
    assert_eq!(args.level_path, Some(PathBuf::from("dry.toml")));
    assert_eq!(args.camera_x, Some(700));
    assert_eq!(args.capture.after_frames, Some(120));
    assert_eq!(args.capture.at_seconds, None);
    assert_eq!(args.capture.output, Some(PathBuf::from("dry.png")));
    assert_eq!(args.capture.inspect_at, Some((24, 18)));
    assert!(native_args_from(["--inspect-at".into(), "bad".into()]).is_err());
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
    let timed = native_args_from([
        "--level".into(),
        "lab.toml".into(),
        "--capture-at-seconds".into(),
        "60".into(),
        "--screenshot".into(),
        "lab.png".into(),
    ])
    .unwrap();
    assert_eq!(timed.capture.at_seconds, Some(60));
    assert!(native_args_from(["--capture-after".into(), "-1".into()]).is_err());
    assert!(native_args_from(["--capture-at-seconds".into(), "-1".into()]).is_err());
    assert!(native_args_from(["--camera-x".into(), "west".into()]).is_err());
    assert!(
        native_args_from([
            "--capture-at-seconds".into(),
            "5".into(),
            "--screenshot".into(),
            "scene.png".into(),
        ])
        .is_err()
    );
    assert!(
        native_args_from([
            "--level".into(),
            "lab.toml".into(),
            "--capture-after".into(),
            "60".into(),
            "--capture-at-seconds".into(),
            "5".into(),
            "--screenshot".into(),
            "lab.png".into(),
        ])
        .is_err()
    );
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
    let wood = app.world.rules().id("wood").unwrap();
    assert!(
        app.world
            .cells()
            .iter()
            .filter(|cell| cell.element == wood)
            .count()
            >= 69,
        "smooth-play seeds several boats on water"
    );
    let level = Preset::DryForest.level(app.world.rules(), 16, 16);
    app.set_level(level).unwrap();
    app.simulate_frame(&egui::Context::default(), FIXED_FRAME_DT, FIXED_FRAME_DT);
}

#[test]
fn deterministic_native_step_targets_sixty_hertz() {
    assert!((FIXED_FRAME_DT - 1.0 / 60.0).abs() < f32::EPSILON);
    assert_eq!(MAX_INTERACTIVE_FRAME_DT, 0.25);
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
    assert_eq!(next, MAX_WORK_PER_FRAME);
    assert!(work.pending > 0.0);
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
