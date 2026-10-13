//! Application loop and shared sandbox state. Native and wasm share this UI.
mod files;
mod input;
mod panels;
mod render;
#[cfg(target_arch = "wasm32")]
mod web;

use eframe::egui;
use tinderbox_sim::{DEFAULT_RULES, Rules, World};
use web_time::Instant;

const DEFAULT_WIDTH: usize = 192;
const DEFAULT_HEIGHT: usize = 120;
const WORK_PER_FRAME: usize = 12_000;

pub(super) struct Tinderbox {
    world: World,
    selected: usize,
    brush: usize,
    running: bool,
    speed: f32,
    show_hud: bool,
    seed: u64,
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
        let rules = Rules::load(DEFAULT_RULES).expect("bundled rules are validated by tests");
        let selected = rules.id("sand").unwrap_or(0);
        let world = World::new(DEFAULT_WIDTH, DEFAULT_HEIGHT, 7, rules)
            .expect("default world dimensions are valid");
        let mut app = Self {
            world,
            selected,
            brush: 3,
            running: true,
            speed: 1.0,
            show_hud: true,
            seed: 7,
            sim_seconds: 0.0,
            last_frame: Instant::now(),
            frame_ms: 0.0,
            stress_play: smooth_play_requested(),
            stress_frames: 0,
            stress_total_ms: 0.0,
            stress_max_ms: 0.0,
        };
        app.wildfire();
        app
    }

    fn reset(&mut self) {
        self.seed = self.seed.wrapping_add(1).max(1);
        self.world.reset(self.seed);
        self.sim_seconds = 0.0;
    }

    fn wildfire(&mut self) {
        let rules = self.world.rules();
        let tree = rules.id("tree").unwrap_or(0);
        let fire = rules.id("fire").unwrap_or(0);
        let water = rules.id("water").unwrap_or(0);
        let stone = rules.id("stone").unwrap_or(0);
        let floor = self.world.height() - 12;
        for y in floor - 32..floor {
            for x in 18..self.world.width() - 18 {
                if (x * 7 + y * 11) % 9 < 7 {
                    self.world.paint(x, y, tree);
                }
            }
        }
        for x in 30..self.world.width() - 30 {
            self.world.paint(x, floor, stone);
        }
        for x in 68..self.world.width() - 70 {
            self.world.paint(x, floor - 16, fire);
        }
        for x in 25..32 {
            self.world.paint(x, floor - 1, water);
        }
        self.running = true;
    }

    fn simulate_frame(&mut self, ctx: &egui::Context, dt: f32) {
        if self.stress_play && self.stress_frames < 900 {
            let water = self.world.rules().id("water").unwrap_or(0);
            let fire = self.world.rules().id("fire").unwrap_or(0);
            for n in 0..32 {
                let x = 20 + (self.stress_frames * 32 + n * 7) % (self.world.width() - 40);
                self.world.paint(
                    x,
                    self.world.height() - 28,
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
            panels::toolbar::show(self, ui);
        });
        egui::SidePanel::left("palette")
            .resizable(false)
            .default_width(142.0)
            .show(ctx, |ui| panels::palette::show(self, ui));
        egui::CentralPanel::default().show(ctx, |ui| {
            panels::world::show(self, ui);
        });
        #[cfg(target_arch = "wasm32")]
        poll_browser_files(ctx, frame, self.frame_ms);
    }
}

#[cfg(target_arch = "wasm32")]
fn poll_browser_files(ctx: &egui::Context, frame: &mut eframe::Frame, frame_ms: f32) {
    if let Some((key, text)) = files::poll(ctx) {
        match Rules::load(&text) {
            Ok(_) => match files::remember(&key, &text) {
                Ok(()) => files::set_notice(format!(
                    "Stored {key} ({} bytes) in this browser. The running world is unchanged.",
                    text.len()
                )),
                Err(err) => files::set_notice(err),
            },
            Err(err) => files::set_notice(err.to_string()),
        }
    }
    web::publish_frame(frame_ms, frame);
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
