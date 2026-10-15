//! World canvas panel and pointer interactions.
use crate::app::{Tinderbox, input, inspector, render};
use eframe::egui::{self, Color32, Sense};
use web_time::Instant;

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) -> egui::Rect {
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
    if app.stress_play {
        app.stress_max_pending_dirty_regions = app
            .stress_max_pending_dirty_regions
            .max(app.world.pending_dirty_regions());
    }
    let upload_started = Instant::now();
    let sky = render::upload_dirty(app);
    if app.stress_play {
        app.stress_max_upload_ms = app
            .stress_max_upload_ms
            .max(upload_started.elapsed().as_secs_f32() * 1000.0);
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_rgb(18, 22, 34));
    let base_view_width = if app.level.mode == tinderbox_sim::level::LevelMode::Endless {
        rect.width() * app.world.height() as f32 / rect.height().max(1.0)
    } else {
        app.world.width() as f32
    };
    let view_width = (base_view_width / app.camera_zoom).max(1.0);
    app.center_camera_on_origin(view_width);
    let pinch = ui
        .input(|input| input.multi_touch())
        .filter(|touch| touch.num_touches >= 2 && rect.contains(touch.center_pos));
    if let Some(touch) = pinch {
        app.touch_gesture_active = true;
        app.zoom_camera(
            touch.zoom_delta,
            (touch.center_pos.x - rect.left()) / rect.width().max(1.0),
            base_view_width,
        );
    }
    if app.level.mode == tinderbox_sim::level::LevelMode::Endless && app.camera_initialized {
        let global =
            app.world_start_chunk as f32 * tinderbox_sim::ENDLESS_CHUNK_WIDTH as f32 + app.camera_x;
        app.sync_active_span(global, view_width);
    }
    let pan_drag = if pinch.is_some() || app.touch_gesture_active {
        true
    } else {
        let (delta_x, shift_down, primary_down, middle_down, scroll) = ui.input(|input| {
            (
                input.pointer.delta().x,
                input.modifiers.shift,
                input.pointer.button_down(egui::PointerButton::Primary),
                input.pointer.button_down(egui::PointerButton::Middle),
                input.smooth_scroll_delta,
            )
        });
        let dragging = response.hovered()
            && (middle_down
                || (shift_down && primary_down)
                || (app.navigation_mode && primary_down));
        if dragging && delta_x != 0.0 {
            app.pan_camera(-delta_x * view_width / rect.width().max(1.0), view_width);
        }
        if app.level.mode == tinderbox_sim::level::LevelMode::Endless
            && response.hovered()
            && scroll != egui::Vec2::ZERO
        {
            app.pan_camera(-(scroll.x + scroll.y) * 0.2, view_width);
        }
        dragging
    };
    let draw_started = Instant::now();
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
    input::handle_canvas_pointer(app, &response, rect, view_width, pan_drag);
    if app.touch_gesture_active && !ui.input(|input| input.any_touches()) {
        app.touch_gesture_active = false;
    }
    if app.inspect_mode {
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
        if let Some(pointer) = response.hover_pos().or(inspect_pointer) {
            inspector::show(app, &painter, rect, pointer, view_width);
        }
    }
    super::hud::show(app, &painter, rect);
    if app.stress_play {
        app.stress_max_draw_ms = app
            .stress_max_draw_ms
            .max(draw_started.elapsed().as_secs_f32() * 1000.0);
    }
    rect
}
