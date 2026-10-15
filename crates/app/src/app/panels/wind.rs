//! Global wind controls and the local gust-brush toggle.
use crate::app::Tinderbox;
use eframe::egui;
use tinderbox_sim::WindDirection;

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    let compact = super::toolbar::compact_touch_layout(ui.ctx().screen_rect().size());
    let mut wind = app.world.wind();
    ui.label("Wind");
    for (label, direction) in [
        ("West", WindDirection::West),
        ("Calm", WindDirection::Calm),
        ("East", WindDirection::East),
    ] {
        let response = if compact {
            ui.add_sized(
                egui::vec2(52.0, 44.0),
                egui::Button::new(label).selected(wind.direction == direction),
            )
        } else {
            ui.selectable_value(&mut wind.direction, direction, label)
        };
        if response.clicked() {
            wind.direction = direction;
            app.world.set_wind(wind.direction, wind.strength);
        }
    }
    let mut strength = wind.strength;
    let strength_slider = egui::Slider::new(&mut strength, 0..=4);
    if compact {
        ui.add_sized(egui::vec2(104.0, 44.0), strength_slider);
    } else {
        ui.add(strength_slider.text("strength"));
    }
    if strength != wind.strength {
        app.world.set_wind(wind.direction, strength);
    }
    let gust = if compact {
        ui.add_sized(
            egui::vec2(92.0, 44.0),
            egui::Button::new("Gust brush").selected(app.gust_mode),
        )
    } else {
        ui.selectable_label(app.gust_mode, "Gust brush")
    };
    if gust.clicked() {
        app.gust_mode = !app.gust_mode;
        if app.gust_mode {
            app.boat_tool = false;
        }
    }
}
