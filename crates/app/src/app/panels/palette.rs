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
                ui.label(
                    egui::RichText::new("Elem.")
                        .size(12.0)
                        .strong()
                        .color(Color32::from_rgb(224, 228, 238)),
                );
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
                ui.label(
                    egui::RichText::new(group)
                        .small()
                        .strong()
                        .color(Color32::from_rgb(224, 228, 238)),
                );
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
                            let foreground = if selected && !ui.visuals().dark_mode {
                                Color32::from_rgb(24, 36, 52)
                            } else {
                                Color32::from_rgb(224, 228, 238)
                            };
                            let text = egui::RichText::new(label).color(foreground);
                            if compact {
                                ui.selectable_label(selected, text.size(10.0))
                            } else {
                                ui.selectable_label(selected, text)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Tinderbox;
    use eframe::egui::Shape;

    fn contrast_ratio(foreground: Color32, background: Color32) -> f32 {
        fn luminance(color: Color32) -> f32 {
            let linear = |channel: u8| {
                let value = channel as f32 / 255.0;
                if value <= 0.04045 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
        }
        let (lighter, darker) = {
            let a = luminance(foreground);
            let b = luminance(background);
            (a.max(b), a.min(b))
        };
        (lighter + 0.05) / (darker + 0.05)
    }

    #[test]
    fn ordinary_and_selected_palette_labels_render_with_readable_contrast_in_both_orientations() {
        for screen in [egui::vec2(390.0, 844.0), egui::vec2(844.0, 390.0)] {
            let ctx = egui::Context::default();
            let mut app = Tinderbox::new_state(false);
            app.selected = app.world.rules().id("sand").unwrap();
            let visuals = ctx.style().visuals.clone();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)),
                    ..Default::default()
                },
                |ctx| {
                    egui::SidePanel::left("palette-test")
                        .exact_width(168.0)
                        .show(ctx, |ui| show(&mut app, ui));
                },
            );
            let rendered_color = |label: &str| {
                output
                    .shapes
                    .iter()
                    .find_map(|clipped| match &clipped.shape {
                        Shape::Text(text) if text.galley.job.text == label => text
                            .galley
                            .job
                            .sections
                            .first()
                            .map(|section| section.format.color),
                        _ => None,
                    })
            };
            let ordinary = rendered_color("Air").expect("ordinary Air label was rendered");
            let selected = rendered_color("Sand").expect("selected Sand label was rendered");
            assert!(contrast_ratio(ordinary, visuals.panel_fill) >= 4.5);
            assert!(
                contrast_ratio(selected, visuals.selection.bg_fill) >= 4.5,
                "selected label contrast is {:.2}: label={selected:?}, selection={:?}",
                contrast_ratio(selected, visuals.selection.bg_fill),
                visuals.selection.bg_fill,
            );
        }
    }
}
