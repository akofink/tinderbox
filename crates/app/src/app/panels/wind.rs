//! Global wind controls and the local gust-brush toggle.
use crate::app::Tinderbox;
use eframe::egui;
use tinderbox_sim::WindDirection;

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    let mut wind = app.world.wind();
    ui.label("Wind");
    for (label, direction) in [
        ("West", WindDirection::West),
        ("Calm", WindDirection::Calm),
        ("East", WindDirection::East),
    ] {
        if ui
            .selectable_value(&mut wind.direction, direction, label)
            .changed()
        {
            app.world.set_wind(wind.direction, wind.strength);
        }
    }
    let mut strength = wind.strength;
    ui.add(egui::Slider::new(&mut strength, 0..=4).text("strength"));
    if strength != wind.strength {
        app.world.set_wind(wind.direction, strength);
    }
    if ui.selectable_label(app.gust_mode, "Gust brush").clicked() {
        app.gust_mode = !app.gust_mode;
        if app.gust_mode {
            app.boat_tool = false;
        }
    }
}
