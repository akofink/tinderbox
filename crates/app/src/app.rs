//! Application loop and shared sandbox state. Native and wasm share this UI.
pub(crate) mod files;
pub(crate) mod input;
mod inspector;
mod level_setup;
mod panels;
mod render;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(test)]
mod tests;

use crate::rules_panel::RulesPanel;
use eframe::egui;
use level_setup::{LevelSetup, SetupAction};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    time::SystemTime,
};
use tinderbox_sim::level::ENDLESS_CHUNK_WIDTH;
#[cfg(not(target_arch = "wasm32"))]
use tinderbox_sim::level::{MAX_LEVEL_CELLS, MAX_LEVEL_DIMENSION};
use tinderbox_sim::{
    DEFAULT_RULES, Rules, World,
    level::{Level, LevelError, LevelMode, Preset},
};
use web_time::Instant;

const DEFAULT_WIDTH: usize = 400;
const DEFAULT_HEIGHT: usize = 250;
const DEFAULT_CELL_SIZE: f32 = 3.5;
const WORK_PER_FRAME: usize = 12_000;
const WORK_UNITS_PER_SECOND: f64 = (WORK_PER_FRAME * 60) as f64;
const MAX_WORK_PER_FRAME: usize = WORK_PER_FRAME;
const MAX_CARRY_FRAMES: usize = 3;
const MAX_SPEED: f32 = 10.0;
const TARGET_SIMULATION_MS_PER_FRAME: f64 = 10.0;
const MAX_ADAPTIVE_WORK_PER_FRAME: usize = WORK_PER_FRAME * MAX_SPEED as usize;
pub(super) const MAX_TEXTURE_REGIONS_PER_FRAME: usize = 32;
const FIXED_FRAME_DT: f32 = 1.0 / 60.0;
#[cfg(not(target_arch = "wasm32"))]
const MIN_SMOOTH_PLAY_FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(15);
// Exclude the initial window/surface startup intervals from steady-play metrics.
const SMOOTH_PLAY_WARMUP_FRAMES: usize = 2;
const MAX_INTERACTIVE_FRAME_DT: f32 = 0.25;

fn simulation_frame_dt(wall_dt: f32, fixed_step: bool) -> f32 {
    if fixed_step {
        FIXED_FRAME_DT
    } else {
        wall_dt.clamp(0.0, MAX_INTERACTIVE_FRAME_DT)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Overlay {
    #[default]
    None,
    Temperature,
    Humidity,
    Pressure,
    Oxygen,
    Moisture,
    BurnProgress,
}

#[derive(Default)]
struct WorkAccumulator {
    pending: f64,
    dropped: f64,
}
impl WorkAccumulator {
    fn accrue(&mut self, dt: f32, speed: f32) {
        let speed = speed.clamp(0.25, MAX_SPEED);
        let due = dt.max(0.0) as f64 * WORK_UNITS_PER_SECOND * speed as f64;
        self.add_with_cap(
            due,
            (MAX_WORK_PER_FRAME * MAX_CARRY_FRAMES) as f64 * speed as f64,
        );
    }

    fn add(&mut self, units: f64) {
        self.add_with_cap(units, (MAX_WORK_PER_FRAME * MAX_CARRY_FRAMES) as f64);
    }

    fn add_with_cap(&mut self, units: f64, cap: f64) {
        let total = self.pending + units.max(0.0);
        self.pending = total.min(cap);
        self.dropped += (total - cap).max(0.0);
    }

    fn take(&mut self, budget_limit: usize) -> usize {
        let budget = (self.pending.floor() as usize).min(budget_limit);
        self.pending -= budget as f64;
        budget
    }

    fn trim_to_speed(&mut self, speed: f32) {
        let speed = speed.clamp(0.25, MAX_SPEED);
        let cap = (MAX_WORK_PER_FRAME * MAX_CARRY_FRAMES) as f64 * speed as f64;
        self.dropped += (self.pending - cap).max(0.0);
        self.pending = self.pending.min(cap);
    }
}

#[derive(Clone, Copy)]
struct AdaptiveWorkBudget {
    work_per_ms: f64,
}
impl Default for AdaptiveWorkBudget {
    fn default() -> Self {
        Self {
            work_per_ms: MAX_WORK_PER_FRAME as f64 / TARGET_SIMULATION_MS_PER_FRAME,
        }
    }
}
impl AdaptiveWorkBudget {
    fn limit(self, fixed: bool) -> usize {
        if fixed {
            MAX_WORK_PER_FRAME
        } else {
            (self.work_per_ms * TARGET_SIMULATION_MS_PER_FRAME)
                .round()
                .clamp(1.0, MAX_ADAPTIVE_WORK_PER_FRAME as f64) as usize
        }
    }

    fn observe(&mut self, completed: usize, elapsed_ms: f64) {
        if completed == 0 || elapsed_ms <= 0.0 {
            return;
        }
        let sample = completed as f64 / elapsed_ms;
        self.work_per_ms = self.work_per_ms * 0.8 + sample * 0.2;
    }
}

pub(super) struct Tinderbox {
    world: World,
    world_texture: eframe::egui::TextureHandle,
    selected: usize,
    brush: usize,
    gust_mode: bool,
    boat_tool: bool,
    inspect_mode: bool,
    inspect_cell: Option<(usize, usize)>,
    overlay: Overlay,
    running: bool,
    speed: f32,
    cell_size: f32,
    show_hud: bool,
    rules_panel: RulesPanel,
    level: Level,
    level_setup: LevelSetup,
    return_running: bool,
    sim_seconds: f64,
    sim_rate: f64,
    work: WorkAccumulator,
    adaptive_budget: AdaptiveWorkBudget,
    #[cfg(not(target_arch = "wasm32"))]
    measure_elapsed: f64,
    #[cfg(not(target_arch = "wasm32"))]
    measure_frame_total_ms: f64,
    #[cfg(not(target_arch = "wasm32"))]
    measure_frame_max_ms: f32,
    #[cfg(not(target_arch = "wasm32"))]
    measure_frames: usize,
    #[cfg(not(target_arch = "wasm32"))]
    measure_reported: bool,
    #[cfg(not(target_arch = "wasm32"))]
    measure_requested_speed: f32,
    peak_carry_work: usize,
    max_dirty_regions_uploaded: usize,
    last_frame: Instant,
    frame_ms: f32,
    camera_x: f32,
    camera_y: f32,
    camera_view_width: f32,
    camera_view_height: f32,
    camera_zoom: f32,
    navigation_mode: bool,
    touch_gesture_active: bool,
    camera_initialized: bool,
    world_start_chunk: i64,
    stress_play: bool,
    stress_frames: usize,
    stress_total_ms: f64,
    stress_max_ms: f32,
    stress_frame_intervals_ms: Vec<f32>,
    stress_max_pan_ms: f32,
    stress_max_extend_ms: f32,
    stress_max_texture_ms: f32,
    stress_max_upload_ms: f32,
    stress_max_pending_dirty_regions: usize,
    stress_max_draw_ms: f32,
    stress_max_step_ms: f32,
    stress_max_update_work_ms: f32,
    stress_slow_frames: usize,
    #[cfg(not(target_arch = "wasm32"))]
    capture: Option<CaptureOptions>,
    #[cfg(not(target_arch = "wasm32"))]
    capture_frames: usize,
    #[cfg(not(target_arch = "wasm32"))]
    capture_requested: bool,
    #[cfg(not(target_arch = "wasm32"))]
    span_evidence_dir: Option<PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    span_shot_path: Option<PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    span_step: SpanStep,
    #[cfg(not(target_arch = "wasm32"))]
    span_wait_frames: usize,
    #[cfg(not(target_arch = "wasm32"))]
    span_mark: f64,
    #[cfg(not(target_arch = "wasm32"))]
    span_home: f32,
    #[cfg(not(target_arch = "wasm32"))]
    span_grove: f32,
    #[cfg(not(target_arch = "wasm32"))]
    level_file: Option<PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    level_file_modified: Option<SystemTime>,
    #[cfg(not(target_arch = "wasm32"))]
    next_level_file_check: Instant,
    #[cfg(not(target_arch = "wasm32"))]
    preview_job: Option<PreviewJob>,
    #[cfg(not(target_arch = "wasm32"))]
    level_reload_job: Option<LevelReloadJob>,
    setup_backup: Option<World>,
    preview_revision: Option<u64>,
    last_setup_revision: u64,
    last_setup_change: Instant,
    play_when_preview_ready: bool,
    failed_preview_revision: Option<u64>,
}

#[cfg(not(target_arch = "wasm32"))]
struct PreviewJob {
    revision: u64,
    receiver: Receiver<Result<World, LevelError>>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LevelReloadJob {
    level: Level,
    modified: SystemTime,
    receiver: Receiver<Result<World, LevelError>>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Default, PartialEq)]
struct CaptureOptions {
    output: Option<PathBuf>,
    after_frames: Option<usize>,
    at_seconds: Option<u64>,
    measure_real_seconds: Option<f64>,
    inspect_at: Option<(usize, usize)>,
}

#[cfg(not(target_arch = "wasm32"))]
impl CaptureOptions {
    fn fixed_step(&self) -> bool {
        self.after_frames.is_some() || self.at_seconds.is_some()
    }
}

/// Scripted span-demo stages. Each capture moves to its next stage when it is requested.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpanStep {
    Settle,
    ToHome,
    Ignite,
    Burning,
    ToControl,
    ToAway,
    Away,
    ToReturn,
    AfterWait,
    ToControlBack,
    Done,
}

/// Seconds after ignition for the burning capture.
#[cfg(not(target_arch = "wasm32"))]
const SPAN_BURN_SECONDS: f64 = 9.0;
/// Simulated seconds the camera stays away before the away capture.
#[cfg(not(target_arch = "wasm32"))]
const SPAN_AWAY_SECONDS: f64 = 60.0;
/// Simulated seconds after returning before the after-30s capture.
#[cfg(not(target_arch = "wasm32"))]
const SPAN_AFTER_SECONDS: f64 = 30.0;
/// Cell offsets of the control spot (west, outside the span) and the away spot (east).
#[cfg(not(target_arch = "wasm32"))]
const SPAN_CONTROL_OFFSET: f32 = 2000.0;
#[cfg(not(target_arch = "wasm32"))]
const SPAN_AWAY_OFFSET: f32 = 4000.0;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Default, PartialEq)]
struct NativeArgs {
    level_path: Option<PathBuf>,
    camera_x: Option<usize>,
    capture: CaptureOptions,
    drop_boat_at: Option<(usize, usize)>,
    span_evidence: Option<PathBuf>,
    speed: f32,
}
impl Tinderbox {
    fn speed_status(&self) -> String {
        if self.fixed_step_capture() {
            return format!("{:.2}x requested, fixed-step capture", self.speed);
        }
        format!(
            "{:.2}x requested, {:.2}x achieved",
            self.speed, self.sim_rate
        )
    }

    fn fixed_step_capture(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.capture
                .as_ref()
                .is_some_and(CaptureOptions::fixed_step)
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }

    fn fixed_step_simulation(&self) -> bool {
        self.stress_play || self.fixed_step_capture()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let args = native_args().unwrap_or_else(|error| panic!("{error}"));
        let level = args.level_path.as_ref().map(|path| {
            let text = std::fs::read_to_string(path).unwrap_or_else(|error| {
                panic!("Cannot read level recipe {}: {error}", path.display())
            });
            Level::load(&text)
                .unwrap_or_else(|error| panic!("Invalid level recipe {}: {error}", path.display()))
        });
        let (width, height) = level.as_ref().map_or_else(requested_world_size, |level| {
            if level.fit_window || level.width == 0 {
                requested_world_size()
            } else {
                (level.width, level.height)
            }
        });
        let mut app =
            Self::new_with_context(cc.egui_ctx.clone(), width, height, smooth_play_requested());
        if args.level_path.is_none()
            && std::env::args().any(|arg| arg == "--world-size" || arg.starts_with("--world-size="))
        {
            app.level.fit_window = false;
            app.level_setup.draft.fit_window = false;
        }
        app.speed = args.speed;
        app.measure_requested_speed = args.speed;
        app.level_file = args.level_path.clone();
        app.level_file_modified = app.level_file.as_ref().and_then(|path| file_modified(path));
        if let Some(mut level) = level {
            if level.width == 0 || level.height == 0 {
                (level.width, level.height) = (width, height);
            }
            app.level.fit_window = level.fit_window;
            app.set_level(level)
                .expect("validated level recipe must generate");
            if let Some(camera_x) = args.camera_x {
                app.set_camera_x(camera_x);
            }
            app.level_setup.open = false;
            app.running =
                args.capture.after_frames != Some(0) && args.capture.at_seconds != Some(0);
        }
        if args.capture.measure_real_seconds.is_some() {
            app.level_setup.open = false;
            app.running = true;
        }
        if let Some((x, y)) = args.drop_boat_at {
            assert!(
                app.world.place_boat(x, y),
                "--drop-boat-at must fit in empty air or water"
            );
        }
        if let Some(dir) = args.span_evidence.clone() {
            app.level.mode = LevelMode::Endless;
            app.level.fit_window = false;
            app.level_setup.draft.mode = LevelMode::Endless;
            app.level_setup.draft.fit_window = false;
            app.set_level(app.level.clone())
                .expect("endless span evidence level must generate");
            app.level_setup.open = false;
            app.running = true;
            app.span_evidence_dir = Some(dir);
        }
        app.inspect_mode = args.capture.inspect_at.is_some();
        app.inspect_cell = args.capture.inspect_at;
        app.capture = (args.capture.after_frames.is_some()
            || args.capture.at_seconds.is_some()
            || args.capture.measure_real_seconds.is_some())
        .then_some(args.capture);
        app
    }

    #[cfg(test)]
    fn new_state(stress_play: bool) -> Self {
        Self::new_with_context(
            egui::Context::default(),
            DEFAULT_WIDTH,
            DEFAULT_HEIGHT,
            stress_play,
        )
    }

    fn new_with_context(
        ctx: egui::Context,
        width: usize,
        height: usize,
        stress_play: bool,
    ) -> Self {
        let rules = Rules::load(DEFAULT_RULES).expect("bundled rules are validated by tests");
        let selected = rules.id("sand").unwrap_or(0);
        let mut level = Preset::DryForest.level(&rules, width, height);
        level.fit_window = true;
        let mut world = level.generate(rules).expect("default level is valid");
        if stress_play {
            seed_smooth_play_boats(&mut world);
        }
        let air = world.rules().id("air").unwrap_or(0);
        let air_color = world.rules().elements[air].color;
        let world_texture = ctx.load_texture(
            "tinderbox-world",
            egui::ColorImage::new(
                [width, height],
                egui::Color32::from_rgb(air_color[0], air_color[1], air_color[2]),
            ),
            egui::TextureOptions::NEAREST,
        );
        let mut level_setup = LevelSetup::new(level.clone());
        level_setup.open = !stress_play;
        let setup_revision = level_setup.revision;
        Self {
            world,
            world_texture,
            selected,
            brush: 3,
            gust_mode: false,
            boat_tool: false,
            inspect_mode: false,
            inspect_cell: None,
            overlay: Overlay::None,
            running: stress_play,
            speed: 1.0,
            cell_size: DEFAULT_CELL_SIZE,
            show_hud: true,
            rules_panel: RulesPanel::default(),
            level,
            level_setup,
            return_running: false,
            sim_seconds: 0.0,
            sim_rate: 0.0,
            work: WorkAccumulator::default(),
            adaptive_budget: AdaptiveWorkBudget::default(),
            #[cfg(not(target_arch = "wasm32"))]
            measure_elapsed: 0.0,
            #[cfg(not(target_arch = "wasm32"))]
            measure_frame_total_ms: 0.0,
            #[cfg(not(target_arch = "wasm32"))]
            measure_frame_max_ms: 0.0,
            #[cfg(not(target_arch = "wasm32"))]
            measure_frames: 0,
            #[cfg(not(target_arch = "wasm32"))]
            measure_reported: false,
            #[cfg(not(target_arch = "wasm32"))]
            measure_requested_speed: 1.0,
            peak_carry_work: 0,
            max_dirty_regions_uploaded: 0,
            last_frame: Instant::now(),
            frame_ms: 0.0,
            camera_x: 0.0,
            camera_y: 0.0,
            camera_view_width: width as f32,
            camera_view_height: height as f32,
            camera_zoom: 1.0,
            navigation_mode: false,
            touch_gesture_active: false,
            camera_initialized: false,
            world_start_chunk: 0,
            stress_play,
            stress_frames: 0,
            stress_total_ms: 0.0,
            stress_max_ms: 0.0,
            stress_frame_intervals_ms: Vec::new(),
            stress_max_pan_ms: 0.0,
            stress_max_extend_ms: 0.0,
            stress_max_texture_ms: 0.0,
            stress_max_upload_ms: 0.0,
            stress_max_pending_dirty_regions: 0,
            stress_max_draw_ms: 0.0,
            stress_max_step_ms: 0.0,
            stress_max_update_work_ms: 0.0,
            stress_slow_frames: 0,
            #[cfg(not(target_arch = "wasm32"))]
            capture: None,
            #[cfg(not(target_arch = "wasm32"))]
            capture_frames: 0,
            #[cfg(not(target_arch = "wasm32"))]
            capture_requested: false,
            #[cfg(not(target_arch = "wasm32"))]
            span_evidence_dir: None,
            #[cfg(not(target_arch = "wasm32"))]
            span_shot_path: None,
            #[cfg(not(target_arch = "wasm32"))]
            span_step: SpanStep::Settle,
            #[cfg(not(target_arch = "wasm32"))]
            span_wait_frames: 0,
            #[cfg(not(target_arch = "wasm32"))]
            span_mark: 0.0,
            #[cfg(not(target_arch = "wasm32"))]
            span_home: 0.0,
            #[cfg(not(target_arch = "wasm32"))]
            span_grove: 0.0,
            #[cfg(not(target_arch = "wasm32"))]
            level_file: None,
            #[cfg(not(target_arch = "wasm32"))]
            level_file_modified: None,
            #[cfg(not(target_arch = "wasm32"))]
            next_level_file_check: Instant::now(),
            #[cfg(not(target_arch = "wasm32"))]
            preview_job: None,
            #[cfg(not(target_arch = "wasm32"))]
            level_reload_job: None,
            setup_backup: None,
            preview_revision: None,
            last_setup_revision: setup_revision,
            last_setup_change: Instant::now(),
            play_when_preview_ready: false,
            failed_preview_revision: None,
        }
    }

    pub(super) fn resize_world_to_fit(&mut self, _available: egui::Vec2) {
        // A viewport is a camera target, never a reason to regenerate the simulation.
        // Fixed worlds are fit by the renderer; endless worlds keep their active span.
    }

    fn reset_camera(&mut self) {
        self.reset_camera_mode(self.level.mode);
    }

    fn reset_camera_mode(&mut self, mode: LevelMode) {
        self.camera_zoom = 1.0;
        self.camera_x = 0.0;
        self.camera_y = 0.0;
        if mode == LevelMode::Endless {
            let chunks = self.world.width() / ENDLESS_CHUNK_WIDTH;
            self.world_start_chunk = if self.world.chunk_span_enabled() {
                self.world.chunk_origin()
            } else {
                -(chunks as i64 / 2)
            };
            self.camera_x = (self.world.width().saturating_sub(self.level.width) as f32) / 2.0;
            self.camera_initialized = false;
        } else {
            self.world_start_chunk = 0;
            self.camera_x = 0.0;
            self.camera_initialized = true;
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn set_camera_x(&mut self, camera_x: usize) {
        if self.level.mode != LevelMode::Endless {
            return;
        }
        let global = self.world_start_chunk as f32 * ENDLESS_CHUNK_WIDTH as f32 + camera_x as f32;
        self.sync_active_span(global, self.level.width.max(1) as f32);
        self.camera_initialized = true;
    }

    pub(super) fn center_camera_on_origin(&mut self, view_width: f32) {
        self.camera_view_width = view_width.max(1.0);
        if self.level.mode != LevelMode::Endless || self.camera_initialized {
            return;
        }
        let origin_x = -(self.world_start_chunk as f32) * ENDLESS_CHUNK_WIDTH as f32;
        let max_camera_x = (self.world.width() as f32 - view_width.max(1.0)).max(0.0);
        self.camera_x = (origin_x - view_width.max(1.0) / 2.0).clamp(0.0, max_camera_x);
        self.camera_initialized = true;
    }

    pub(super) fn pan_camera(&mut self, delta: f32, view_width: f32) {
        let previous_camera_x = self.camera_x;
        if self.level.mode != LevelMode::Endless {
            self.camera_view_width = view_width.max(1.0);
            self.camera_x = (self.camera_x + delta).clamp(
                0.0,
                (self.world.width() as f32 - self.camera_view_width).max(0.0),
            );
            return;
        }
        let previous_view_width = self.camera_view_width;
        let previous_camera_initialized = self.camera_initialized;
        let previous_world_width = self.world.width();
        let previous_world_start = self.world_start_chunk;
        self.center_camera_on_origin(view_width);
        let global =
            self.world_start_chunk as f32 * ENDLESS_CHUNK_WIDTH as f32 + self.camera_x + delta;
        self.sync_active_span(global, view_width);
        if self.camera_x != previous_camera_x
            || self.world.width() != previous_world_width
            || self.world_start_chunk != previous_world_start
        {
            if !previous_camera_initialized
                || previous_view_width != self.camera_view_width
                || self.world_start_chunk != previous_world_start
            {
                self.mark_visible_world_dirty();
            } else {
                self.mark_newly_visible_world_span(previous_camera_x, previous_view_width);
            }
        }
    }

    pub(super) fn zoom_camera(
        &mut self,
        factor: f32,
        anchor: egui::Vec2,
        base_view_width: f32,
        base_view_height: f32,
    ) {
        let old_view_width = self.camera_view_width.max(1.0);
        let old_view_height = self.camera_view_height.max(1.0);
        let anchor = egui::vec2(anchor.x.clamp(0.0, 1.0), anchor.y.clamp(0.0, 1.0));
        let anchor_world_x = self.camera_x + old_view_width * anchor.x;
        let anchor_world_y = self.camera_y + old_view_height * anchor.y;
        let max_zoom = base_view_width.min(base_view_height).clamp(1.0, 8.0);
        self.camera_zoom = (self.camera_zoom * factor).clamp(1.0, max_zoom);
        let new_view_width = base_view_width / self.camera_zoom;
        let new_view_height = (base_view_height / self.camera_zoom).max(1.0);
        self.camera_view_width = new_view_width;
        self.camera_view_height = new_view_height;
        self.camera_x = anchor_world_x - new_view_width * anchor.x;
        self.camera_y = anchor_world_y - new_view_height * anchor.y;
        if self.level.mode == LevelMode::Endless {
            self.camera_initialized = true;
            self.pan_camera(0.0, new_view_width);
        } else {
            self.camera_x = self
                .camera_x
                .clamp(0.0, (self.world.width() as f32 - new_view_width).max(0.0));
        }
        self.camera_y = self
            .camera_y
            .clamp(0.0, (self.world.height() as f32 - new_view_height).max(0.0));
        self.world.mark_all_dirty();
    }

    pub(super) fn pan_camera_2d(
        &mut self,
        delta_x: f32,
        delta_y: f32,
        view_width: f32,
        view_height: f32,
    ) {
        self.pan_camera(delta_x, view_width);
        self.camera_view_height = view_height.max(1.0);
        self.camera_y = (self.camera_y + delta_y).clamp(
            0.0,
            (self.world.height() as f32 - self.camera_view_height).max(0.0),
        );
    }

    fn mark_newly_visible_world_span(&mut self, previous_camera_x: f32, previous_view_width: f32) {
        let old_left = previous_camera_x.floor().max(0.0) as usize;
        let old_right = (previous_camera_x + previous_view_width).ceil().max(0.0) as usize;
        let new_left = self.camera_x.floor().max(0.0) as usize;
        let new_right = (self.camera_x + self.camera_view_width).ceil().max(0.0) as usize;
        if new_left < old_left {
            self.world
                .mark_dirty_rect(new_left, 0, old_left - new_left, self.world.height());
        }
        if new_right > old_right {
            self.world
                .mark_dirty_rect(old_right, 0, new_right - old_right, self.world.height());
        }
    }

    fn mark_visible_world_dirty(&mut self) {
        let left = self.camera_x.floor().max(0.0) as usize;
        let right = (self.camera_x + self.camera_view_width).ceil().max(0.0) as usize;
        self.world
            .mark_dirty_rect(left, 0, right.saturating_sub(left), self.world.height());
    }

    /// Keeps the resident grid to the visible chunks plus the configured margin.
    ///
    /// An unchanged chunk range returns before generating terrain. When the left edge moves,
    /// resident indices shift, so the visible texture rect is invalidated. Dormant chunks are
    /// not uploaded.
    fn sync_active_span(&mut self, global_camera: f32, view_width: f32) {
        if self.level.mode != LevelMode::Endless || !self.world.chunk_span_enabled() {
            return;
        }
        let view_width = view_width.max(1.0);
        self.camera_view_width = view_width;
        let global_i = global_camera.floor() as i64;
        let right = (global_camera + view_width).ceil() as i64;
        let view_cells = usize::try_from(right.saturating_sub(global_i).max(1)).unwrap_or(1);
        if let Some((first, last)) = self.world.active_chunk_range(global_i, view_cells) {
            let count = self.world.width() / ENDLESS_CHUNK_WIDTH;
            let current_last = self.world.chunk_origin() + count as i64 - 1;
            if first == self.world.chunk_origin() && last == current_last {
                self.world_start_chunk = self.world.chunk_origin();
                self.camera_x =
                    global_camera - self.world_start_chunk as f32 * ENDLESS_CHUNK_WIDTH as f32;
                return;
            }
        }
        let rules = self.world.rules().clone();
        let level = self.level.clone();
        let extend_started = Instant::now();
        let adjusted = self
            .world
            .retain_active_span(global_i, view_cells, |index| {
                level
                    .generate_chunk(index, &rules)
                    .map_err(|error| tinderbox_sim::WorldError(error.to_string()))
            });
        if self.stress_play {
            self.stress_max_extend_ms = self
                .stress_max_extend_ms
                .max(extend_started.elapsed().as_secs_f32() * 1000.0);
        }
        let Ok(adjustment) = adjusted else {
            return;
        };
        let origin_shifted = adjustment.added_left > 0 || adjustment.removed_left > 0;
        self.world_start_chunk = adjustment.origin;
        self.camera_x = global_camera - adjustment.origin as f32 * ENDLESS_CHUNK_WIDTH as f32;
        if origin_shifted {
            self.mark_visible_world_dirty();
        }
        let texture_started = Instant::now();
        if self.resize_world_texture() {
            self.world.mark_all_dirty();
        }
        if self.stress_play {
            self.stress_max_texture_ms = self
                .stress_max_texture_ms
                .max(texture_started.elapsed().as_secs_f32() * 1000.0);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn drive_span_evidence(&mut self, ctx: &egui::Context) {
        if self.span_evidence_dir.is_none() || self.level_setup.open || !self.camera_initialized {
            return;
        }
        if self.span_shot_path.is_some() {
            return;
        }
        let dir = self.span_evidence_dir.clone().unwrap();
        let view = self.camera_view_width.max(1.0);
        let settled = self.world.pending_dirty_regions() == 0;
        self.span_wait_frames += 1;
        match self.span_step {
            SpanStep::Settle => {
                let center = self.find_grove_center(view);
                self.span_grove = self.global_x_of(center);
                self.span_home = self.span_grove - view * 0.5;
                println!(
                    "SPAN_DEMO grove resident_x={center} global_x={:.1} home={:.1} control={:.1} away={:.1}",
                    self.span_grove,
                    self.span_home,
                    self.span_home - SPAN_CONTROL_OFFSET,
                    self.span_home + SPAN_AWAY_OFFSET
                );
                self.span_step = SpanStep::ToHome;
                self.span_wait_frames = 0;
            }
            SpanStep::ToHome => {
                if self.span_goto(self.span_home, view) {
                    self.span_shoot(ctx, &dir, "1-before.png", SpanStep::Ignite);
                }
            }
            SpanStep::Ignite => {
                self.ignite_span_grove();
                self.span_mark = self.sim_seconds;
                self.span_step = SpanStep::Burning;
                self.span_wait_frames = 0;
            }
            SpanStep::Burning => {
                if settled && self.sim_seconds >= self.span_mark + SPAN_BURN_SECONDS {
                    self.span_shoot(ctx, &dir, "2-burning.png", SpanStep::ToControl);
                }
            }
            SpanStep::ToControl => {
                if self.span_goto(self.span_home - SPAN_CONTROL_OFFSET, view) {
                    self.span_shoot(ctx, &dir, "control-out.png", SpanStep::ToAway);
                }
            }
            SpanStep::ToAway => {
                if self.span_goto(self.span_home + SPAN_AWAY_OFFSET, view) {
                    self.span_mark = self.sim_seconds;
                    self.span_step = SpanStep::Away;
                    self.span_wait_frames = 0;
                }
            }
            SpanStep::Away => {
                if settled && self.sim_seconds >= self.span_mark + SPAN_AWAY_SECONDS {
                    self.span_shoot(ctx, &dir, "3-away.png", SpanStep::ToReturn);
                }
            }
            SpanStep::ToReturn => {
                if self.span_goto(self.span_home, view) {
                    self.span_mark = self.sim_seconds;
                    self.span_shoot(ctx, &dir, "4-return.png", SpanStep::AfterWait);
                }
            }
            SpanStep::AfterWait => {
                if settled && self.sim_seconds >= self.span_mark + SPAN_AFTER_SECONDS {
                    self.span_shoot(ctx, &dir, "5-after-30s.png", SpanStep::ToControlBack);
                }
            }
            SpanStep::ToControlBack => {
                if self.span_goto(self.span_home - SPAN_CONTROL_OFFSET, view) {
                    self.span_shoot(ctx, &dir, "control-back.png", SpanStep::Done);
                }
            }
            SpanStep::Done => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    /// Moves the camera toward a global x. Returns true once it is parked there,
    /// the world is clean, and at least a few frames have passed.
    #[cfg(not(target_arch = "wasm32"))]
    fn span_goto(&mut self, target: f32, view: f32) -> bool {
        let global = self.global_x_of(0) + self.camera_x;
        let delta = target - global;
        if delta.abs() > 1.0 {
            self.pan_camera(delta.clamp(-32.0, 32.0), view);
            return false;
        }
        self.span_wait_frames >= 3 && self.world.pending_dirty_regions() == 0
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn global_x_of(&self, resident_x: usize) -> f32 {
        self.world_start_chunk as f32 * ENDLESS_CHUNK_WIDTH as f32 + resident_x as f32
    }

    /// Picks the resident column whose ±12-column window holds the most tree cells in view.
    #[cfg(not(target_arch = "wasm32"))]
    fn find_grove_center(&self, view: f32) -> usize {
        let Some(tree) = self.world.rules().id("tree") else {
            return self.camera_x as usize;
        };
        let width = self.world.width();
        let height = self.world.height();
        let cells = self.world.cells();
        let column_trees: Vec<usize> = (0..width)
            .map(|x| {
                (0..height)
                    .filter(|&y| cells[y * width + x].element == tree)
                    .count()
            })
            .collect();
        let first = (self.camera_x as usize).min(width.saturating_sub(1));
        let last = ((self.camera_x + view) as usize).min(width);
        let radius = 12usize;
        (first..last)
            .max_by_key(|&center| {
                let lo = center.saturating_sub(radius);
                let hi = (center + radius + 1).min(width);
                column_trees[lo..hi].iter().sum::<usize>()
            })
            .unwrap_or(first)
    }

    /// Ignites the lowest tree in each column of the grove, so fire starts at ground level.
    #[cfg(not(target_arch = "wasm32"))]
    fn ignite_span_grove(&mut self) {
        let (Some(fire), Some(tree)) =
            (self.world.rules().id("fire"), self.world.rules().id("tree"))
        else {
            return;
        };
        let center = (self.span_grove - self.global_x_of(0)).round().max(0.0) as usize;
        let width = self.world.width();
        let height = self.world.height();
        let mut ignited = Vec::new();
        for x in center.saturating_sub(2)..(center + 3).min(width) {
            let base = (0..height)
                .rev()
                .find(|&y| self.world.cells()[y * width + x].element == tree);
            if let Some(y) = base {
                self.world.paint(x, y, fire);
                ignited.push((x, y));
            }
        }
        println!(
            "SPAN_DEMO ignited ground-level cells={:?} sim={:.2} camera_chunk={} world={}x{}",
            ignited,
            self.sim_seconds,
            self.world_start_chunk + (self.camera_x as usize / ENDLESS_CHUNK_WIDTH) as i64,
            width,
            height
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn span_shoot(
        &mut self,
        ctx: &egui::Context,
        dir: &std::path::Path,
        name: &str,
        next: SpanStep,
    ) {
        println!(
            "SPAN_DEMO capture={name} sim={:.2} camera_chunk={} world={}x{} dormant_chunks={}",
            self.sim_seconds,
            self.world_start_chunk + (self.camera_x as usize / ENDLESS_CHUNK_WIDTH) as i64,
            self.world.width(),
            self.world.height(),
            self.world.dormant_chunk_count()
        );
        self.span_step = next;
        self.span_wait_frames = 0;
        self.request_span_shot(ctx, dir.join(name));
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn request_span_shot(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.span_shot_path = Some(path);
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
    }

    fn resize_world_texture(&mut self) -> bool {
        self.resize_world_texture_for_mode(self.level.mode)
    }

    fn resize_world_texture_for_mode(&mut self, mode: LevelMode) -> bool {
        let texture_width = if mode == LevelMode::Endless {
            self.world
                .width()
                .next_power_of_two()
                .max(self.world_texture.size()[0].max(8192))
        } else {
            self.world.width()
        };
        if self.world_texture.size() == [texture_width, self.world.height()] {
            return false;
        }
        let air = self.world.rules().id("air").unwrap_or(0);
        let color = self.world.rules().elements[air].color;
        self.world_texture.set(
            egui::ColorImage::new(
                [texture_width, self.world.height()],
                egui::Color32::from_rgb(color[0], color[1], color[2]),
            ),
            egui::TextureOptions::NEAREST,
        );
        true
    }

    fn install_world(&mut self, world: World) {
        let size_changed =
            (world.width(), world.height()) != (self.world.width(), self.world.height());
        self.world = world;
        if size_changed {
            self.resize_world_texture();
        }
        self.world.mark_all_dirty();
    }

    fn reset_simulation_clock(&mut self) {
        self.sim_seconds = 0.0;
        self.sim_rate = 0.0;
        self.work = WorkAccumulator::default();
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.measure_elapsed = 0.0;
            self.measure_frame_total_ms = 0.0;
            self.measure_frame_max_ms = 0.0;
            self.measure_frames = 0;
            self.measure_reported = false;
        }
        self.adaptive_budget = AdaptiveWorkBudget::default();
        self.peak_carry_work = 0;
        self.max_dirty_regions_uploaded = 0;
    }

    fn set_level(&mut self, level: Level) -> Result<(), LevelError> {
        let world = level.generate(self.world.rules().clone())?;
        self.level = level;
        self.install_world(world);
        self.reset_camera();
        self.reset_simulation_clock();
        self.preview_revision = None;
        Ok(())
    }

    fn request_step(&mut self) {
        self.work.add(WORK_PER_FRAME as f64);
    }

    fn reset(&mut self) {
        if let Err(error) = self.set_level(self.level.clone()) {
            self.open_level_setup();
            self.level_setup.error = Some(error.to_string());
        }
    }

    fn open_level_setup(&mut self) {
        self.return_running = self.running;
        self.running = false;
        self.level_setup.begin(&self.level, self.world.rules());
        self.last_setup_revision = self.level_setup.revision;
        self.preview_revision = None;
        self.last_setup_change = Instant::now();
        self.play_when_preview_ready = false;
        self.failed_preview_revision = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.level_reload_job = None;
        }
        self.setup_backup = None;
    }

    fn setup_action(&mut self, action: SetupAction) {
        match action {
            SetupAction::Play(level) => {
                if let Err(error) = level.validate() {
                    self.level_setup.error = Some(error.to_string());
                    return;
                }
                if self.failed_preview_revision == Some(self.level_setup.revision) {
                    return;
                }
                if self.preview_revision == Some(self.level_setup.revision) {
                    self.finish_setup_play(level);
                } else {
                    self.play_when_preview_ready = true;
                    self.level_setup.set_message(
                        "Preparing this level. Play will start when its preview is ready.",
                    );
                    self.last_setup_change = Instant::now() - std::time::Duration::from_millis(200);
                }
            }
            SetupAction::Cancel => {
                if let Some(world) = self.setup_backup.take() {
                    self.install_world(world);
                    self.reset_camera();
                }
                self.preview_revision = None;
                self.play_when_preview_ready = false;
                self.level_setup.open = false;
                self.running = self.return_running;
            }
        }
    }

    fn finish_setup_play(&mut self, level: Level) {
        self.level = level;
        if self.level.mode == LevelMode::Endless {
            match self.level.generate(self.world.rules().clone()) {
                Ok(world) => self.install_world(world),
                Err(error) => {
                    self.level_setup.error = Some(error.to_string());
                    return;
                }
            }
        }
        self.reset_camera();
        self.reset_simulation_clock();
        self.preview_revision = None;
        self.play_when_preview_ready = false;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.preview_job = None;
            self.level_reload_job = None;
        }
        self.setup_backup = None;
        self.level_setup.open = false;
        self.level_setup.set_message("");
        self.running = true;
    }

    fn apply_setup_preview(&mut self, world: World, revision: u64) {
        if self.setup_backup.is_none() {
            let size_changed =
                (world.width(), world.height()) != (self.world.width(), self.world.height());
            self.setup_backup = Some(std::mem::replace(&mut self.world, world));
            if size_changed {
                self.resize_world_texture();
            }
            self.reset_camera_mode(self.level_setup.draft.mode);
            self.world.mark_all_dirty();
        } else {
            self.install_world(world);
            self.reset_camera_mode(self.level_setup.draft.mode);
        }
        if self.resize_world_texture_for_mode(self.level_setup.draft.mode) {
            self.world.mark_all_dirty();
        }
        self.preview_revision = Some(revision);
        self.level_setup.error = None;
        if self.play_when_preview_ready {
            self.finish_setup_play(self.level_setup.draft.clone());
        }
    }

    fn update_setup_preview(&mut self) {
        if !self.level_setup.open {
            #[cfg(not(target_arch = "wasm32"))]
            if self
                .preview_job
                .as_ref()
                .is_some_and(|job| job.receiver.try_recv().is_ok())
            {
                self.preview_job = None;
            }
            return;
        }
        let revision = self.level_setup.revision;
        if revision != self.last_setup_revision {
            self.last_setup_revision = revision;
            self.failed_preview_revision = None;
            self.last_setup_change = Instant::now();
        }
        if self.preview_revision == Some(revision)
            || self.failed_preview_revision == Some(revision)
            || self.last_setup_change.elapsed() < std::time::Duration::from_millis(200)
        {
            return;
        }
        let level = self.level_setup.draft.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(job) = &self.preview_job {
                if let Ok(result) = job.receiver.try_recv() {
                    let job = self.preview_job.take().expect("preview job exists");
                    if job.revision == self.level_setup.revision && self.level_setup.open {
                        match result {
                            Ok(world) => self.apply_setup_preview(world, job.revision),
                            Err(error) => {
                                self.failed_preview_revision = Some(job.revision);
                                self.play_when_preview_ready = false;
                                self.level_setup.error = Some(error.to_string());
                            }
                        }
                    }
                }
            }
            if self.level_setup.open
                && self.preview_job.is_none()
                && self.preview_revision != Some(revision)
            {
                let rules = self.world.rules().clone();
                let (sender, receiver) = mpsc::channel();
                let worker_level = level.clone();
                std::thread::spawn(move || {
                    let _ = sender.send(worker_level.generate(rules));
                });
                self.preview_job = Some(PreviewJob { revision, receiver });
            }
        }
        #[cfg(target_arch = "wasm32")]
        if self.preview_revision != Some(revision) {
            match level.generate(self.world.rules().clone()) {
                Ok(world) => self.apply_setup_preview(world, revision),
                Err(error) => {
                    self.failed_preview_revision = Some(revision);
                    self.play_when_preview_ready = false;
                    self.level_setup.error = Some(error.to_string());
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_level_file(&mut self) {
        if let Some(job) = &self.level_reload_job {
            if let Ok(result) = job.receiver.try_recv() {
                let job = self.level_reload_job.take().expect("reload job exists");
                match result {
                    Ok(world) => {
                        self.install_world(world);
                        self.level = job.level;
                        self.level_file_modified = Some(job.modified);
                        self.reset_simulation_clock();
                        self.running = true;
                    }
                    Err(_) => self.level_file_modified = Some(job.modified),
                }
            }
        }
        if self.next_level_file_check > Instant::now() {
            return;
        }
        self.next_level_file_check = Instant::now() + std::time::Duration::from_millis(500);
        let Some(path) = &self.level_file else { return };
        let Some(modified) = file_modified(path) else {
            return;
        };
        if self.level_file_modified == Some(modified) {
            return;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(level) = Level::load(&text) else {
            return;
        };
        if self.level_setup.open {
            self.level_file_modified = Some(modified);
            self.level_setup.import_text(text, self.world.rules());
            return;
        }
        if self.level_reload_job.is_some() {
            return;
        }
        let rules = self.world.rules().clone();
        let worker_level = level.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(worker_level.generate(rules));
        });
        self.level_reload_job = Some(LevelReloadJob {
            level,
            modified,
            receiver,
        });
    }

    fn wildfire(&mut self) {
        let level =
            Preset::DryForest.level(self.world.rules(), self.world.width(), self.world.height());
        self.setup_action(SetupAction::Play(level));
    }

    fn step_world_with_budget(&mut self, budget: usize) -> usize {
        let pass_limit = self
            .world
            .width()
            .saturating_mul(self.world.height())
            .max(1);
        let mut remaining = budget;
        let mut completed = 0;
        while remaining > 0 {
            let used = self.world.step(remaining.min(pass_limit));
            if used == 0 {
                break;
            }
            remaining -= used;
            completed += used;
        }
        completed
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn capture_frame(&mut self, ctx: &egui::Context) {
        if let Some(image) = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        }) {
            if self.world.pending_dirty_regions() != 0 {
                self.capture_requested = false;
                // A scripted span shot must not be dropped: ask again once the world is clean.
                if self.span_shot_path.is_some() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                        egui::UserData::default(),
                    ));
                }
                return;
            }
            if self.span_shot_path.is_some() || self.capture.is_some() {
                let path = self
                    .span_shot_path
                    .clone()
                    .or_else(|| {
                        self.capture
                            .as_ref()
                            .and_then(|options| options.output.clone())
                    })
                    .expect("capture output was validated");
                let rgba: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                image::save_buffer(
                    &path,
                    &rgba,
                    image.size[0] as u32,
                    image.size[1] as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap_or_else(|error| {
                    panic!("Cannot save screenshot {}: {error}", path.display())
                });
                println!(
                    "SCREENSHOT_CAPTURED path={} frames={} sim_seconds={:.3} world={}x{} chunk_origin={} camera_x={:.1} dormant_chunks={}",
                    path.display(),
                    self.capture_frames,
                    self.sim_seconds,
                    self.world.width(),
                    self.world.height(),
                    self.world.chunk_origin(),
                    self.camera_x,
                    self.world.dormant_chunk_count()
                );
                if self.span_shot_path.take().is_some() {
                    return;
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }
        let Some(options) = &self.capture else {
            return;
        };
        self.capture_frames += 1;
        let measured = options
            .measure_real_seconds
            .is_some_and(|seconds| self.measure_elapsed >= seconds);
        if measured {
            self.running = false;
            if !self.measure_reported {
                let elapsed = self.measure_elapsed.max(f64::EPSILON);
                println!(
                    "NATIVE_SPEED_MEASUREMENT requested={:.1}x elapsed_real_s={:.3} sim_seconds={:.3} achieved={:.3} frame_avg_ms={:.2} frame_worst_ms={:.2} frames={} pending_work={} dropped_work={:.0}",
                    self.measure_requested_speed,
                    self.measure_elapsed,
                    self.sim_seconds,
                    self.sim_seconds / elapsed,
                    self.measure_frame_total_ms / self.measure_frames.max(1) as f64,
                    self.measure_frame_max_ms,
                    self.measure_frames,
                    self.work.pending.ceil() as usize,
                    self.work.dropped
                );
                self.measure_reported = true;
            }
            if options.output.is_none() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }
        if measured
            || options
                .after_frames
                .is_some_and(|frames| self.capture_frames >= frames)
            || options
                .at_seconds
                .is_some_and(|seconds| self.sim_seconds >= seconds as f64)
        {
            // Stop at the requested measurement or simulation frame and let the
            // bounded texture uploader drain before asking eframe for a screenshot.
            self.running = false;
            if !self.capture_requested && self.world.pending_dirty_regions() == 0 {
                if options.output.is_some() {
                    self.capture_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                        egui::UserData::default(),
                    ));
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn simulate_frame(&mut self, ctx: &egui::Context, dt: f32, wall_dt: f32) {
        if self.stress_play && self.stress_frames < 900 {
            let frame_index = self.stress_frames;
            let previous_world_width = self.world.width();
            let water = self.world.rules().id("water").unwrap_or(0);
            let fire = self.world.rules().id("fire").unwrap_or(0);
            for n in 0..32 {
                let margin = (self.world.width() / 4).min(20);
                let x =
                    margin + (self.stress_frames * 32 + n * 7) % (self.world.width() - 2 * margin);
                self.world.paint(
                    x,
                    self.world.height() * 2 / 3,
                    if n % 4 == 0 { fire } else { water },
                );
            }
            if self.level.mode == LevelMode::Endless && self.camera_initialized {
                let pan_started = Instant::now();
                self.pan_camera(3.0, self.camera_view_width);
                self.stress_max_pan_ms = self
                    .stress_max_pan_ms
                    .max(pan_started.elapsed().as_secs_f32() * 1000.0);
            }
            self.stress_frames += 1;
            let frame_ms = wall_dt * 1000.0;
            self.stress_total_ms += f64::from(frame_ms);
            self.stress_max_ms = self.stress_max_ms.max(frame_ms);
            self.stress_frame_intervals_ms.push(frame_ms);
            if frame_ms > 33.3 {
                println!(
                    "NATIVE_SMOOTH_PLAY_SLOW_FRAME frame={} elapsed_s={:.3} interval_ms={:.2} world={}x{} width_changed={}",
                    frame_index + 1,
                    self.stress_total_ms / 1000.0,
                    frame_ms,
                    self.world.width(),
                    self.world.height(),
                    self.world.width() != previous_world_width
                );
            }
            self.stress_slow_frames += usize::from(frame_ms > 33.3);
            if self.stress_frames == 900 {
                let mut sorted_intervals = self.stress_frame_intervals_ms.clone();
                sorted_intervals.sort_by(f32::total_cmp);
                let p99_index = (sorted_intervals.len() * 99 / 100).min(sorted_intervals.len() - 1);
                let mut steady_intervals = self
                    .stress_frame_intervals_ms
                    .iter()
                    .skip(SMOOTH_PLAY_WARMUP_FRAMES)
                    .copied()
                    .collect::<Vec<_>>();
                steady_intervals.sort_by(f32::total_cmp);
                let steady_p99_index =
                    (steady_intervals.len() * 99 / 100).min(steady_intervals.len() - 1);
                let startup_worst_ms = self
                    .stress_frame_intervals_ms
                    .iter()
                    .take(SMOOTH_PLAY_WARMUP_FRAMES)
                    .copied()
                    .fold(0.0_f32, f32::max);
                let steady_avg_ms = steady_intervals
                    .iter()
                    .map(|ms| f64::from(*ms))
                    .sum::<f64>()
                    / steady_intervals.len() as f64;
                println!(
                    "NATIVE_SMOOTH_PLAY world={}x{} frames={} warmup_frames={} max_work_per_frame={} peak_carry_work={} dirty_region_cap={} max_dirty_regions_uploaded={} avg_frame_ms={:.2} p99_frame_ms={:.2} worst_frame_ms={:.2} startup_worst_ms={:.2} steady_avg_frame_ms={:.2} steady_p99_frame_ms={:.2} steady_worst_frame_ms={:.2} slow_frames_33ms={} max_pan_ms={:.2} max_extend_ms={:.2} max_texture_ms={:.2} max_upload_ms={:.2} max_pending_dirty_regions={} max_draw_ms={:.2} max_step_ms={:.2} max_update_work_ms={:.2}",
                    self.world.width(),
                    self.world.height(),
                    self.stress_frames,
                    SMOOTH_PLAY_WARMUP_FRAMES,
                    MAX_WORK_PER_FRAME,
                    self.peak_carry_work,
                    MAX_TEXTURE_REGIONS_PER_FRAME,
                    self.max_dirty_regions_uploaded,
                    self.stress_total_ms / self.stress_frames as f64,
                    sorted_intervals[p99_index],
                    self.stress_max_ms,
                    startup_worst_ms,
                    steady_avg_ms,
                    steady_intervals[steady_p99_index],
                    steady_intervals[steady_intervals.len() - 1],
                    self.stress_slow_frames,
                    self.stress_max_pan_ms,
                    self.stress_max_extend_ms,
                    self.stress_max_texture_ms,
                    self.stress_max_upload_ms,
                    self.stress_max_pending_dirty_regions,
                    self.stress_max_draw_ms,
                    self.stress_max_step_ms,
                    self.stress_max_update_work_ms
                );
                #[cfg(target_arch = "wasm32")]
                web::publish_profile(self);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        self.work.trim_to_speed(self.speed);
        if self.running {
            self.work.accrue(dt, self.speed);
            #[cfg(not(target_arch = "wasm32"))]
            if self
                .capture
                .as_ref()
                .is_some_and(|capture| capture.measure_real_seconds.is_some())
            {
                let frame_ms = wall_dt.max(0.0) * 1000.0;
                self.measure_elapsed += wall_dt.max(0.0) as f64;
                self.measure_frame_total_ms += frame_ms as f64;
                self.measure_frame_max_ms = self.measure_frame_max_ms.max(frame_ms);
                self.measure_frames += 1;
            }
        }
        self.peak_carry_work = self.peak_carry_work.max(self.work.pending.ceil() as usize);
        #[cfg(not(target_arch = "wasm32"))]
        let fixed_budget = self.stress_play
            || self.capture.as_ref().is_some_and(|capture| {
                capture.after_frames.is_some() || capture.at_seconds.is_some()
            });
        #[cfg(target_arch = "wasm32")]
        let fixed_budget = self.stress_play;
        let budget = self.work.take(self.adaptive_budget.limit(fixed_budget));
        if budget > 0 {
            let previous_cloud_second = self.sim_seconds.floor();
            let step_started = Instant::now();
            let completed = self.step_world_with_budget(budget);
            let step_ms = step_started.elapsed().as_secs_f32() * 1000.0;
            if self.stress_play {
                self.stress_max_step_ms = self.stress_max_step_ms.max(step_ms);
            }
            if !fixed_budget {
                self.adaptive_budget.observe(completed, f64::from(step_ms));
            }
            let completed_seconds = completed as f64 / WORK_UNITS_PER_SECOND;
            self.sim_seconds += completed_seconds;
            if wall_dt > 0.0 {
                let frame_rate = completed_seconds / wall_dt as f64;
                self.sim_rate = self.sim_rate * 0.8 + frame_rate * 0.2;
            }
            if self.sim_seconds.floor() > previous_cloud_second {
                // Cloud noise and weather fade depend on simulation time. Only visible
                // columns need an immediate refresh; panning dirties the newly visible span.
                self.mark_visible_world_dirty();
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn file_modified(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

fn seed_smooth_play_boats(world: &mut World) {
    let water = world.rules().id("water").unwrap_or(0);
    let stone = world.rules().id("stone").unwrap_or(0);
    let y = world.height() / 3;
    for center_x in [world.width() / 4, world.width() / 2, world.width() * 3 / 4] {
        for dx in -7isize..=7 {
            let Some(x) = center_x
                .checked_add_signed(dx)
                .filter(|x| *x < world.width())
            else {
                continue;
            };
            for dy in 1..=2 {
                if y + dy < world.height() {
                    world.paint(x, y + dy, water);
                }
            }
            if y + 3 < world.height() {
                world.paint(x, y + 3, stone);
            }
        }
        world.place_boat(center_x, y);
    }
}

impl eframe::App for Tinderbox {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = frame;
        #[cfg(not(target_arch = "wasm32"))]
        let mut now = Instant::now();
        #[cfg(target_arch = "wasm32")]
        let now = Instant::now();
        #[cfg(not(target_arch = "wasm32"))]
        if self.stress_play
            && let Some(wait) =
                MIN_SMOOTH_PLAY_FRAME_INTERVAL.checked_sub(now.duration_since(self.last_frame))
        {
            std::thread::sleep(wait);
            now = Instant::now();
        }
        let wall_dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        let update_work_started = Instant::now();
        self.frame_ms = self.frame_ms * 0.9 + wall_dt * 100.0;
        let fixed_step = self.fixed_step_simulation();
        let dt = simulation_frame_dt(wall_dt, fixed_step);
        self.simulate_frame(ctx, dt, wall_dt);
        input::handle_keyboard(self, ctx);
        // Interactive play and smooth-play use native vsync. Captures request frames
        // immediately but advance by deterministic fixed simulation steps.
        ctx.request_repaint();

        let toolbar_panel = egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_enabled_ui(!self.level_setup.open, |ui| panels::toolbar::show(self, ui));
        });
        panels::toolbar::record_test_panel_bounds("toolbar", toolbar_panel.response.rect);
        let compact_palette = ctx.screen_rect().width() <= 480.0;
        let palette_panel = egui::SidePanel::left("palette")
            .frame(egui::Frame::NONE)
            .resizable(true)
            .default_width(if compact_palette { 60.0 } else { 168.0 })
            .width_range(if compact_palette {
                52.0..=62.0
            } else {
                148.0..=260.0
            })
            .show(ctx, |ui| {
                ui.add_enabled_ui(!self.level_setup.open, |ui| panels::palette::show(self, ui));
            });
        panels::toolbar::record_test_panel_bounds("palette", palette_panel.response.rect);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                ui.add_enabled_ui(!self.level_setup.open, |ui| panels::world::show(self, ui));
            });
        if self.rules_panel.open && !self.level_setup.open {
            let selected_name = self
                .world
                .rules()
                .elements
                .get(self.selected)
                .map(|e| e.name.clone());
            self.rules_panel.show(ctx, &mut self.world);
            self.selected = selected_name
                .as_deref()
                .and_then(|name| self.world.rules().id(name))
                .unwrap_or_else(|| self.world.rules().id("air").unwrap_or(0));
        }
        let setup_was_open = self.level_setup.open;
        if let Some(action) = self.level_setup.show(ctx, self.world.rules()) {
            self.setup_action(action);
        }
        self.update_setup_preview();
        if setup_was_open && !self.level_setup.open {
            input::clear_level_setup_text_focus(ctx);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(path) = self.level_setup.take_loaded_path() {
                self.level_file_modified = file_modified(&path);
                self.level_file = Some(path);
            }
            self.poll_level_file();
            self.capture_frame(ctx);
            self.drive_span_evidence(ctx);
        }
        #[cfg(target_arch = "wasm32")]
        poll_browser_files(ctx, frame, self);
        if self.stress_play {
            self.stress_max_update_work_ms = self
                .stress_max_update_work_ms
                .max(update_work_started.elapsed().as_secs_f32() * 1000.0);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn poll_browser_files(ctx: &egui::Context, frame: &mut eframe::Frame, app: &mut Tinderbox) {
    if let Some((key, text)) = files::poll(ctx) {
        if key == "level" || Level::load(&text).is_ok() {
            if !app.level_setup.open {
                app.open_level_setup();
            }
            app.level_setup.import_text(text.clone(), app.world.rules());
            if app.level_setup.error.is_none() {
                match files::remember("level", &text) {
                    Ok(()) => {
                        files::set_notice("Loaded level settings. Press Play level to start.")
                    }
                    Err(err) => files::set_notice(err),
                }
            }
        } else {
            let selected_name = app
                .world
                .rules()
                .elements
                .get(app.selected)
                .map(|e| e.name.clone());
            match app.world.load_rules(&text) {
                Ok(()) => {
                    app.selected = selected_name
                        .as_deref()
                        .and_then(|name| app.world.rules().id(name))
                        .unwrap_or_else(|| app.world.rules().id("air").unwrap_or(0));
                    match files::remember(&key, &text) {
                        Ok(()) => files::set_notice(format!(
                            "Applied and stored {key} ({} bytes). The world keeps running.",
                            text.len()
                        )),
                        Err(err) => {
                            files::set_notice(format!("Rules applied, but not stored: {err}"))
                        }
                    }
                }
                Err(err) => files::set_notice(format!("{err}. Current rules are unchanged.")),
            }
        }
    }
    web::publish_frame(app, frame);
}

#[cfg(not(target_arch = "wasm32"))]
fn smooth_play_requested() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::args().any(|arg| arg == "--smooth-play")
    }
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) fn start_web() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn native_args_from(args: impl IntoIterator<Item = String>) -> Result<NativeArgs, String> {
    let mut args = args.into_iter();
    let mut parsed = NativeArgs {
        speed: 1.0,
        ..NativeArgs::default()
    };
    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "--level" => Some((
                "level",
                args.next().ok_or("--level requires a recipe path")?,
            )),
            "--screenshot" => Some((
                "screenshot",
                args.next().ok_or("--screenshot requires a PNG path")?,
            )),
            "--camera-x" => Some((
                "camera-x",
                args.next().ok_or("--camera-x requires a cell coordinate")?,
            )),
            "--capture-after" => Some((
                "frames",
                args.next()
                    .ok_or("--capture-after requires a frame count")?,
            )),
            "--capture-at-seconds" => Some((
                "seconds",
                args.next()
                    .ok_or("--capture-at-seconds requires a second count")?,
            )),
            "--measure-real-seconds" => Some((
                "measure_seconds",
                args.next()
                    .ok_or("--measure-real-seconds requires a duration")?,
            )),
            "--inspect-at" => Some((
                "inspect_at",
                args.next()
                    .ok_or("--inspect-at requires X,Y cell coordinates")?,
            )),
            "--drop-boat-at" => Some((
                "drop_boat_at",
                args.next()
                    .ok_or("--drop-boat-at requires X,Y cell coordinates")?,
            )),
            "--speed" => Some(("speed", args.next().ok_or("--speed requires a value")?)),
            "--span-evidence" => Some((
                "span_evidence",
                args.next()
                    .ok_or("--span-evidence requires an output directory")?,
            )),
            _ => None,
        };
        let Some((flag, value)) = value else {
            continue;
        };
        match flag {
            "level" => parsed.level_path = Some(PathBuf::from(value)),
            "camera-x" => {
                parsed.camera_x = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| "--camera-x must be a non-negative world-cell coordinate")?,
                );
            }
            "screenshot" => parsed.capture.output = Some(PathBuf::from(value)),
            "frames" => {
                let count = value
                    .parse::<usize>()
                    .map_err(|_| "--capture-after must be a non-negative integer")?;
                parsed.capture.after_frames = Some(count);
            }
            "seconds" => {
                let count = value
                    .parse::<u64>()
                    .map_err(|_| "--capture-at-seconds must be a non-negative integer")?;
                parsed.capture.at_seconds = Some(count);
            }
            "measure_seconds" => {
                let seconds = value
                    .parse::<f64>()
                    .map_err(|_| "--measure-real-seconds must be between 0 and 3600")?;
                if !seconds.is_finite() || !(0.0..=3600.0).contains(&seconds) || seconds == 0.0 {
                    return Err("--measure-real-seconds must be between 0 and 3600".into());
                }
                parsed.capture.measure_real_seconds = Some(seconds);
            }
            "inspect_at" => {
                let (x, y) = value
                    .split_once(',')
                    .and_then(|(x, y)| Some((x.parse::<usize>().ok()?, y.parse::<usize>().ok()?)))
                    .ok_or("--inspect-at must use non-negative X,Y cell coordinates")?;
                parsed.capture.inspect_at = Some((x, y));
            }
            "drop_boat_at" => {
                let (x, y) = value
                    .split_once(',')
                    .and_then(|(x, y)| Some((x.parse::<usize>().ok()?, y.parse::<usize>().ok()?)))
                    .ok_or("--drop-boat-at must use non-negative X,Y cell coordinates")?;
                parsed.drop_boat_at = Some((x, y));
            }
            "speed" => {
                let speed = value
                    .parse::<f32>()
                    .map_err(|_| "--speed must be between 0.25 and 10")?;
                if !speed.is_finite() || !(0.25..=10.0).contains(&speed) {
                    return Err("--speed must be between 0.25 and 10".into());
                }
                parsed.speed = speed;
            }
            "span_evidence" => parsed.span_evidence = Some(PathBuf::from(value)),
            _ => unreachable!(),
        }
    }
    let fixed_capture =
        parsed.capture.after_frames.is_some() || parsed.capture.at_seconds.is_some();
    if parsed.capture.after_frames.is_some() && parsed.capture.at_seconds.is_some() {
        return Err("Choose either --capture-after or --capture-at-seconds".into());
    }
    if fixed_capture && parsed.capture.measure_real_seconds.is_some() {
        return Err("Choose either fixed-step capture or --measure-real-seconds".into());
    }
    if parsed.capture.output.is_some()
        && !fixed_capture
        && parsed.capture.measure_real_seconds.is_none()
        || parsed.capture.output.is_none() && fixed_capture
    {
        return Err("--screenshot must be paired with a capture or measurement time".into());
    }
    if parsed
        .capture
        .output
        .as_ref()
        .is_some_and(|path| path.extension().is_none_or(|ext| ext != "png"))
    {
        return Err("--screenshot path must end in .png".into());
    }
    if fixed_capture && parsed.level_path.is_none() {
        return Err("A fixed-step capture requires a level recipe supplied with --level".into());
    }
    Ok(parsed)
}

#[cfg(not(target_arch = "wasm32"))]
fn native_args() -> Result<NativeArgs, String> {
    native_args_from(std::env::args().skip(1))
}

#[cfg(not(target_arch = "wasm32"))]
fn requested_world_size() -> (usize, usize) {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = if arg == "--world-size" {
            args.next()
        } else {
            arg.strip_prefix("--world-size=").map(str::to_owned)
        };
        let Some(value) = value else {
            continue;
        };
        let parsed = value.split_once('x').and_then(|(width, height)| {
            Some((width.parse::<usize>().ok()?, height.parse::<usize>().ok()?))
        });
        if let Some((width, height)) = parsed
            && width >= 128
            && height >= 96
            && width <= MAX_LEVEL_DIMENSION
            && height <= MAX_LEVEL_DIMENSION
            && width
                .checked_mul(height)
                .is_some_and(|cells| cells <= MAX_LEVEL_CELLS)
        {
            return (width, height);
        }
        eprintln!(
            "Ignoring invalid --world-size={value}; expected WIDTHxHEIGHT within 128..={MAX_LEVEL_DIMENSION} by 96..={MAX_LEVEL_DIMENSION} (max {MAX_LEVEL_CELLS} cells)"
        );
    }
    (DEFAULT_WIDTH, DEFAULT_HEIGHT)
}

#[cfg(not(target_arch = "wasm32"))]
fn background_window_requested(args: impl IntoIterator<Item = String>) -> bool {
    let args = args.into_iter().collect::<Vec<_>>();
    native_args_from(args).is_ok_and(|parsed| {
        parsed.capture.after_frames.is_some()
            || parsed.capture.at_seconds.is_some()
            || parsed.span_evidence.is_some()
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        vsync: true,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([720.0, 520.0])
            .with_active(!background_window_requested(std::env::args().skip(1))),
        ..Default::default()
    };
    eframe::run_native(
        "Tinderbox",
        options,
        Box::new(|cc| Ok(Box::new(Tinderbox::new(cc)))),
    )
}
