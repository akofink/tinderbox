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
    if narrow_toolbar_layout(screen) {
        show_narrow(app, ui);
        return;
    }
    let compact = compact_touch_layout(screen);
    if compact {
        ui.spacing_mut().interact_size = egui::vec2(44.0, 44.0);
    }
    ui.horizontal_wrapped(|ui| {
        let play = toolbar_button(ui, if app.running { "Pause" } else { "Play" }, compact);
        record_test_bounds(ui, "play-pause", play.rect, || app.running.to_string());
        if play.clicked() {
            app.running = !app.running;
        }
        let step = toolbar_button(ui, "Step", compact);
        record_test_bounds(ui, "step", step.rect, String::new);
        if step.clicked() {
            app.request_step();
        }
        let reset = toolbar_button(ui, "Reset", compact);
        record_test_bounds(ui, "reset", reset.rect, String::new);
        if reset.clicked() {
            app.reset();
        }
        if compact {
            let navigate = ui
                .add_sized(
                    egui::vec2(92.0, 44.0),
                    egui::Button::new("Navigate").selected(app.navigation_mode),
                )
                .on_hover_text("Drag the canvas to pan; pinch to zoom");
            record_test_bounds(ui, "navigate", navigate.rect, String::new);
            if navigate.clicked() {
                app.navigation_mode = !app.navigation_mode;
            }
        }
        ui.separator();
        ui.label("Speed");
        let speed = ui.add(egui::Slider::new(&mut app.speed, 0.25..=10.0).suffix("×"));
        record_test_bounds(ui, "speed", speed.rect, || app.speed.to_string());
        ui.label("Brush");
        let response = ui.add(egui::Slider::new(&mut app.brush, 1..=16).suffix(" cells"));
        record_test_bounds(ui, "brush", response.rect, || app.brush.to_string());
    });
    ui.horizontal_wrapped(|ui| {
        let level_setup = toolbar_button(ui, "Level setup", compact);
        record_test_bounds(ui, "level-setup", level_setup.rect, String::new);
        if level_setup.clicked() {
            app.open_level_setup();
        }
        let wildfire = toolbar_button(ui, "Wildfire", compact);
        record_test_bounds(ui, "wildfire", wildfire.rect, String::new);
        if wildfire.clicked() {
            app.wildfire();
        }
        ui.label("Cell size");
        let response = ui.add(egui::Slider::new(&mut app.cell_size, 2.0..=8.0).suffix(" pt"));
        record_test_bounds(ui, "cell-size", response.rect, || {
            format!("{:.2}", app.cell_size)
        });
        let hud = ui.checkbox(&mut app.show_hud, "HUD");
        record_test_bounds(ui, "hud", hud.rect, || app.show_hud.to_string());
        let inspect = ui.add_sized(
            egui::vec2(104.0, 44.0),
            egui::Button::new("Inspect (M)").selected(app.inspect_mode),
        );
        record_test_bounds(ui, "inspect", inspect.rect, String::new);
        if inspect.clicked() {
            app.inspect_mode = !app.inspect_mode;
        }
        if app.inspect_mode {
            ui.label("Tap a cell; Inspect again to paint");
        }
        let previous_overlay = app.overlay;
        let overlay = egui::ComboBox::from_label("Overlay")
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
        record_test_bounds(ui, "overlay", overlay.response.rect, String::new);
        if app.overlay != previous_overlay {
            app.world.mark_all_dirty();
        }
        if compact {
            let rules = ui.add_sized(
                egui::vec2(56.0, 44.0),
                egui::Button::new("Rules").selected(app.rules_panel.open),
            );
            record_test_bounds(ui, "rules", rules.rect, String::new);
            if rules.clicked() {
                app.rules_panel.open = !app.rules_panel.open;
            }
        } else {
            let rules = ui.toggle_value(&mut app.rules_panel.open, "Rules");
            record_test_bounds(ui, "rules", rules.rect, String::new);
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
            let play = ui.add_sized(
                egui::vec2(52.0, 44.0),
                egui::Button::new(if app.running { "Pause" } else { "Play" }),
            );
            record_test_bounds(ui, "play-pause", play.rect, || app.running.to_string());
            if play.clicked() {
                app.running = !app.running;
            }
            let step = toolbar_button(ui, "Step", true);
            record_test_bounds(ui, "step", step.rect, String::new);
            if step.clicked() {
                app.request_step();
            }
            let reset = ui.add_sized(egui::vec2(52.0, 44.0), egui::Button::new("Reset"));
            record_test_bounds(ui, "reset", reset.rect, String::new);
            if reset.clicked() {
                app.reset();
            }
            let navigate = ui.add_sized(
                egui::vec2(92.0, 44.0),
                egui::Button::new("Navigate").selected(app.navigation_mode),
            );
            record_test_bounds(ui, "navigate", navigate.rect, String::new);
            if navigate.clicked() {
                app.navigation_mode = !app.navigation_mode;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Speed");
            let speed = ui.add_sized(
                egui::vec2(104.0, 44.0),
                egui::Slider::new(&mut app.speed, 0.25..=10.0).suffix("×"),
            );
            record_test_bounds(ui, "speed", speed.rect, || app.speed.to_string());
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
            let level_setup =
                ui.add_sized(egui::vec2(92.0, 44.0), egui::Button::new("Level setup"));
            record_test_bounds(ui, "level-setup", level_setup.rect, String::new);
            if level_setup.clicked() {
                app.open_level_setup();
            }
            let wildfire = ui.add_sized(egui::vec2(76.0, 44.0), egui::Button::new("Wildfire"));
            record_test_bounds(ui, "wildfire", wildfire.rect, String::new);
            if wildfire.clicked() {
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
            let hud = ui.checkbox(&mut app.show_hud, "HUD");
            record_test_bounds(ui, "hud", hud.rect, || app.show_hud.to_string());
            let inspect = ui.add_sized(
                egui::vec2(104.0, 44.0),
                egui::Button::new("Inspect (M)").selected(app.inspect_mode),
            );
            record_test_bounds(ui, "inspect", inspect.rect, String::new);
            if inspect.clicked() {
                app.inspect_mode = !app.inspect_mode;
            }
            let rules = ui.add_sized(
                egui::vec2(56.0, 44.0),
                egui::Button::new("Rules").selected(app.rules_panel.open),
            );
            record_test_bounds(ui, "rules", rules.rect, String::new);
            if rules.clicked() {
                app.rules_panel.open = !app.rules_panel.open;
            }
        });
        ui.horizontal(|ui| {
            let overlay = egui::ComboBox::from_label("Overlay")
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
            record_test_bounds(ui, "overlay", overlay.response.rect, String::new);
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

fn narrow_toolbar_layout(screen: egui::Vec2) -> bool {
    screen.x <= 420.0
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
        assert!(narrow_toolbar_layout(egui::vec2(390.0, 664.0)));
        assert!(narrow_toolbar_layout(egui::vec2(390.0, 844.0)));
        assert!(narrow_toolbar_layout(egui::vec2(320.0, 640.0)));
        assert!(!narrow_toolbar_layout(egui::vec2(750.0, 342.0)));
        assert!(!narrow_toolbar_layout(egui::vec2(1180.0, 800.0)));
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
