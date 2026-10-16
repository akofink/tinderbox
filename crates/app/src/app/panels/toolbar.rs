//! Play controls, brush controls, speed, reset, and scene setup.
use crate::app::{Overlay, Tinderbox};
use eframe::egui;
#[cfg(target_arch = "wasm32")]
use std::sync::OnceLock;

#[cfg(target_arch = "wasm32")]
static TOOLBAR_TEST_MODE: OnceLock<bool> = OnceLock::new();

#[cfg(target_arch = "wasm32")]
pub(in crate::app) fn set_test_mode(enabled: bool) {
    let _ = TOOLBAR_TEST_MODE.set(enabled);
}

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    let screen = ui.ctx().screen_rect().size();
    if screen.x <= 360.0 {
        show_narrow(app, ui);
        return;
    }
    let compact = compact_touch_layout(screen);
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
        if compact
            && ui
                .add_sized(
                    egui::vec2(92.0, 44.0),
                    egui::Button::new("Navigate").selected(app.navigation_mode),
                )
                .on_hover_text("Drag the canvas to pan; pinch to zoom")
                .clicked()
        {
            app.navigation_mode = !app.navigation_mode;
        }
        ui.separator();
        ui.label("Speed");
        ui.add(egui::Slider::new(&mut app.speed, 0.25..=10.0).suffix("×"));
        ui.label("Brush");
        let response = ui.add(egui::Slider::new(&mut app.brush, 1..=16).suffix(" cells"));
        record_test_bounds(ui, "brush", response.rect, || app.brush.to_string());
    });
    ui.horizontal_wrapped(|ui| {
        if toolbar_button(ui, "Level setup", compact).clicked() {
            app.open_level_setup();
        }
        if toolbar_button(ui, "Wildfire", compact).clicked() {
            app.wildfire();
        }
        ui.label("Cell size");
        let response = ui.add(egui::Slider::new(&mut app.cell_size, 2.0..=8.0).suffix(" pt"));
        record_test_bounds(ui, "cell-size", response.rect, || {
            format!("{:.2}", app.cell_size)
        });
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

fn show_narrow(app: &mut Tinderbox, ui: &mut egui::Ui) {
    ui.spacing_mut().interact_size = egui::vec2(44.0, 44.0);
    ui.spacing_mut().slider_width = 104.0;
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            if ui
                .add_sized(
                    egui::vec2(52.0, 44.0),
                    egui::Button::new(if app.running { "Pause" } else { "Play" }),
                )
                .clicked()
            {
                app.running = !app.running;
            }
            if toolbar_button(ui, "Step", true).clicked() {
                app.request_step();
            }
            if ui
                .add_sized(egui::vec2(52.0, 44.0), egui::Button::new("Reset"))
                .clicked()
            {
                app.reset();
            }
            if ui
                .add_sized(
                    egui::vec2(92.0, 44.0),
                    egui::Button::new("Navigate").selected(app.navigation_mode),
                )
                .clicked()
            {
                app.navigation_mode = !app.navigation_mode;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Speed");
            ui.add_sized(
                egui::vec2(104.0, 44.0),
                egui::Slider::new(&mut app.speed, 0.25..=10.0).suffix("×"),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Brush");
            let response = ui.add_sized(
                egui::vec2(104.0, 44.0),
                egui::Slider::new(&mut app.brush, 1..=16).suffix(" cells"),
            );
            record_test_bounds(ui, "brush", response.rect, || app.brush.to_string());
        });
        ui.horizontal(|ui| {
            if ui
                .add_sized(egui::vec2(92.0, 44.0), egui::Button::new("Level setup"))
                .clicked()
            {
                app.open_level_setup();
            }
            if ui
                .add_sized(egui::vec2(76.0, 44.0), egui::Button::new("Wildfire"))
                .clicked()
            {
                app.wildfire();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Cell size");
            let response = ui.add_sized(
                egui::vec2(104.0, 44.0),
                egui::Slider::new(&mut app.cell_size, 2.0..=8.0).suffix(" pt"),
            );
            record_test_bounds(ui, "cell-size", response.rect, || {
                format!("{:.2}", app.cell_size)
            });
        });
        ui.horizontal(|ui| {
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
            if ui
                .add_sized(
                    egui::vec2(56.0, 44.0),
                    egui::Button::new("Rules").selected(app.rules_panel.open),
                )
                .clicked()
            {
                app.rules_panel.open = !app.rules_panel.open;
            }
        });
        ui.horizontal(|ui| {
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
        });
        ui.horizontal(|ui| super::wind::show_directions(app, ui));
        ui.horizontal(|ui| super::wind::show_strength_and_gust(app, ui, true));
    });
}

pub(in crate::app) fn record_test_bounds<F>(ui: &egui::Ui, name: &str, rect: egui::Rect, value: F)
where
    F: FnOnce() -> String,
{
    #[cfg(target_arch = "wasm32")]
    {
        if !TOOLBAR_TEST_MODE.get().copied().unwrap_or(false) {
            return;
        }
        let Some(canvas) = eframe::web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("the_canvas_id"))
        else {
            return;
        };
        let clip = ui.clip_rect();
        let value = value();
        let _ = canvas.set_attribute(
            &format!("data-toolbar-{name}"),
            &format!(
                "{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{value}",
                rect.min.x,
                rect.min.y,
                rect.max.x,
                rect.max.y,
                clip.min.x,
                clip.min.y,
                clip.max.x,
                clip.max.y,
            ),
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (ui, name, rect, value);
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
    fn disabled_test_bounds_do_not_format_values() {
        let ctx = egui::Context::default();
        let mut formatted = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                record_test_bounds(ui, "probe", ui.min_rect(), || {
                    formatted = true;
                    String::from("probe")
                });
            });
        });
        assert!(!formatted);
    }

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
