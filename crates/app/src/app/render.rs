//! Cell-grid drawing helpers. egui's native renderer owns GPU upload and presentation.
use eframe::egui::{Color32, Painter, Rect, Vec2};
use tinderbox_sim::World;

pub(super) fn fitted_size(world: &World, available: Vec2) -> Vec2 {
    let cell = (available.x / world.width() as f32)
        .min(available.y / world.height() as f32)
        .max(1.0);
    Vec2::new(cell * world.width() as f32, cell * world.height() as f32)
}

pub(super) fn draw_world(world: &World, painter: &Painter, rect: Rect) {
    let cell_size = rect.width() / world.width() as f32;
    let air = world.rules().id("air").unwrap_or(0);
    for (i, cell) in world.cells().iter().enumerate() {
        if cell.element == air {
            continue;
        }
        let Some(element) = world.rules().elements.get(cell.element) else {
            continue;
        };
        let x = i % world.width();
        let y = i / world.width();
        let min = rect.min + Vec2::new(x as f32 * cell_size, y as f32 * cell_size);
        painter.rect_filled(
            Rect::from_min_size(min, Vec2::splat(cell_size + 0.35)),
            0.0,
            Color32::from_rgb(element.color[0], element.color[1], element.color[2]),
        );
    }
}
