//! Play controls, brush controls, speed, reset, and scene setup.
use crate::app::{Overlay, Tinderbox};
use eframe::egui;

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    let compact = compact_touch_layout(ui.ctx().screen_rect().size());
    if compact {
        ui.spacing_mut().interact_size = egui::vec2(44.0, 44.0);
    }
    ui.horizontal_wrapped(|ui| {
        if toolbar_button(ui, if app.running { "Pause" } else { "Play" }, compact).clicked() {
            app.running = !app.running;
        }
        if toolbar_button(ui, "Step", compact).clicked() {
            app.request_step();
        }
        if toolbar_button(ui, "Reset", compact).clicked() {
            app.reset();
        }
        ui.separator();
        ui.label("Speed");
        ui.add(egui::Slider::new(&mut app.speed, 0.25..=10.0).suffix("×"));
        ui.label("Brush");
        ui.add(egui::Slider::new(&mut app.brush, 1..=16).suffix(" cells"));
    });
    ui.horizontal_wrapped(|ui| {
        if toolbar_button(ui, "Level setup", compact).clicked() {
            app.open_level_setup();
        }
        if toolbar_button(ui, "Wildfire", compact).clicked() {
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
        if compact {
            if ui
                .add_sized(
                    egui::vec2(56.0, 44.0),
                    egui::Button::new("Rules").selected(app.rules_panel.open),
                )
                .clicked()
            {
                app.rules_panel.open = !app.rules_panel.open;
            }
        } else {
            ui.toggle_value(&mut app.rules_panel.open, "Rules");
        }
    });
    ui.horizontal_wrapped(|ui| {
        super::wind::show(app, ui);
    });
}

fn toolbar_button(ui: &mut egui::Ui, label: &str, compact: bool) -> egui::Response {
    if compact {
        ui.add_sized(egui::vec2(44.0, 44.0), egui::Button::new(label))
    } else {
        ui.button(label)
    }
}

pub(in crate::app) fn compact_touch_layout(screen: egui::Vec2) -> bool {
    screen.x <= 600.0 || screen.y <= 500.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_phone_viewports_size_toolbar_controls_for_touch() {
        assert!(compact_touch_layout(egui::vec2(390.0, 844.0)));
        assert!(compact_touch_layout(egui::vec2(844.0, 390.0)));
        assert!(compact_touch_layout(egui::vec2(320.0, 640.0)));
        assert!(!compact_touch_layout(egui::vec2(1280.0, 800.0)));

        let ctx = egui::Context::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(390.0, 844.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if compact_touch_layout(ui.ctx().screen_rect().size()) {
                        ui.spacing_mut().interact_size = egui::vec2(44.0, 44.0);
                    }
                    let button = toolbar_button(ui, "Play", true);
                    let checkbox = ui.checkbox(&mut false, "HUD");
                    let slider = ui.add(egui::Slider::new(&mut 0.5_f32, 0.0..=1.0));
                    for (name, response) in [
                        ("button", button),
                        ("checkbox", checkbox),
                        ("slider", slider),
                    ] {
                        assert!(
                            response.rect.height() >= 44.0,
                            "{name} target is too short: {:?}",
                            response.rect
                        );
                        assert!(
                            response.rect.width() >= 44.0,
                            "{name} target is too narrow: {:?}",
                            response.rect
                        );
                    }
                });
            },
        );
        assert!(!output.shapes.is_empty());
    }
}
