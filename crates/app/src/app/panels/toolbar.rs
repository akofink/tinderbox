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
        ui.separator();
        ui.label("Speed");
        ui.add(egui::Slider::new(&mut app.speed, 0.25..=10.0).suffix("×"));
        ui.label("Brush");
        ui.add(egui::Slider::new(&mut app.brush, 1..=16).suffix(" cells"));
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("Level setup").clicked() {
            app.open_level_setup();
        }
        if ui.button("Wildfire").clicked() {
            app.wildfire();
        }
        ui.label("Cell size");
        ui.add(egui::Slider::new(&mut app.cell_size, 2.0..=8.0).suffix(" pt"));
        ui.checkbox(&mut app.show_hud, "HUD");
        if ui
            .add_sized(
                egui::vec2(104.0, 44.0),
                egui::Button::new("Inspect (M)").selected(app.inspect_mode),
            )
            .clicked()
        {
            app.inspect_mode = !app.inspect_mode;
        }
        if app.inspect_mode {
            ui.label("Tap a cell; Inspect again to paint");
        }
        let previous_overlay = app.overlay;
        egui::ComboBox::from_label("Overlay")
            .selected_text(match app.overlay {
                Overlay::None => "Off",
                Overlay::Temperature => "Temperature",
                Overlay::Humidity => "Humidity",
                Overlay::Pressure => "Pressure",
                Overlay::Oxygen => "Oxygen",
                Overlay::Moisture => "Material moisture",
                Overlay::BurnProgress => "Burn progress",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.overlay, Overlay::None, "Off");
                ui.selectable_value(&mut app.overlay, Overlay::Temperature, "Temperature");
                ui.selectable_value(&mut app.overlay, Overlay::Humidity, "Humidity");
                ui.selectable_value(&mut app.overlay, Overlay::Pressure, "Pressure");
                ui.selectable_value(&mut app.overlay, Overlay::Oxygen, "Oxygen");
                ui.selectable_value(&mut app.overlay, Overlay::Moisture, "Material moisture");
                ui.selectable_value(&mut app.overlay, Overlay::BurnProgress, "Burn progress");
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
