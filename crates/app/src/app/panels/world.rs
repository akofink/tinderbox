//! World canvas panel and pointer interactions.
use crate::app::{Tinderbox, input, inspector, render};
use eframe::egui::{self, Color32, Sense};

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    if app.level.fit_window {
        app.resize_world_to_fit(ui.available_size());
    }
    let size = if app.level.mode == tinderbox_sim::level::LevelMode::Endless {
        let available = ui.available_size();
        egui::vec2(
            available.x,
            available.y.min(app.world.height() as f32 * app.cell_size),
        )
    } else {
        render::fitted_size(&app.world, ui.available_size())
    };
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let sky = render::upload_dirty(app);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_rgb(18, 22, 34));
    let view_width = if app.level.mode == tinderbox_sim::level::LevelMode::Endless {
        rect.width() * app.world.height() as f32 / rect.height().max(1.0)
    } else {
        app.world.width() as f32
    };
    app.center_camera_on_origin(view_width);
    if app.level.mode == tinderbox_sim::level::LevelMode::Endless {
        let scroll = ui.input(|input| input.smooth_scroll_delta);
        if response.hovered() && scroll != egui::Vec2::ZERO {
            app.pan_camera(-(scroll.x + scroll.y) * 0.2, view_width);
        }
    }
    render::draw_world(
        &app.world_texture,
        &painter,
        rect,
        app.world.width(),
        app.camera_x,
        view_width,
    );
    render::draw_life_sprites(
        &app.world,
        &painter,
        rect,
        app.overlay,
        &sky,
        app.camera_x,
        view_width,
    );
    if app.level.mode == tinderbox_sim::level::LevelMode::Endless {
        render::draw_endless_cursor(&painter, rect);
    }
    input::handle_canvas_pointer(app, &response, rect, view_width);
    if app.inspect_mode {
        #[cfg(not(target_arch = "wasm32"))]
        let inspect_pointer = app.inspect_cell.map(|(x, y)| {
            input::world_to_screen(
                x as f32 + 0.5,
                y as f32 + 0.5,
                rect,
                view_width,
                app.world.height(),
                app.camera_x,
            )
        });
        #[cfg(target_arch = "wasm32")]
        let inspect_pointer = None;
        if let Some(pointer) = response.hover_pos().or(inspect_pointer) {
            inspector::show(app, &painter, rect, pointer, view_width);
        }
    }
    super::hud::show(app, &painter, rect);
}
