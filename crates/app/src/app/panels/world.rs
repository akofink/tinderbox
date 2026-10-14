//! World canvas panel and pointer interactions.
use crate::app::{Tinderbox, input, render};
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
    super::hud::show(app, &painter, rect);
}
