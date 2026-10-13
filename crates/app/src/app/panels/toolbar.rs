//! Play controls, brush controls, speed, reset, and scene setup.
use crate::app::{Overlay, Tinderbox};
use eframe::egui;

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(if app.running { "Pause" } else { "Play" })
            .clicked()
        {
            app.running = !app.running;
        }
        if ui.button("Step").clicked() {
            app.request_step();
        }
        if ui.button("Reset").clicked() {
            app.reset();
        }
        if ui.button("Level setup").clicked() {
            app.open_level_setup();
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
        let previous_overlay = app.overlay;
        egui::ComboBox::from_label("Overlay")
            .selected_text(match app.overlay {
                Overlay::None => "Off",
                Overlay::Temperature => "Temperature",
                Overlay::Humidity => "Humidity",
                Overlay::Pressure => "Pressure",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.overlay, Overlay::None, "Off");
                ui.selectable_value(&mut app.overlay, Overlay::Temperature, "Temperature");
                ui.selectable_value(&mut app.overlay, Overlay::Humidity, "Humidity");
                ui.selectable_value(&mut app.overlay, Overlay::Pressure, "Pressure");
            });
        if app.overlay != previous_overlay {
            app.world.mark_all_dirty();
        }
        ui.toggle_value(&mut app.rules_panel.open, "Rules");
    });
    ui.horizontal_wrapped(|ui| {
        super::wind::show(app, ui);
    });
}
