use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use std::time::Instant;
use tinderbox_sim::{DEFAULT_RULES, Rules, World};

const WIDTH: usize = 192;
const HEIGHT: usize = 120;
const WORK_PER_FRAME: usize = 12_000;

fn main() -> eframe::Result {
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

struct Tinderbox {
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
        let world =
            World::new(WIDTH, HEIGHT, 7, rules).expect("default world dimensions are valid");
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
            stress_play: std::env::args().any(|arg| arg == "--smooth-play"),
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
        let tree = self.world.rules.id("tree").unwrap_or(0);
        let fire = self.world.rules.id("fire").unwrap_or(0);
        let water = self.world.rules.id("water").unwrap_or(0);
        let stone = self.world.rules.id("stone").unwrap_or(0);
        let floor = self.world.height - 12;
        for y in floor - 32..floor {
            for x in 18..self.world.width - 18 {
                if (x * 7 + y * 11) % 9 < 7 {
                    self.world.paint(x, y, tree);
                }
            }
        }
        for x in 30..self.world.width - 30 {
            self.world.paint(x, floor, stone);
        }
        for x in 68..self.world.width - 70 {
            self.world.paint(x, floor - 16, fire);
        }
        for x in 25..32 {
            self.world.paint(x, floor - 1, water);
        }
        self.running = true;
    }
    fn paint_at(&mut self, pos: Pos2, rect: Rect, element: usize) {
        let x = ((pos.x - rect.left()) / rect.width() * self.world.width as f32) as usize;
        let y = ((pos.y - rect.top()) / rect.height() * self.world.height as f32) as usize;
        let size = self.brush as isize;
        let start = -(size / 2);
        for dy in start..start + size {
            for dx in start..start + size {
                let px = x as isize + dx;
                let py = y as isize + dy;
                if px >= 0 && py >= 0 {
                    self.world.paint(px as usize, py as usize, element);
                }
            }
        }
    }
}
impl eframe::App for Tinderbox {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.frame_ms = self.frame_ms * 0.9 + dt * 100.0;
        if self.stress_play && self.stress_frames < 900 {
            let water = self.world.rules.id("water").unwrap_or(0);
            let fire = self.world.rules.id("fire").unwrap_or(0);
            for n in 0..32 {
                let x = 20 + (self.stress_frames * 32 + n * 7) % (WIDTH - 40);
                self.world
                    .paint(x, HEIGHT - 28, if n % 4 == 0 { fire } else { water });
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
        ctx.request_repaint();
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(if self.running {
                        "⏸ Pause"
                    } else {
                        "▶ Play"
                    })
                    .clicked()
                {
                    self.running = !self.running;
                }
                if ui.button("Step").clicked() {
                    self.world.step(WORK_PER_FRAME);
                    self.sim_seconds += 1.0 / 60.0;
                }
                if ui.button("Reset").clicked() {
                    self.reset();
                }
                if ui.button("Wildfire").clicked() {
                    self.wildfire();
                }
                ui.separator();
                ui.label("Brush");
                ui.add(egui::Slider::new(&mut self.brush, 1..=16).suffix(" cells"));
                ui.label("Speed");
                ui.add(egui::Slider::new(&mut self.speed, 0.25..=4.0).suffix("×"));
                ui.checkbox(&mut self.show_hud, "HUD");
            });
        });
        egui::SidePanel::left("palette").resizable(false).default_width(142.0).show(ctx, |ui| {
            ui.heading("Elements"); ui.separator();
            for (id, element) in self.world.rules.elements.iter().enumerate() {
                let chosen=id==self.selected;
                let color=element.color;
                let response=ui.selectable_label(chosen, format!("  {}",element.name));
                ui.painter().circle_filled(response.rect.left_center()+Vec2::new(9.0,0.0),5.0,Color32::from_rgb(color[0],color[1],color[2]));
                if response.clicked() { self.selected=id; }
            }
            ui.separator();
            if ui.button("Ignite (I)").clicked() { self.selected=self.world.rules.id("fire").unwrap_or(self.selected); }
            if ui.button("Erase (E)").clicked() { self.selected=self.world.rules.id("air").unwrap_or(self.selected); }
            ui.small("Left-drag paints\nRight-click ignites\nSpace play/pause\n. single step\nR reset\nH HUD");
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_size();
            let cell_size = (available.x / WIDTH as f32)
                .min(available.y / HEIGHT as f32)
                .max(1.0);
            let size = Vec2::new(cell_size * WIDTH as f32, cell_size * HEIGHT as f32);
            let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 2.0, Color32::from_rgb(18, 22, 34));
            for (i, cell) in self.world.cells.iter().enumerate() {
                if cell.element == 0 {
                    continue;
                }
                let color = self.world.rules.elements[cell.element].color;
                let x = i % WIDTH;
                let y = i / WIDTH;
                let min = rect.min + Vec2::new(x as f32 * cell_size, y as f32 * cell_size);
                painter.rect_filled(
                    Rect::from_min_size(min, Vec2::splat(cell_size + 0.35)),
                    0.0,
                    Color32::from_rgb(color[0], color[1], color[2]),
                );
            }
            if response.hovered() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if response.dragged_by(egui::PointerButton::Primary)
                        || response.clicked_by(egui::PointerButton::Primary)
                    {
                        self.paint_at(pos, rect, self.selected);
                    }
                    if response.clicked_by(egui::PointerButton::Secondary) {
                        let fire = self.world.rules.id("fire").unwrap_or(self.selected);
                        self.paint_at(pos, rect, fire);
                    }
                }
            }
            let input = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::Space),
                    i.key_pressed(egui::Key::Period),
                    i.key_pressed(egui::Key::R),
                    i.key_pressed(egui::Key::H),
                    i.key_pressed(egui::Key::E),
                    i.key_pressed(egui::Key::I),
                )
            });
            if input.0 {
                self.running = !self.running;
            }
            if input.1 {
                self.world.step(WORK_PER_FRAME);
            }
            if input.2 {
                self.reset();
            }
            if input.3 {
                self.show_hud = !self.show_hud;
            }
            if input.4 {
                self.selected = self.world.rules.id("air").unwrap_or(self.selected);
            }
            if input.5 {
                self.selected = self.world.rules.id("fire").unwrap_or(self.selected);
            }
            if self.show_hud {
                painter.text(
                    rect.left_top() + Vec2::splat(8.0),
                    egui::Align2::LEFT_TOP,
                    format!(
                        "{:.1} ms/frame   sim {:.1}s   pending {}",
                        self.frame_ms, self.sim_seconds, self.world.pending
                    ),
                    egui::FontId::monospace(13.0),
                    Color32::WHITE,
                );
            }
        });
    }
}
