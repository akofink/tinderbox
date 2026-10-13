//! Play controls, brush controls, speed, reset, and scene setup.
use crate::app::Tinderbox;
use eframe::egui;

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(if app.running { "⏸ Pause" } else { "▶ Play" })
            .clicked()
        {
            app.running = !app.running;
        }
        if ui.button("Step").clicked() {
            app.world.step(super::super::WORK_PER_FRAME);
            app.sim_seconds += 1.0 / 60.0;
        }
        if ui.button("Reset").clicked() {
            app.reset();
        }
        if ui.button("Wildfire").clicked() {
            app.wildfire();
        }
        ui.separator();
        ui.label("Brush");
        ui.add(egui::Slider::new(&mut app.brush, 1..=16).suffix(" cells"));
        ui.label("Speed");
        ui.add(egui::Slider::new(&mut app.speed, 0.25..=4.0).suffix("×"));
        ui.checkbox(&mut app.show_hud, "HUD");
    });
}
