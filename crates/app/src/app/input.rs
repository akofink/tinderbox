//! Keyboard and pointer mapping for player tools.
use super::Tinderbox;
use eframe::egui::{self, Pos2, Rect, Response};

#[derive(Clone, Copy, Debug)]
pub(super) struct CameraView {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
}

/// Convert a canvas pointer position into world-space cell coordinates.
pub(super) fn screen_to_world(pos: Pos2, rect: Rect, camera: CameraView) -> (f32, f32) {
    let x = camera.x + (pos.x - rect.left()) / rect.width().max(1.0) * camera.width;
    let y = camera.y + (pos.y - rect.top()) / rect.height().max(1.0) * camera.height;
    (x, y)
}

/// Convert world-space cell coordinates into their canvas position.
#[allow(dead_code)] // Shared with the parallel cell-inspector feature.
pub(super) fn world_to_screen(x: f32, y: f32, rect: Rect, camera: CameraView) -> Pos2 {
    Pos2::new(
        rect.left() + (x - camera.x) / camera.width.max(1.0) * rect.width(),
        rect.top() + (y - camera.y) / camera.height.max(1.0) * rect.height(),
    )
}

pub(super) fn clear_level_setup_text_focus(ctx: &egui::Context) {
    if let Some(focused) = ctx
        .memory(|memory| memory.focused())
        .filter(|id| egui::text_edit::TextEditState::load(ctx, *id).is_some())
    {
        ctx.memory_mut(|memory| memory.surrender_focus(focused));
    }
}

pub(super) fn handle_keyboard(app: &mut Tinderbox, ctx: &egui::Context) {
    let editing_text = ctx
        .memory(|m| m.focused())
        .is_some_and(|id| egui::text_edit::TextEditState::load(ctx, id).is_some());
    if app.level_setup.open || (app.rules_panel.open && editing_text) {
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
        app.pan_camera(-8.0, app.camera_view_width);
    }
    if keys.8 {
        app.pan_camera(8.0, app.camera_view_width);
    }
}

pub(super) fn handle_canvas_pointer(
    app: &mut Tinderbox,
    response: &Response,
    rect: Rect,
    camera: CameraView,
    pan_drag: bool,
) {
    if app.level_setup.open
        || pan_drag
        || (app.navigation_mode && !app.inspect_mode)
        || !response.hovered()
    {
        return;
    }
    let Some(pos) = response.interact_pointer_pos() else {
        return;
    };
    if app.inspect_mode {
        if response.clicked_by(egui::PointerButton::Primary) {
            app.inspect_cell = Some(cell_at(
                pos,
                rect,
                camera,
                app.world.width(),
                app.world.height(),
            ));
        }
        return;
    }
    if response.dragged_by(egui::PointerButton::Primary)
        || response.clicked_by(egui::PointerButton::Primary)
    {
        if app.boat_tool {
            boat_at(app, pos, rect, camera);
            app.boat_tool = false;
        } else if app.gust_mode {
            gust_at(app, pos, rect, camera);
        } else {
            paint_at(app, pos, rect, camera, app.selected);
        }
    }
    if response.clicked_by(egui::PointerButton::Secondary) {
        let fire = app.world.rules().id("fire").unwrap_or(app.selected);
        paint_at(app, pos, rect, camera, fire);
    }
}

fn cell_at(
    pos: Pos2,
    rect: Rect,
    camera: CameraView,
    world_width: usize,
    world_height: usize,
) -> (usize, usize) {
    let (x, y) = screen_to_world(pos, rect, camera);
    (
        (x.floor() as isize).clamp(0, world_width.saturating_sub(1) as isize) as usize,
        (y.floor() as isize).clamp(0, world_height.saturating_sub(1) as isize) as usize,
    )
}

fn boat_at(app: &mut Tinderbox, pos: Pos2, rect: Rect, camera: CameraView) {
    let (x, y) = screen_to_world(pos, rect, camera);
    let (x, y) = (x.floor() as usize, y.floor() as usize);
    app.world.place_boat(x, y);
}

fn gust_at(app: &mut Tinderbox, pos: Pos2, rect: Rect, camera: CameraView) {
    let (x, y) = screen_to_world(pos, rect, camera);
    let (x, y) = (x.floor() as usize, y.floor() as usize);
    let wind = app.world.wind();
    app.world
        .gust(x, y, app.brush / 2, wind.direction, wind.strength);
}

fn paint_at(app: &mut Tinderbox, pos: Pos2, rect: Rect, camera: CameraView, element: usize) {
    let (x, y) = screen_to_world(pos, rect, camera);
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
        let camera = CameraView {
            x: 80.0,
            y: 40.0,
            width: 240.0,
            height: 80.0,
        };
        let screen = world_to_screen(193.25, 87.5, rect, camera);
        let (world_x, world_y) = screen_to_world(screen, rect, camera);
        assert!((world_x - 193.25).abs() < 0.001);
        assert!((world_y - 87.5).abs() < 0.001);
    }

    #[test]
    fn inspection_pointer_maps_to_a_clamped_world_cell() {
        let rect = Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(210.0, 120.0));
        assert_eq!(
            cell_at(
                Pos2::new(110.0, 70.0),
                rect,
                CameraView {
                    x: 5.0,
                    y: 0.0,
                    width: 40.0,
                    height: 20.0,
                },
                40,
                20,
            ),
            (25, 10)
        );
        assert_eq!(
            cell_at(
                Pos2::new(500.0, 500.0),
                rect,
                CameraView {
                    x: 5.0,
                    y: 0.0,
                    width: 40.0,
                    height: 20.0,
                },
                40,
                20,
            ),
            (39, 19)
        );
    }
}
