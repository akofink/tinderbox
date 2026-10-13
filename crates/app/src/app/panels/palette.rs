//! Element palette and quick tool selections.
use crate::app::Tinderbox;
use eframe::egui::{self, Color32, Vec2};

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    ui.heading("Elements");
    ui.separator();
    for (id, element) in app.world.rules().elements.iter().enumerate() {
        let response = ui.selectable_label(id == app.selected, format!("  {}", element.name));
        ui.painter().circle_filled(
            response.rect.left_center() + Vec2::new(9.0, 0.0),
            5.0,
            Color32::from_rgb(element.color[0], element.color[1], element.color[2]),
        );
        if response.clicked() {
            app.selected = id;
        }
    }
    ui.separator();
    if ui.button("Ignite (I)").clicked() {
        app.selected = app.world.rules().id("fire").unwrap_or(app.selected);
    }
    if ui.button("Erase (E)").clicked() {
        app.selected = app.world.rules().id("air").unwrap_or(app.selected);
    }
    ui.small(
        "Left-drag paints\nRight-click ignites\nSpace play/pause\n. single step\nR reset\nH HUD",
    );
}
