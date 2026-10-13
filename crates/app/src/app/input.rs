//! Keyboard and pointer mapping for player tools.
use super::Tinderbox;
use eframe::egui::{self, Pos2, Rect, Response};

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
}

pub(super) fn handle_canvas_pointer(app: &mut Tinderbox, response: &Response, rect: Rect) {
    if app.level_setup.open || !response.hovered() {
        return;
    }
    let Some(pos) = response.interact_pointer_pos() else {
        return;
    };
    if response.dragged_by(egui::PointerButton::Primary)
        || response.clicked_by(egui::PointerButton::Primary)
    {
        if app.gust_mode {
            gust_at(app, pos, rect);
        } else {
            paint_at(app, pos, rect, app.selected);
        }
    }
    if response.clicked_by(egui::PointerButton::Secondary) {
        let fire = app.world.rules().id("fire").unwrap_or(app.selected);
        paint_at(app, pos, rect, fire);
    }
}

fn gust_at(app: &mut Tinderbox, pos: Pos2, rect: Rect) {
    let x = ((pos.x - rect.left()) / rect.width() * app.world.width() as f32) as usize;
    let y = ((pos.y - rect.top()) / rect.height() * app.world.height() as f32) as usize;
    let wind = app.world.wind();
    app.world
        .gust(x, y, app.brush / 2, wind.direction, wind.strength);
}

fn paint_at(app: &mut Tinderbox, pos: Pos2, rect: Rect, element: usize) {
    let x = ((pos.x - rect.left()) / rect.width() * app.world.width() as f32) as usize;
    let y = ((pos.y - rect.top()) / rect.height() * app.world.height() as f32) as usize;
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
