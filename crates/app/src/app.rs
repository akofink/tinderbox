//! Application loop and shared sandbox state. Native and wasm share this UI.
pub(crate) mod files;
mod input;
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
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use tinderbox_sim::level::{MAX_LEVEL_CELLS, MAX_LEVEL_DIMENSION};
use tinderbox_sim::{
    DEFAULT_RULES, Rules, World,
    level::{Level, LevelError, Preset},
};
use web_time::Instant;

const DEFAULT_WIDTH: usize = 400;
const DEFAULT_HEIGHT: usize = 250;
const WORK_PER_FRAME: usize = 12_000;
const WORK_UNITS_PER_SECOND: f64 = (WORK_PER_FRAME * 60) as f64;
const MAX_WORK_PER_FRAME: usize = WORK_PER_FRAME * 4;
const MAX_CARRY_FRAMES: usize = 8;
pub(super) const MAX_TEXTURE_REGIONS_PER_FRAME: usize = 32;

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
}
impl WorkAccumulator {
    fn accrue(&mut self, dt: f32, speed: f32) {
        let due = dt.max(0.0) as f64 * WORK_UNITS_PER_SECOND * speed.clamp(0.0, 4.0) as f64;
        self.add(due);
    }

    fn add(&mut self, units: f64) {
        let carry_cap = (MAX_WORK_PER_FRAME * MAX_CARRY_FRAMES) as f64;
        self.pending = (self.pending + units).min(carry_cap);
    }

    fn take(&mut self) -> usize {
        let budget = (self.pending.floor() as usize).min(MAX_WORK_PER_FRAME);
        self.pending -= budget as f64;
        budget
    }
}

pub(super) struct Tinderbox {
    world: World,
    world_texture: eframe::egui::TextureHandle,
    selected: usize,
    brush: usize,
    gust_mode: bool,
    overlay: Overlay,
    running: bool,
    speed: f32,
    show_hud: bool,
    rules_panel: RulesPanel,
    level: Level,
    level_setup: LevelSetup,
    return_running: bool,
    sim_seconds: f64,
    work: WorkAccumulator,
    peak_carry_work: usize,
    max_dirty_regions_uploaded: usize,
    last_frame: Instant,
    frame_ms: f32,
    stress_play: bool,
    stress_frames: usize,
    stress_total_ms: f64,
    stress_max_ms: f32,
    #[cfg(not(target_arch = "wasm32"))]
    capture: Option<CaptureOptions>,
    #[cfg(not(target_arch = "wasm32"))]
    capture_frames: usize,
    #[cfg(not(target_arch = "wasm32"))]
    capture_requested: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct CaptureOptions {
    output: Option<PathBuf>,
    after_frames: Option<usize>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct NativeArgs {
    level_path: Option<PathBuf>,
    capture: CaptureOptions,
}
impl Tinderbox {
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
        let (width, height) = level
            .as_ref()
            .map_or_else(requested_world_size, |level| (level.width, level.height));
        let mut app =
            Self::new_with_context(cc.egui_ctx.clone(), width, height, smooth_play_requested());
        if let Some(level) = level {
            app.set_level(level)
                .expect("validated level recipe must generate");
            app.level_setup.open = false;
            app.running = args.capture.after_frames != Some(0);
        }
        app.capture = (args.capture.after_frames.is_some()).then_some(args.capture);
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
        let level = Preset::DryForest.level(&rules, width, height);
        let world = level.generate(rules).expect("default level is valid");
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
        Self {
            world,
            world_texture,
            selected,
            brush: 3,
            gust_mode: false,
            overlay: Overlay::None,
            running: stress_play,
            speed: 1.0,
            show_hud: true,
            rules_panel: RulesPanel::default(),
            level,
            level_setup,
            return_running: false,
            sim_seconds: 0.0,
            work: WorkAccumulator::default(),
            peak_carry_work: 0,
            max_dirty_regions_uploaded: 0,
            last_frame: Instant::now(),
            frame_ms: 0.0,
            stress_play,
            stress_frames: 0,
            stress_total_ms: 0.0,
            stress_max_ms: 0.0,
            #[cfg(not(target_arch = "wasm32"))]
            capture: None,
            #[cfg(not(target_arch = "wasm32"))]
            capture_frames: 0,
            #[cfg(not(target_arch = "wasm32"))]
            capture_requested: false,
        }
    }

    fn set_level(&mut self, level: Level) -> Result<(), LevelError> {
        let world = level.generate(self.world.rules().clone())?;
        let air = world.rules().id("air").unwrap_or(0);
        let air_color = world.rules().elements[air].color;
        self.world_texture.set(
            egui::ColorImage::new(
                [world.width(), world.height()],
                egui::Color32::from_rgb(air_color[0], air_color[1], air_color[2]),
            ),
            egui::TextureOptions::NEAREST,
        );
        self.world = world;
        self.world.mark_all_dirty();
        self.level = level;
        self.sim_seconds = 0.0;
        self.work = WorkAccumulator::default();
        self.peak_carry_work = 0;
        self.max_dirty_regions_uploaded = 0;
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
    }

    fn setup_action(&mut self, action: SetupAction) {
        match action {
            SetupAction::Play(level) => match self.set_level(level) {
                Ok(()) => {
                    self.level_setup.open = false;
                    self.running = true;
                }
                Err(error) => self.level_setup.error = Some(error.to_string()),
            },
            SetupAction::Cancel => {
                self.level_setup.open = false;
                self.running = self.return_running;
            }
        }
    }

    fn wildfire(&mut self) {
        let level =
            Preset::DryForest.level(self.world.rules(), self.world.width(), self.world.height());
        self.setup_action(SetupAction::Play(level));
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
                return;
            }
            if let Some(options) = &self.capture {
                let path = options
                    .output
                    .as_ref()
                    .expect("capture output was validated");
                let rgba: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                image::save_buffer(
                    path,
                    &rgba,
                    image.size[0] as u32,
                    image.size[1] as u32,
                    image::ColorType::Rgba8,
                )
                .unwrap_or_else(|error| {
                    panic!("Cannot save screenshot {}: {error}", path.display())
                });
                println!(
                    "SCREENSHOT_CAPTURED path={} frames={}",
                    path.display(),
                    self.capture_frames
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }
        let Some(options) = &self.capture else {
            return;
        };
        self.capture_frames += 1;
        if self.capture_frames >= options.after_frames.expect("validated frame count") {
            // Stop at the requested simulation frame and let the capped texture
            // uploader drain every dirty region before asking eframe for a screenshot.
            self.running = false;
            if !self.capture_requested && self.world.pending_dirty_regions() == 0 {
                self.capture_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
        }
    }

    fn simulate_frame(&mut self, ctx: &egui::Context, dt: f32) {
        if self.stress_play && self.stress_frames < 900 {
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
            self.stress_frames += 1;
            self.stress_total_ms += dt as f64 * 1000.0;
            self.stress_max_ms = self.stress_max_ms.max(dt * 1000.0);
            if self.stress_frames == 900 {
                println!(
                    "NATIVE_SMOOTH_PLAY world={}x{} frames={} max_work_per_frame={} peak_carry_work={} dirty_region_cap={} max_dirty_regions_uploaded={} avg_frame_ms={:.2} worst_frame_ms={:.2}",
                    self.world.width(),
                    self.world.height(),
                    self.stress_frames,
                    MAX_WORK_PER_FRAME,
                    self.peak_carry_work,
                    MAX_TEXTURE_REGIONS_PER_FRAME,
                    self.max_dirty_regions_uploaded,
                    self.stress_total_ms / self.stress_frames as f64,
                    self.stress_max_ms
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self.running {
            self.work.accrue(dt, self.speed);
        }
        self.peak_carry_work = self.peak_carry_work.max(self.work.pending.ceil() as usize);
        let budget = self.work.take();
        if budget > 0 {
            let completed = self.world.step(budget);
            self.sim_seconds += completed as f64 / WORK_UNITS_PER_SECOND;
        }
    }
}

impl eframe::App for Tinderbox {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = frame;
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.frame_ms = self.frame_ms * 0.9 + dt * 100.0;
        self.simulate_frame(ctx, dt);
        input::handle_keyboard(self, ctx);
        ctx.request_repaint();

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_enabled_ui(!self.level_setup.open, |ui| panels::toolbar::show(self, ui));
        });
        egui::SidePanel::left("palette")
            .resizable(true)
            .default_width(168.0)
            .min_width(148.0)
            .show(ctx, |ui| {
                ui.add_enabled_ui(!self.level_setup.open, |ui| panels::palette::show(self, ui));
            });
        egui::CentralPanel::default().show(ctx, |ui| {
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
        if let Some(action) = self.level_setup.show(ctx, self.world.rules()) {
            self.setup_action(action);
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.capture_frame(ctx);
        #[cfg(target_arch = "wasm32")]
        poll_browser_files(ctx, frame, self);
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
    web::publish_frame(app.frame_ms, frame);
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
    let mut parsed = NativeArgs::default();
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
            "--capture-after" => Some((
                "frames",
                args.next()
                    .ok_or("--capture-after requires a frame count")?,
            )),
            _ => None,
        };
        let Some((flag, value)) = value else {
            continue;
        };
        match flag {
            "level" => parsed.level_path = Some(PathBuf::from(value)),
            "screenshot" => parsed.capture.output = Some(PathBuf::from(value)),
            "frames" => {
                let count = value
                    .parse::<usize>()
                    .map_err(|_| "--capture-after must be a non-negative integer")?;
                parsed.capture.after_frames = Some(count);
            }
            _ => unreachable!(),
        }
    }
    if parsed.capture.output.is_some() != parsed.capture.after_frames.is_some() {
        return Err("--screenshot and --capture-after must be supplied together".into());
    }
    if parsed
        .capture
        .output
        .as_ref()
        .is_some_and(|path| path.extension().is_none_or(|ext| ext != "png"))
    {
        return Err("--screenshot path must end in .png".into());
    }
    if parsed.capture.after_frames.is_some() && parsed.level_path.is_none() {
        return Err("--capture-after requires a level recipe supplied with --level".into());
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
    args.iter().any(|arg| arg == "--smooth-play")
        || native_args_from(args).is_ok_and(|parsed| parsed.capture.after_frames.is_some())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
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
