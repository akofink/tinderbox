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
    let base_view_height = app.world.height() as f32;
    let base_view_width = rect.width() * base_view_height / rect.height().max(1.0);
    let view_width = (base_view_width / app.camera_zoom).max(1.0);
    let view_height = (base_view_height / app.camera_zoom).max(1.0);
    let was_camera_initialized = app.camera_initialized;
    let previous_view_width = app.camera_view_width;
    app.center_camera_on_origin(view_width);
    if was_camera_initialized
        && app.level.mode == tinderbox_sim::level::LevelMode::Endless
        && (previous_view_width - view_width).abs() > f32::EPSILON
    {
        // Preserve the global world point at the viewport center while rotation
        // changes the visible width, then retain enough chunks around that view.
        app.camera_x += (previous_view_width - view_width) / 2.0;
        app.pan_camera(0.0, view_width);
    }
    app.camera_view_height = view_height;
    let mut camera = input::CameraView {
        x: app.camera_x,
        y: app.camera_y,
        width: view_width,
        height: view_height,
    };
    let pinch = ui
        .input(|input| input.multi_touch())
        .filter(|touch| touch.num_touches >= 2 && rect.contains(touch.center_pos));
    if let Some(touch) = pinch {
        app.touch_gesture_active = true;
        app.zoom_camera(
            touch.zoom_delta,
            egui::vec2(
                (touch.center_pos.x - rect.left()) / rect.width().max(1.0),
                (touch.center_pos.y - rect.top()) / rect.height().max(1.0),
            ),
            base_view_width,
            base_view_height,
        );
        camera = input::CameraView {
            x: app.camera_x,
            y: app.camera_y,
            width: app.camera_view_width,
            height: app.camera_view_height,
        };
    }
    if app.level.mode == tinderbox_sim::level::LevelMode::Endless && app.camera_initialized {
        let global =
            app.world_start_chunk as f32 * tinderbox_sim::ENDLESS_CHUNK_WIDTH as f32 + app.camera_x;
        app.sync_active_span(global, camera.width);
        camera.x = app.camera_x;
    }
    let pan_drag = if pinch.is_some() || app.touch_gesture_active {
        true
    } else {
        let (delta, shift_down, primary_down, middle_down, scroll) = ui.input(|input| {
            (
                input.pointer.delta(),
                input.modifiers.shift,
                input.pointer.button_down(egui::PointerButton::Primary),
                input.pointer.button_down(egui::PointerButton::Middle),
                input.smooth_scroll_delta,
            )
        });
        let endless = app.level.mode == tinderbox_sim::level::LevelMode::Endless;
        let dragging = response.hovered()
            && ((endless && (middle_down || (shift_down && primary_down)))
                || (app.navigation_mode && primary_down));
        if dragging && delta != egui::Vec2::ZERO {
            app.pan_camera_2d(
                -delta.x * camera.width / rect.width().max(1.0),
                if app.navigation_mode {
                    -delta.y * camera.height / rect.height().max(1.0)
                } else {
                    0.0
                },
                camera.width,
                camera.height,
            );
            camera.x = app.camera_x;
            camera.y = app.camera_y;
        }
        if app.level.mode == tinderbox_sim::level::LevelMode::Endless
            && response.hovered()
            && scroll != egui::Vec2::ZERO
        {
            app.pan_camera(-(scroll.x + scroll.y) * 0.2, camera.width);
            camera.x = app.camera_x;
        }
        dragging
    };
    let draw_started = Instant::now();
    render::draw_world(
        &app.world_texture,
        &painter,
        rect,
        app.world.width(),
        app.world.height(),
        camera,
    );
    render::draw_life_sprites(&app.world, &painter, rect, app.overlay, &sky, camera);
    if app.level.mode == tinderbox_sim::level::LevelMode::Endless {
        render::draw_endless_cursor(&painter, rect);
    }
    input::handle_canvas_pointer(app, &response, rect, camera, pan_drag);
    if app.touch_gesture_active && !ui.input(|input| input.any_touches()) {
        app.touch_gesture_active = false;
    }
    if app.inspect_mode {
        let inspect_pointer = app
            .inspect_cell
            .map(|(x, y)| input::world_to_screen(x as f32 + 0.5, y as f32 + 0.5, rect, camera));
        if let Some(pointer) = response.hover_pos().or(inspect_pointer) {
            inspector::show(app, &painter, rect, pointer, camera);
        }
    }
    super::hud::show(app, &painter, rect);
    if app.stress_play {
        app.stress_max_draw_ms = app
            .stress_max_draw_ms
            .max(draw_started.elapsed().as_secs_f32() * 1000.0);
    }
    super::toolbar::record_test_panel_bounds("world", rect);
    rect
}
