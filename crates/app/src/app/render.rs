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
        let image = region_image(
            &app.world,
            region,
            app.overlay,
            app.level.ambient_humidity,
            app.sim_seconds,
        );
        app.world_texture
            .set_partial([region.x, region.y], image, TextureOptions::NEAREST);
        uploaded += 1;
    }
    app.max_dirty_regions_uploaded = app.max_dirty_regions_uploaded.max(uploaded);
}

fn region_image(
    world: &World,
    region: DirtyRegion,
    overlay: Overlay,
    ambient_humidity: u8,
    sim_seconds: f64,
) -> ColorImage {
    let cloud = world.rules().id("cloud").unwrap_or(usize::MAX);
    let rain = world.rules().id("rain").unwrap_or(usize::MAX);
    let cloud_fade = cloud_fade(sim_seconds);
    let mut image = ColorImage::new([region.width, region.height], Color32::BLACK);
    for (y, row) in image.pixels.chunks_exact_mut(region.width).enumerate() {
        for (x, pixel) in row.iter_mut().enumerate() {
            let index = (region.y + y) * world.width() + region.x + x;
            let cell = world.cells()[index];
            if overlay == Overlay::None {
                if let Some(element) = world.rules().elements.get(cell.element) {
                    let base =
                        Color32::from_rgb(element.color[0], element.color[1], element.color[2]);
                    let world_x = region.x + x;
                    let world_y = region.y + y;
                    *pixel = if element.name == "cloud" {
                        cloud_color(
                            world,
                            world_x,
                            world_y,
                            base,
                            cloud,
                            ambient_humidity,
                            cloud_fade,
                        )
                    } else if element.name == "air" {
                        rain_streak(world, world_x, world_y, rain).unwrap_or_else(|| {
                            cloud_layer(world, world_x, world_y, base, ambient_humidity, cloud_fade)
                        })
                    } else if element.name == "rain" {
                        rain_streak(world, world_x, world_y, rain).unwrap_or(base)
                    } else {
                        base
                    };
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

fn cloud_color(
    world: &World,
    x: usize,
    y: usize,
    base: Color32,
    cloud: usize,
    ambient_humidity: u8,
    cloud_fade: f32,
) -> Color32 {
    let mut neighbors = 0u8;
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            if (dx != 0 || dy != 0)
                && let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                && nx < world.width()
                && ny < world.height()
                && world.cells()[ny * world.width() + nx].element == cloud
            {
                neighbors += 1;
            }
        }
    }
    // Hide isolated coarse cloud particles inside the interpolated, softer cloud layer.
    if neighbors < 2 {
        let air = world.rules().id("air").unwrap_or(usize::MAX);
        if let Some(element) = world.rules().elements.get(air) {
            return cloud_layer(
                world,
                x,
                y,
                Color32::from_rgb(element.color[0], element.color[1], element.color[2]),
                ambient_humidity,
                cloud_fade,
            );
        }
    }
    // Clouds darken as local vapor is depleted; clustered cells form softer edges.
    let humidity = world.atmosphere_at(x, y).map_or(45.0, |air| air.humidity);
    let maturity = ((100.0 - humidity) / 100.0).clamp(0.0, 1.0);
    let density = neighbors as f32 / 8.0;
    let shade = 1.0 - maturity * 0.24 + density * 0.08;
    Color32::from_rgb(
        (f32::from(base.r()) * shade).clamp(0.0, 255.0) as u8,
        (f32::from(base.g()) * shade).clamp(0.0, 255.0) as u8,
        (f32::from(base.b()) * shade).clamp(0.0, 255.0) as u8,
    )
}

fn cloud_layer(
    world: &World,
    x: usize,
    y: usize,
    base: Color32,
    ambient_humidity: u8,
    cloud_fade: f32,
) -> Color32 {
    let humidity = interpolated_humidity(world, x, y);
    if humidity < 45.0 {
        return base;
    }
    let (coverage, heaviness) = cloud_weather_profile(humidity, ambient_humidity, cloud_fade);
    let noise = cloud_noise(x as f32, y as f32);
    let edge = (coverage * 0.24).max(0.018);
    let mask = ((coverage + edge - noise) / edge).clamp(0.0, 1.0);
    let amount = mask * 0.72;
    if amount <= 0.0 {
        return base;
    }

    // High vapor loads create shaded cloud bodies; vapor depletion softens and clears them.
    let cloud_color = [
        221.0 - heaviness * 75.0,
        229.0 - heaviness * 72.0,
        239.0 - heaviness * 65.0,
    ];
    Color32::from_rgb(
        (f32::from(base.r()) * (1.0 - amount) + cloud_color[0] * amount) as u8,
        (f32::from(base.g()) * (1.0 - amount) + cloud_color[1] * amount) as u8,
        (f32::from(base.b()) * (1.0 - amount) + cloud_color[2] * amount) as u8,
    )
}

fn cloud_coverage(humidity: f32) -> f32 {
    if humidity < 45.0 {
        0.0
    } else {
        (0.10 + (humidity - 50.0).max(0.0) / 50.0 * 0.62).clamp(0.0, 0.72)
    }
}

fn cloud_fade(sim_seconds: f64) -> f32 {
    2.0f32.powf(-(sim_seconds.max(0.0) as f32) / 60.0)
}

fn cloud_weather_profile(humidity: f32, ambient_humidity: u8, reservoir_fade: f32) -> (f32, f32) {
    let local_coverage = cloud_coverage(humidity);
    let reservoir_coverage = if ambient_humidity > 50 {
        cloud_coverage(f32::from(ambient_humidity)) * humidity / f32::from(ambient_humidity)
            * reservoir_fade
    } else {
        0.0
    };
    let coverage = local_coverage.max(reservoir_coverage);
    let source_heaviness = if ambient_humidity > 50 {
        (f32::from(ambient_humidity - 50) / 50.0 * humidity / f32::from(ambient_humidity)
            * reservoir_fade)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let heaviness = ((humidity - 58.0) / 42.0)
        .clamp(0.0, 1.0)
        .max(source_heaviness);
    (coverage, heaviness)
}

fn interpolated_humidity(world: &World, x: usize, y: usize) -> f32 {
    let cell = 8usize;
    let x0 = x / cell * cell;
    let y0 = y / cell * cell;
    let x1 = (x0 + cell).min(world.width() - 1);
    let y1 = (y0 + cell).min(world.height() - 1);
    let tx = (x - x0) as f32 / cell as f32;
    let ty = (y - y0) as f32 / cell as f32;
    let sample = |sx, sy| world.atmosphere_at(sx, sy).map_or(0.0, |air| air.humidity);
    let top = sample(x0, y0) * (1.0 - tx) + sample(x1, y0) * tx;
    let bottom = sample(x0, y1) * (1.0 - tx) + sample(x1, y1) * tx;
    top * (1.0 - ty) + bottom * ty
}

fn cloud_noise(x: f32, y: f32) -> f32 {
    // Smooth interpolated value noise on a broad scale, independent of the atmosphere grid.
    let x = x / 18.0;
    let y = y / 11.0;
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let smooth = |v: f32| v * v * (3.0 - 2.0 * v);
    let tx = smooth(x - ix as f32);
    let ty = smooth(y - iy as f32);
    let value = |px: i32, py: i32| {
        let mut hash = (px as u32).wrapping_mul(0x9e37_79b9)
            ^ (py as u32).wrapping_mul(0x85eb_ca6b)
            ^ 0x6a09_e667;
        hash ^= hash >> 16;
        hash = hash.wrapping_mul(0x7feb_352d);
        hash ^= hash >> 15;
        hash = hash.wrapping_mul(0x846c_a68b);
        hash ^= hash >> 16;
        hash as f32 / u32::MAX as f32
    };
    let top = value(ix, iy) * (1.0 - tx) + value(ix + 1, iy) * tx;
    let bottom = value(ix, iy + 1) * (1.0 - tx) + value(ix + 1, iy + 1) * tx;
    let broad = top * (1.0 - ty) + bottom * ty;
    broad.clamp(0.0, 1.0)
}

fn rain_streak(world: &World, x: usize, y: usize, rain: usize) -> Option<Color32> {
    let humidity = world.atmosphere_at(x, y).map_or(0.0, |air| air.humidity);
    if humidity < 45.0 {
        return None;
    }
    let streak_length = 14usize;
    for offset in 0..=streak_length {
        let Some(row) = y.checked_add(offset).filter(|row| *row < world.height()) else {
            break;
        };
        if world.cells()[row * world.width() + x].element == rain {
            return Some(Color32::from_rgb(95, 174, 226));
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;
    use tinderbox_sim::{DEFAULT_RULES, Rules};

    fn cloud_coverage_at(humidity: u8, sim_seconds: f64) -> f32 {
        let mut world = World::new(256, 192, 2026, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
        world.set_ambient(20, humidity, 21);
        let air = world.rules().id("air").unwrap();
        let base = Color32::from_rgb(
            world.rules().elements[air].color[0],
            world.rules().elements[air].color[1],
            world.rules().elements[air].color[2],
        );
        let image = region_image(
            &world,
            DirtyRegion {
                x: 0,
                y: 0,
                width: world.width(),
                height: world.height(),
            },
            Overlay::None,
            humidity,
            sim_seconds,
        );
        image.pixels.iter().filter(|pixel| **pixel != base).count() as f32
            / image.pixels.len() as f32
    }

    #[test]
    fn humidity_drives_soft_cloud_coverage_without_coarse_grid_artifacts() {
        let dry = cloud_coverage_at(0, 0.0);
        let occasional = cloud_coverage_at(50, 0.0);
        let heavy = cloud_coverage_at(100, 0.0);
        let early_weather = cloud_weather_profile(50.0, 100, cloud_fade(10.0)).0;
        let thinning = cloud_weather_profile(50.0, 100, cloud_fade(30.0)).0;
        let rained_out = cloud_weather_profile(50.0, 100, cloud_fade(120.0)).0;
        assert_eq!(dry, 0.0, "zero humidity must render clear sky");
        assert!(
            occasional > 0.0,
            "50% humidity should render occasional clouds"
        );
        assert!(
            heavy > 0.5,
            "100% humidity should cover more than half the sky, got {:.1}%",
            heavy * 100.0
        );
        assert!(
            occasional < heavy,
            "50% humidity should cover less sky than 100%"
        );
        assert!(
            early_weather > thinning && thinning > rained_out,
            "cloud cover should thin over simulation time: {:.1}% at 10 seconds, {:.1}% at 30 seconds, {:.1}% at 120 seconds",
            early_weather * 100.0,
            thinning * 100.0,
            rained_out * 100.0
        );
    }
}
