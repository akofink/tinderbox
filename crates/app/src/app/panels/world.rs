//! World canvas panel and pointer interactions.
use crate::app::{Tinderbox, input, inspector, render};
use eframe::egui::{self, Color32, Sense};

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    if app.level.fit_window {
        app.resize_world_to_fit(ui.available_size());
    }
    let size = render::fitted_size(&app.world, ui.available_size());
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let sky = render::upload_dirty(app);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_rgb(18, 22, 34));
    render::draw_world(&app.world_texture, &painter, rect);
    render::draw_life_sprites(&app.world, &painter, rect, app.overlay, &sky);
    input::handle_canvas_pointer(app, &response, rect);
    if app.inspect_mode {
        #[cfg(not(target_arch = "wasm32"))]
        let inspect_pointer = app.inspect_cell.map(|(x, y)| {
            egui::pos2(
                rect.left() + (x as f32 + 0.5) / app.world.width() as f32 * rect.width(),
                rect.top() + (y as f32 + 0.5) / app.world.height() as f32 * rect.height(),
            )
        });
        #[cfg(target_arch = "wasm32")]
        let inspect_pointer = None;
        if let Some(pointer) = response.hover_pos().or(inspect_pointer) {
            inspector::show(app, &painter, rect, pointer);
        }
    }
    super::hud::show(app, &painter, rect);
}
