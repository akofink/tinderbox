//! Incremental cell texture uploads and canvas geometry.
use crate::app::{MAX_TEXTURE_REGIONS_PER_FRAME, Overlay, Tinderbox};
use eframe::egui::{Color32, ColorImage, Painter, Rect, TextureHandle, TextureOptions, Vec2};
use tinderbox_sim::{DirtyRegion, World};

pub(super) fn fitted_size(world: &World, available: Vec2) -> Vec2 {
    let cell = (available.x / world.width() as f32)
        .min(available.y / world.height() as f32)
        .max(0.01);
    Vec2::new(cell * world.width() as f32, cell * world.height() as f32)
}

/// Uploads only changed regions, preserving excess work for later frames.
pub(super) fn upload_dirty(app: &mut Tinderbox) {
    let mut uploaded = 0;
    for _ in 0..MAX_TEXTURE_REGIONS_PER_FRAME {
        let Some(region) = app.world.pop_dirty_region() else {
            break;
        };
        let image = region_image(&app.world, region, app.overlay);
        app.world_texture
            .set_partial([region.x, region.y], image, TextureOptions::NEAREST);
        uploaded += 1;
    }
    app.max_dirty_regions_uploaded = app.max_dirty_regions_uploaded.max(uploaded);
}

fn region_image(world: &World, region: DirtyRegion, overlay: Overlay) -> ColorImage {
    let mut image = ColorImage::new([region.width, region.height], Color32::BLACK);
    for (y, row) in image.pixels.chunks_exact_mut(region.width).enumerate() {
        for (x, pixel) in row.iter_mut().enumerate() {
            let index = (region.y + y) * world.width() + region.x + x;
            let cell = world.cells()[index];
            if overlay == Overlay::None {
                if let Some(element) = world.rules().elements.get(cell.element) {
                    *pixel =
                        Color32::from_rgb(element.color[0], element.color[1], element.color[2]);
                }
            } else if let Some(atmosphere) = world.atmosphere_at(region.x + x, region.y + y) {
                let (value, low, high) = match overlay {
                    Overlay::Temperature => (atmosphere.temperature, -20.0, 200.0),
                    Overlay::Humidity => (atmosphere.humidity, 0.0, 100.0),
                    Overlay::Pressure => (atmosphere.pressure, -40.0, 40.0),
                    Overlay::Oxygen => (atmosphere.oxygen, 0.0, 21.0),
                    Overlay::Moisture => (f32::from(cell.moisture) * 100.0 / 255.0, 0.0, 100.0),
                    Overlay::BurnProgress => {
                        (f32::from(cell.burn_progress) * 100.0 / 255.0, 0.0, 100.0)
                    }
                    Overlay::None => unreachable!(),
                };
                *pixel = heatmap(value, low, high);
            }
        }
    }
    image
}

fn heatmap(value: f32, low: f32, high: f32) -> Color32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    let (r, g, b) = if t < 0.5 {
        let t = t * 2.0;
        (0.0, 80.0 + 150.0 * t, 255.0 * (1.0 - t))
    } else {
        let t = (t - 0.5) * 2.0;
        (255.0 * t, 230.0 * (1.0 - t), 0.0)
    };
    Color32::from_rgb(r as u8, g as u8, b as u8)
}

pub(super) fn draw_world(texture: &TextureHandle, painter: &Painter, rect: Rect) {
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(eframe::egui::Pos2::ZERO, eframe::egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}
