//! Element palette and quick tool selections.
use crate::app::Tinderbox;
use crate::labels::{self, GROUPS};
use eframe::egui::{self, Color32, Sense, Vec2};

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    let compact = ui.ctx().screen_rect().width() <= 480.0;
    if compact {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.spacing_mut().item_spacing.y = 2.0;
    }
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
            if compact {
                ui.label(egui::RichText::new("Elem.").size(12.0).strong());
            } else {
                ui.heading("Elements");
            }
            ui.separator();
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
                            let marker_size = if compact { 10.0 } else { 14.0 };
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::splat(marker_size),
                                Sense::hover(),
                            );
                            ui.painter().circle_filled(
                                rect.center(),
                                if compact { 4.0 } else { 5.0 },
                                Color32::from_rgb(color[0], color[1], color[2]),
                            );
                            let label = if compact {
                                match name.as_str() {
                                    "fertile_soil" => String::from("Fertile"),
                                    "leaf_litter" => String::from("Litter"),
                                    "wet_soil" => String::from("Wet"),
                                    "deadwood" => String::from("Dead"),
                                    _ => labels::display_name(name),
                                }
                            } else {
                                labels::display_name(name)
                            };
                            if compact {
                                ui.selectable_label(
                                    selected,
                                    egui::RichText::new(label).size(10.0),
                                )
                            } else {
                                ui.selectable_label(selected, label)
                            }
                        })
                        .inner;
                    if response.clicked() {
                        app.selected = id;
                        app.boat_tool = false;
                    }
                }
            }
            ui.separator();
            let boat_button = if compact {
                if app.boat_tool { "Tap world" } else { "Boat" }
            } else if app.boat_tool {
                "Click world to place boat"
            } else {
                "Place wooden boat"
            };
            if ui.button(boat_button).clicked() {
                app.boat_tool = true;
                app.gust_mode = false;
            }
            if app.boat_tool {
                ui.small("Click at the waterline; the stamp needs clear space.");
            }
            if ui.button(if compact { "Ignite" } else { "Ignite (I)" }).clicked() {
                app.selected = app.world.rules().id("fire").unwrap_or(app.selected);
            }
            if ui.button(if compact { "Erase" } else { "Erase (E)" }).clicked() {
                app.selected = app.world.rules().id("air").unwrap_or(0);
            }
            if !compact {
                ui.small("Drag paints. Right-click ignites.\nSpace play/pause. Period steps.\nR reset. H HUD.");
            }
        });
}
