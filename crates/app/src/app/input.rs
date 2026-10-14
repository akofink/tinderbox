//! Keyboard and pointer mapping for player tools.
use super::Tinderbox;
use eframe::egui::{self, Pos2, Rect, Response};

/// Convert a canvas pointer position into world-space cell coordinates.
pub(crate) fn screen_to_world(
    pos: Pos2,
    rect: Rect,
    view_width: f32,
    world_height: usize,
    camera_x: f32,
) -> (f32, f32) {
    let x = camera_x + (pos.x - rect.left()) / rect.width().max(1.0) * view_width;
    let y = (pos.y - rect.top()) / rect.height().max(1.0) * world_height as f32;
    (x, y)
}

/// Convert world-space cell coordinates into their canvas position.
#[allow(dead_code)] // Shared with the parallel cell-inspector feature.
pub(crate) fn world_to_screen(
    x: f32,
    y: f32,
    rect: Rect,
    view_width: f32,
    world_height: usize,
    camera_x: f32,
) -> Pos2 {
    Pos2::new(
        rect.left() + (x - camera_x) / view_width.max(1.0) * rect.width(),
        rect.top() + y / world_height.max(1) as f32 * rect.height(),
    )
}

pub(super) fn handle_keyboard(app: &mut Tinderbox, ctx: &egui::Context) {
    let editing_text = ctx
        .memory(|m| m.focused())
        .is_some_and(|id| egui::text_edit::TextEditState::load(ctx, id).is_some());
    if app.level_setup.open || editing_text {
        return;
    }
    let keys = ctx.input(|input| {
        (
            input.key_pressed(egui::Key::Space),
            input.key_pressed(egui::Key::Period),
            input.key_pressed(egui::Key::R),
            input.key_pressed(egui::Key::H),
            input.key_pressed(egui::Key::E),
            input.key_pressed(egui::Key::I),
            input.key_pressed(egui::Key::M),
            input.key_down(egui::Key::ArrowLeft),
            input.key_down(egui::Key::ArrowRight),
        )
    });
    if keys.0 {
        app.running = !app.running;
    }
    if keys.1 {
        app.request_step();
    }
    if keys.2 {
        app.reset();
    }
    if keys.3 {
        app.show_hud = !app.show_hud;
    }
    if keys.4 {
        app.selected = app.world.rules().id("air").unwrap_or(app.selected);
    }
    if keys.5 {
        app.selected = app.world.rules().id("fire").unwrap_or(app.selected);
    }
    if keys.6 {
        app.inspect_mode = !app.inspect_mode;
    }
    if keys.7 {
        app.pan_camera(-8.0, app.level.width as f32);
    }
    if keys.8 {
        app.pan_camera(8.0, app.level.width as f32);
    }
}

pub(super) fn handle_canvas_pointer(
    app: &mut Tinderbox,
    response: &Response,
    rect: Rect,
    view_width: f32,
) {
    if app.level_setup.open || app.inspect_mode || !response.hovered() {
        return;
    }
    let Some(pos) = response.interact_pointer_pos() else {
        return;
    };
    if response.dragged_by(egui::PointerButton::Primary)
        || response.clicked_by(egui::PointerButton::Primary)
    {
        if app.boat_tool {
            boat_at(app, pos, rect, view_width);
            app.boat_tool = false;
        } else if app.gust_mode {
            gust_at(app, pos, rect, view_width);
        } else {
            paint_at(app, pos, rect, view_width, app.selected);
        }
    }
    if response.clicked_by(egui::PointerButton::Secondary) {
        let fire = app.world.rules().id("fire").unwrap_or(app.selected);
        paint_at(app, pos, rect, view_width, fire);
    }
}

fn boat_at(app: &mut Tinderbox, pos: Pos2, rect: Rect, view_width: f32) {
    let (x, y) = screen_to_world(pos, rect, view_width, app.world.height(), app.camera_x);
    let (x, y) = (x.floor() as usize, y.floor() as usize);
    app.world.place_boat(x, y);
}

fn gust_at(app: &mut Tinderbox, pos: Pos2, rect: Rect, view_width: f32) {
    let (x, y) = screen_to_world(pos, rect, view_width, app.world.height(), app.camera_x);
    let (x, y) = (x.floor() as usize, y.floor() as usize);
    let wind = app.world.wind();
    app.world
        .gust(x, y, app.brush / 2, wind.direction, wind.strength);
}

fn paint_at(app: &mut Tinderbox, pos: Pos2, rect: Rect, view_width: f32, element: usize) {
    let (x, y) = screen_to_world(pos, rect, view_width, app.world.height(), app.camera_x);
    let (x, y) = (x.floor() as usize, y.floor() as usize);
    let size = app.brush as isize;
    let start = -(size / 2);
    for dy in start..start + size {
        for dx in start..start + size {
            let px = x as isize + dx;
            let py = y as isize + dy;
            if px >= 0 && py >= 0 {
                app.world.paint(px as usize, py as usize, element);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_screen_transforms_round_trip_with_camera_offset() {
        let rect = Rect::from_min_max(Pos2::new(17.0, 23.0), Pos2::new(617.0, 423.0));
        let screen = world_to_screen(193.25, 87.5, rect, 240.0, 160, 80.0);
        let (world_x, world_y) = screen_to_world(screen, rect, 240.0, 160, 80.0);
        assert!((world_x - 193.25).abs() < 0.001);
        assert!((world_y - 87.5).abs() < 0.001);
    }
}
