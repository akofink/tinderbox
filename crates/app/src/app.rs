//! Application loop and shared sandbox state. Native and wasm share this UI.
mod files;
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
use tinderbox_sim::{
    DEFAULT_RULES, Rules, World,
    level::{Level, LevelError, Preset},
};
use web_time::Instant;

const DEFAULT_WIDTH: usize = 192;
const DEFAULT_HEIGHT: usize = 120;
const WORK_PER_FRAME: usize = 12_000;

pub(super) struct Tinderbox {
    world: World,
    selected: usize,
    brush: usize,
    gust_mode: bool,
    running: bool,
    speed: f32,
    show_hud: bool,
    rules_panel: RulesPanel,
    level: Level,
    level_setup: LevelSetup,
    return_running: bool,
    sim_seconds: f64,
    last_frame: Instant,
    frame_ms: f32,
    stress_play: bool,
    stress_frames: usize,
    stress_total_ms: f64,
    stress_max_ms: f32,
}
impl Tinderbox {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::new_state(smooth_play_requested())
    }

    fn new_state(stress_play: bool) -> Self {
        let rules = Rules::load(DEFAULT_RULES).expect("bundled rules are validated by tests");
        let selected = rules.id("sand").unwrap_or(0);
        let level = Preset::DryForest.level(&rules, DEFAULT_WIDTH, DEFAULT_HEIGHT);
        let world = level.generate(rules).expect("default level is valid");
        let mut level_setup = LevelSetup::new(level.clone());
        level_setup.open = !stress_play;
        Self {
            world,
            selected,
            brush: 3,
            gust_mode: false,
            running: stress_play,
            speed: 1.0,
            show_hud: true,
            rules_panel: RulesPanel::default(),
            level,
            level_setup,
            return_running: false,
            sim_seconds: 0.0,
            last_frame: Instant::now(),
            frame_ms: 0.0,
            stress_play,
            stress_frames: 0,
            stress_total_ms: 0.0,
            stress_max_ms: 0.0,
        }
    }

    fn set_level(&mut self, level: Level) -> Result<(), LevelError> {
        let world = level.generate(self.world.rules().clone())?;
        self.world = world;
        self.level = level;
        self.sim_seconds = 0.0;
        Ok(())
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
                    "NATIVE_SMOOTH_PLAY frames={} avg_frame_ms={:.2} worst_frame_ms={:.2}",
                    self.stress_frames,
                    self.stress_total_ms / self.stress_frames as f64,
                    self.stress_max_ms
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self.running {
            let units = (WORK_PER_FRAME as f32 * self.speed).round() as usize;
            self.world.step(units);
            self.sim_seconds += dt as f64 * self.speed as f64;
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
            .resizable(false)
            .default_width(142.0)
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
pub(super) fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([720.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tinderbox",
        options,
        Box::new(|cc| Ok(Box::new(Tinderbox::new(cc)))),
    )
}
