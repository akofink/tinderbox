//! Element palette and quick tool selections.
use crate::app::Tinderbox;
use crate::labels::{self, GROUPS};
use eframe::egui::{self, Color32, Sense, Vec2};

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    ui.heading("Elements");
    ui.separator();
    let elements: Vec<_> = app
        .world
        .rules()
        .elements
        .iter()
        .map(|element| (element.name.clone(), element.color))
        .collect();
    egui::ScrollArea::vertical()
        .id_salt("palette")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .show(ui, |ui| {
            for group in GROUPS {
                let rows: Vec<_> = elements
                    .iter()
                    .enumerate()
                    .filter(|(_, (name, _))| labels::group(name) == group)
                    .collect();
                if rows.is_empty() {
                    continue;
                }
                ui.label(egui::RichText::new(group).small().strong());
                for (id, (name, color)) in rows {
                    let selected = id == app.selected;
                    let response = ui
                        .horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                            ui.painter().circle_filled(
                                rect.center(),
                                5.0,
                                Color32::from_rgb(color[0], color[1], color[2]),
                            );
                            ui.selectable_label(selected, labels::display_name(name))
                        })
                        .inner;
                    if response.clicked() {
                        app.selected = id;
                    }
                }
            }
        });
    ui.separator();
    if ui.button("Ignite (I)").clicked() {
        app.selected = app.world.rules().id("fire").unwrap_or(app.selected);
    }
    if ui.button("Erase (E)").clicked() {
        app.selected = app.world.rules().id("air").unwrap_or(app.selected);
    }
    ui.small("Drag paints. Right-click ignites.\nSpace play/pause. Period steps.\nR reset. H HUD.");
}
