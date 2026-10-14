//! Incremental cell texture uploads and canvas geometry.
use crate::app::{MAX_TEXTURE_REGIONS_PER_FRAME, Overlay, Tinderbox};
use eframe::egui::{
    Color32, ColorImage, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, TextureHandle,
    TextureOptions, Vec2,
};
use tinderbox_sim::{DirtyRegion, Motion, World};

pub(super) fn fitted_size(world: &World, available: Vec2) -> Vec2 {
    let cell = (available.x / world.width() as f32)
        .min(available.y / world.height() as f32)
        .max(0.01);
    Vec2::new(cell * world.width() as f32, cell * world.height() as f32)
}

/// Uploads only changed regions, preserving excess work for later frames.
pub(super) fn upload_dirty(app: &mut Tinderbox) -> Vec<bool> {
    let sky = sky_mask(&app.world);
    let skyline = smoothed_skyline(&app.world);
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
            &sky,
            &skyline,
        );
        app.world_texture
            .set_partial([region.x, region.y], image, TextureOptions::NEAREST);
        uploaded += 1;
    }
    app.max_dirty_regions_uploaded = app.max_dirty_regions_uploaded.max(uploaded);
    sky
}

fn region_image(
    world: &World,
    region: DirtyRegion,
    overlay: Overlay,
    ambient_humidity: u8,
    sim_seconds: f64,
    sky: &[bool],
    skyline: &[f32],
) -> ColorImage {
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
                    let cloud_params = CloudRenderParams {
                        ambient_humidity,
                        cloud_fade,
                        sim_seconds,
                        altitude: cloud_altitude_weight(world_y, skyline[world_x]),
                    };
                    let life = life_sprite(&element.name);
                    *pixel = if life.is_some()
                        && !has_sky_neighbor(world_x, world_y, world.width(), world.height(), sky)
                    {
                        buried_life_color(&element.name)
                    } else if life == Some(LifeSprite::Fungus)
                        && !fungus_cap_anchor(world, world_x, world_y, sky)
                    {
                        Color32::from_rgb(157, 148, 127)
                    } else if element.name == "cloud" {
                        if !sky[index] {
                            let air = world.rules().id("air").unwrap();
                            let color = world.rules().elements[air].color;
                            Color32::from_rgb(color[0], color[1], color[2])
                        } else {
                            cloud_color(world, world_x, world_y, base, cloud_params)
                        }
                    } else if element.name == "air" {
                        if !sky[index] {
                            base
                        } else {
                            rain_streak(world, world_x, world_y, rain).unwrap_or_else(|| {
                                cloud_layer(world, world_x, world_y, base, cloud_params)
                            })
                        }
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

#[derive(Clone, Copy)]
struct CloudRenderParams {
    ambient_humidity: u8,
    cloud_fade: f32,
    sim_seconds: f64,
    altitude: f32,
}

fn cloud_color(
    world: &World,
    x: usize,
    y: usize,
    base: Color32,
    params: CloudRenderParams,
) -> Color32 {
    let cloud = world.rules().id("cloud").unwrap_or(usize::MAX);
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
    let air = world.rules().id("air").unwrap_or(usize::MAX);
    let air_color = world.rules().elements[air].color;
    let clear = Color32::from_rgb(air_color[0], air_color[1], air_color[2]);
    if params.altitude <= 0.0 {
        return clear;
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
                params,
            );
        }
    }
    // Clouds darken as local vapor is depleted; clustered cells form softer edges.
    let humidity = world.atmosphere_at(x, y).map_or(45.0, |air| air.humidity);
    let maturity = ((100.0 - humidity) / 100.0).clamp(0.0, 1.0);
    let heaviness = cloud_weather_profile(humidity, params.ambient_humidity, params.cloud_fade).1;
    let density = neighbors as f32 / 8.0;
    let core = ((density - 0.1) / 0.7).clamp(0.0, 1.0);
    let shade = 1.0 - maturity * 0.12 - heaviness * 0.58 * core + density * 0.04;
    mix_color(
        clear,
        Color32::from_rgb(
            (f32::from(base.r()) * shade).clamp(0.0, 255.0) as u8,
            (f32::from(base.g()) * shade).clamp(0.0, 255.0) as u8,
            (f32::from(base.b()) * shade).clamp(0.0, 255.0) as u8,
        ),
        params.altitude,
    )
}

fn cloud_layer(
    world: &World,
    x: usize,
    y: usize,
    base: Color32,
    params: CloudRenderParams,
) -> Color32 {
    let humidity = interpolated_humidity(world, x, y);
    if humidity < 45.0 {
        return base;
    }
    let (coverage, heaviness) =
        cloud_weather_profile(humidity, params.ambient_humidity, params.cloud_fade);
    let offset = cloud_drift_offset(world.wind(), params.sim_seconds);
    let coarse_cells = world.width() < 160 || world.height() < 100;
    let mask = cloud_cell_mask(x as f32, y as f32, offset, coverage, coarse_cells);
    let amount = mask * 0.72 * params.altitude;
    if amount <= 0.0 {
        return base;
    }

    // High vapor loads create shaded cloud bodies; vapor depletion softens and clears them.
    let cloud_color = heavy_cloud_color(Color32::from_rgb(224, 231, 240), heaviness, mask);
    Color32::from_rgb(
        (f32::from(base.r()) * (1.0 - amount) + f32::from(cloud_color.r()) * amount) as u8,
        (f32::from(base.g()) * (1.0 - amount) + f32::from(cloud_color.g()) * amount) as u8,
        (f32::from(base.b()) * (1.0 - amount) + f32::from(cloud_color.b()) * amount) as u8,
    )
}

fn cloud_mask(noise: f32, coverage: f32) -> f32 {
    let edge = (coverage * 0.65).max(0.045);
    let transition = ((coverage + edge * 0.5 - noise) / edge).clamp(0.0, 1.0);
    transition * transition * (3.0 - 2.0 * transition)
}

fn cloud_cell_mask(x: f32, y: f32, offset: f32, coverage: f32, coarse_cells: bool) -> f32 {
    if !coarse_cells {
        return cloud_mask(cloud_noise(x - offset, y), coverage);
    }
    const SAMPLES: [(f32, f32); 4] = [(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)];
    SAMPLES
        .iter()
        .map(|(dx, dy)| cloud_mask(cloud_noise(x + dx - offset, y + dy), coverage))
        .sum::<f32>()
        / SAMPLES.len() as f32
}

fn heavy_cloud_color(base: Color32, heaviness: f32, core: f32) -> Color32 {
    let darkness = (heaviness * core * 1.8).clamp(0.0, 1.0);
    let dark_slate = Color32::from_rgb(86, 99, 120);
    mix_color(base, dark_slate, darkness)
}

fn mix_color(background: Color32, foreground: Color32, amount: f32) -> Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (f32::from(a) * (1.0 - amount) + f32::from(b) * amount).round() as u8;
    Color32::from_rgb(
        mix(background.r(), foreground.r()),
        mix(background.g(), foreground.g()),
        mix(background.b(), foreground.b()),
    )
}

fn cloud_drift_offset(wind: tinderbox_sim::Wind, sim_seconds: f64) -> f32 {
    let horizontal = match wind.direction {
        tinderbox_sim::WindDirection::West => -1.0,
        tinderbox_sim::WindDirection::Calm => 0.0,
        tinderbox_sim::WindDirection::East => 1.0,
    };
    let wind_speed = horizontal * f32::from(wind.strength.min(4)) * 2.0;
    let calm_evolution = 0.15;
    (wind_speed + calm_evolution) * sim_seconds.max(0.0) as f32
}

fn smoothed_skyline(world: &World) -> Vec<f32> {
    let raw = (0..world.width())
        .map(|x| {
            (0..world.height())
                .find(|&y| {
                    let element =
                        &world.rules().elements[world.cells()[y * world.width() + x].element];
                    is_skyline_obstacle(element.name.as_str(), element.motion)
                })
                .unwrap_or(world.height()) as f32
        })
        .collect::<Vec<_>>();
    smooth_skyline(&raw)
}

fn is_skyline_obstacle(name: &str, motion: Motion) -> bool {
    motion != Motion::Gas
        && !matches!(
            name,
            "air" | "rain" | "fire" | "oil_fire" | "sand" | "ash" | "seed"
        )
}

fn smooth_skyline(raw: &[f32]) -> Vec<f32> {
    let envelope_radius = 8usize;
    let smoothing_radius = 4usize;
    (0..raw.len())
        .map(|x| {
            let start = x.saturating_sub(smoothing_radius);
            let end = (x + smoothing_radius + 1).min(raw.len());
            (start..end)
                .map(|neighbor| {
                    let start = neighbor.saturating_sub(envelope_radius);
                    let end = (neighbor + envelope_radius + 1).min(raw.len());
                    raw[start..end]
                        .iter()
                        .copied()
                        .fold(f32::INFINITY, f32::min)
                })
                .sum::<f32>()
                / (end - start) as f32
        })
        .collect()
}

fn cloud_altitude_weight(y: usize, skyline_y: f32) -> f32 {
    const CLEARANCE: f32 = 10.0;
    const FADE_BAND: f32 = 24.0;
    ((skyline_y - y as f32 - CLEARANCE) / FADE_BAND).clamp(0.0, 1.0)
}

fn sky_mask(world: &World) -> Vec<bool> {
    let air = world.rules().id("air").unwrap_or(usize::MAX);
    let cloud = world.rules().id("cloud").unwrap_or(usize::MAX);
    let rain = world.rules().id("rain").unwrap_or(usize::MAX);
    sky_mask_from_cells(
        world.width(),
        world.height(),
        world.cells(),
        air,
        cloud,
        rain,
    )
}

fn sky_mask_from_cells(
    width: usize,
    height: usize,
    cells: &[tinderbox_sim::Cell],
    air: usize,
    cloud: usize,
    rain: usize,
) -> Vec<bool> {
    let is_open = |element| element == air || element == cloud || element == rain;
    let mut sky = vec![false; cells.len()];
    let mut queue = Vec::with_capacity(width);
    for x in 0..width {
        let index = x;
        let element = cells[index].element;
        if is_open(element) && !sky[index] {
            sky[index] = true;
            queue.push(index);
        }
    }
    let mut cursor = 0;
    while cursor < queue.len() {
        let index = queue[cursor];
        cursor += 1;
        let x = index % width;
        let y = index / width;
        let neighbors = [
            (x > 0).then_some(index.wrapping_sub(1)),
            (x + 1 < width).then_some(index + 1),
            (y > 0).then_some(index.wrapping_sub(width)),
            (y + 1 < height).then_some(index + width),
        ];
        for next in neighbors.into_iter().flatten() {
            let element = cells[next].element;
            if !sky[next] && is_open(element) {
                sky[next] = true;
                queue.push(next);
            }
        }
    }
    sky
}

fn cloud_coverage(humidity: f32) -> f32 {
    if humidity < 45.0 {
        0.0
    } else {
        // Ease the first occasional clouds in between 45% and 50% instead of stepping on at 45%.
        let onset = ((humidity - 45.0) / 5.0).clamp(0.0, 1.0);
        let onset = onset * onset * (3.0 - 2.0 * onset);
        (0.10 * onset + (humidity - 50.0).max(0.0) / 50.0 * 0.62).clamp(0.0, 0.72)
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

// Blending the octaves narrows the noise (standard deviation 0.181, down from 0.216 at #43). Coverage
// thresholds were tuned against the wider spread, so stretch the blend back to it.
const CLOUD_NOISE_SPREAD: f32 = 1.19;

fn cloud_noise(x: f32, y: f32) -> f32 {
    // Domain-warped isotropic value noise forms soft, irregular cloud lobes instead of streaks.
    let warp_x = (value_noise(x / 48.0, y / 48.0) - 0.5) * 14.0;
    let warp_y = (value_noise((x + 23.0) / 48.0, (y - 37.0) / 48.0) - 0.5) * 14.0;
    let broad = value_noise((x + warp_x) / 18.0, (y + warp_y) / 18.0);
    let detail = value_noise((x - 31.0) / 9.0, (y + 17.0) / 9.0);
    let blended = broad * 0.82 + detail * 0.18;
    ((blended - 0.5) * CLOUD_NOISE_SPREAD + 0.5).clamp(0.0, 1.0)
}

fn value_noise(x: f32, y: f32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let smooth = |value: f32| value * value * (3.0 - 2.0 * value);
    let tx = smooth(x - ix as f32);
    let ty = smooth(y - iy as f32);
    let top = cloud_hash(ix, iy) * (1.0 - tx) + cloud_hash(ix + 1, iy) * tx;
    let bottom = cloud_hash(ix, iy + 1) * (1.0 - tx) + cloud_hash(ix + 1, iy + 1) * tx;
    top * (1.0 - ty) + bottom * ty
}

fn cloud_hash(x: i32, y: i32) -> f32 {
    let mut hash =
        (x as u32).wrapping_mul(0x9e37_79b9) ^ (y as u32).wrapping_mul(0x85eb_ca6b) ^ 0x6a09_e667;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7feb_352d);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846c_a68b);
    hash ^= hash >> 16;
    hash as f32 / u32::MAX as f32
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

pub(super) fn draw_endless_cursor(painter: &Painter, rect: Rect) {
    let center = rect.center();
    let stroke = Stroke::new(1.5_f32, Color32::from_rgb(232, 224, 182));
    painter.circle_stroke(center, 6.0, stroke);
    painter.line_segment(
        [
            Pos2::new(center.x - 10.0, center.y),
            Pos2::new(center.x + 10.0, center.y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(center.x, center.y - 10.0),
            Pos2::new(center.x, center.y + 10.0),
        ],
        stroke,
    );
}

pub(super) fn draw_world(
    texture: &TextureHandle,
    painter: &Painter,
    rect: Rect,
    world_width: usize,
    camera_x: f32,
    view_width: f32,
) {
    let texture_width = texture.size()[0] as f32;
    let left = (camera_x / texture_width).clamp(0.0, 1.0);
    let right =
        ((camera_x + view_width) / texture_width).clamp(left, world_width as f32 / texture_width);
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(Pos2::new(left, 0.0), eframe::egui::pos2(right, 1.0)),
        Color32::WHITE,
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifeSprite {
    Beetle,
    Termite,
    Fungus,
}

fn life_sprite(name: &str) -> Option<LifeSprite> {
    match name {
        "beetle" => Some(LifeSprite::Beetle),
        "termite" => Some(LifeSprite::Termite),
        "fungus" => Some(LifeSprite::Fungus),
        _ => None,
    }
}

fn buried_life_color(name: &str) -> Color32 {
    match life_sprite(name) {
        Some(LifeSprite::Beetle) => Color32::from_rgb(88, 68, 49),
        Some(LifeSprite::Termite) => Color32::from_rgb(105, 88, 61),
        Some(LifeSprite::Fungus) => Color32::from_rgb(139, 129, 108),
        None => Color32::TRANSPARENT,
    }
}

fn fungus_cap_anchor(world: &World, x: usize, y: usize, sky: &[bool]) -> bool {
    if y == 0 || !sky[(y - 1) * world.width() + x] {
        return false;
    }
    let fungus = world.rules().id("fungus").unwrap_or(usize::MAX);
    x == 0
        || world.cells()[y * world.width() + x - 1].element != fungus
        || !sky[(y - 1) * world.width() + x - 1]
}

fn has_sky_neighbor(x: usize, y: usize, width: usize, height: usize, sky: &[bool]) -> bool {
    let x0 = x.saturating_sub(1);
    let x1 = (x + 1).min(width.saturating_sub(1));
    let y0 = y.saturating_sub(1);
    let y1 = (y + 1).min(height.saturating_sub(1));
    (y0..=y1).any(|ny| (x0..=x1).any(|nx| (nx != x || ny != y) && sky[ny * width + nx]))
}

pub(super) fn draw_life_sprites(
    world: &World,
    painter: &Painter,
    rect: Rect,
    overlay: Overlay,
    sky: &[bool],
    camera_x: f32,
    view_width: f32,
) {
    if overlay != Overlay::None {
        return;
    }
    let cell_width = rect.width() / view_width.max(1.0);
    let cell_height = rect.height() / world.height() as f32;
    let x_start = camera_x.floor().max(0.0) as usize;
    let x_end = (camera_x + view_width).ceil().min(world.width() as f32) as usize;
    for y in 0..world.height() {
        for x in x_start..x_end {
            let index = y * world.width() + x;
            let cell = &world.cells()[index];
            let name = &world.rules().elements[cell.element].name;
            let Some(kind) = life_sprite(name) else {
                continue;
            };
            let exposed = has_sky_neighbor(x, y, world.width(), world.height(), sky);
            if !exposed || (kind == LifeSprite::Fungus && !fungus_cap_anchor(world, x, y, sky)) {
                continue;
            }
            let center = Pos2::new(
                rect.left() + (x as f32 + 0.5 - camera_x) * cell_width,
                rect.top() + (y as f32 + 0.5) * cell_height,
            );
            match kind {
                LifeSprite::Beetle => draw_beetle(painter, center, cell_width, cell_height),
                LifeSprite::Termite => draw_termite(painter, center, cell_width, cell_height),
                LifeSprite::Fungus => draw_fungus(painter, center, cell_width, cell_height),
            }
        }
    }
}

fn sprite_rect(center: Pos2, cell_width: f32, cell_height: f32, width: f32, height: f32) -> Rect {
    let size = Vec2::new(cell_width * width, cell_height * height);
    Rect::from_center_size(center, size)
}

fn draw_beetle(painter: &Painter, center: Pos2, cell_width: f32, cell_height: f32) {
    let shell = Color32::from_rgb(160, 102, 59);
    let highlight = Color32::from_rgb(206, 149, 91);
    let outline = Color32::from_rgb(52, 43, 35);
    let body = sprite_rect(center, cell_width, cell_height, 1.15, 0.78);
    painter.add(Shape::ellipse_filled(
        body.center(),
        body.size() * 0.5,
        shell,
    ));
    painter.add(Shape::ellipse_stroke(
        body.center(),
        body.size() * 0.5,
        Stroke::new((cell_width * 0.10).max(0.45), outline),
    ));
    painter.line_segment(
        [
            Pos2::new(body.left() + body.width() * 0.56, body.top()),
            Pos2::new(body.left() + body.width() * 0.56, body.bottom()),
        ],
        Stroke::new((cell_width * 0.07).max(0.35), highlight),
    );
    let head = sprite_rect(
        Pos2::new(body.right() - body.width() * 0.02, center.y),
        cell_width,
        cell_height,
        0.42,
        0.42,
    );
    painter.circle_filled(head.center(), head.width() * 0.5, highlight);
    painter.circle_stroke(
        head.center(),
        head.width() * 0.5,
        Stroke::new((cell_width * 0.08).max(0.35), outline),
    );
    for fraction in [0.24, 0.5, 0.76] {
        let x = body.left() + body.width() * fraction;
        for sign in [-1.0, 1.0] {
            painter.line_segment(
                [
                    Pos2::new(x, center.y),
                    Pos2::new(x + cell_width * 0.18, center.y + sign * cell_height * 0.48),
                ],
                Stroke::new((cell_width * 0.07).max(0.35), outline),
            );
        }
    }
}

fn draw_termite(painter: &Painter, center: Pos2, cell_width: f32, cell_height: f32) {
    let body_color = Color32::from_rgb(190, 174, 117);
    let segment = Color32::from_rgb(224, 210, 159);
    let outline = Color32::from_rgb(54, 49, 39);
    let body = sprite_rect(center, cell_width, cell_height, 1.48, 0.60);
    painter.add(Shape::ellipse_filled(
        body.center(),
        body.size() * 0.5,
        body_color,
    ));
    painter.add(Shape::ellipse_stroke(
        body.center(),
        body.size() * 0.5,
        Stroke::new((cell_width * 0.09).max(0.4), outline),
    ));
    for fraction in [0.38, 0.61, 0.79] {
        let x = body.left() + body.width() * fraction;
        painter.line_segment(
            [
                Pos2::new(x, body.top() + body.height() * 0.12),
                Pos2::new(x, body.bottom() - body.height() * 0.12),
            ],
            Stroke::new((cell_width * 0.07).max(0.35), segment),
        );
    }
    let head = Pos2::new(body.left() + body.width() * 0.09, center.y);
    painter.circle_filled(head, cell_width * 0.16, segment);
    painter.circle_stroke(
        head,
        cell_width * 0.16,
        Stroke::new((cell_width * 0.07).max(0.35), outline),
    );
    for sign in [-1.0, 1.0] {
        painter.line_segment(
            [
                head,
                Pos2::new(
                    head.x - cell_width * 0.24,
                    head.y + sign * cell_height * 0.35,
                ),
            ],
            Stroke::new((cell_width * 0.06).max(0.3), outline),
        );
    }
}

fn draw_fungus(painter: &Painter, center: Pos2, cell_width: f32, cell_height: f32) {
    let cap = Color32::from_rgb(219, 207, 178);
    let stem = Color32::from_rgb(198, 185, 157);
    let outline = Color32::from_rgb(89, 75, 59);
    let stem_rect = sprite_rect(
        Pos2::new(center.x, center.y + cell_height * 0.16),
        cell_width,
        cell_height,
        0.30,
        0.54,
    );
    painter.rect_filled(stem_rect, cell_width * 0.12, stem);
    painter.rect_stroke(
        stem_rect,
        cell_width * 0.12,
        Stroke::new((cell_width * 0.08).max(0.35), outline),
        StrokeKind::Middle,
    );
    let cap_rect = sprite_rect(
        Pos2::new(center.x, center.y - cell_height * 0.12),
        cell_width,
        cell_height,
        1.12,
        0.58,
    );
    painter.add(Shape::ellipse_filled(
        cap_rect.center(),
        cap_rect.size() * 0.5,
        cap,
    ));
    painter.add(Shape::ellipse_stroke(
        cap_rect.center(),
        cap_rect.size() * 0.5,
        Stroke::new((cell_width * 0.10).max(0.4), outline),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tinderbox_sim::{
        Cell, DEFAULT_RULES, Rules, Wind, WindDirection,
        level::{Generator, Level, LevelMode},
    };

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
            &sky_mask(&world),
            &smoothed_skyline(&world),
        );
        image.pixels.iter().filter(|pixel| **pixel != base).count() as f32
            / image.pixels.len() as f32
    }

    #[test]
    fn sky_mask_excludes_below_surface_and_enclosed_air() {
        let (width, height) = (7, 7);
        let (air, solid) = (0, 1);
        let mut cells = vec![Cell::default(); width * height];
        for cell in &mut cells {
            cell.element = air;
        }
        for x in 0..width {
            cells[5 * width + x].element = solid;
        }
        // A sealed air pocket sits under the terrain ridge.
        for x in 2..=4 {
            cells[4 * width + x].element = solid;
            cells[3 * width + x].element = air;
            cells[2 * width + x].element = solid;
        }
        cells[3 * width + 2 - 1].element = solid;
        cells[3 * width + 4 + 1].element = solid;
        let sky = sky_mask_from_cells(width, height, &cells, air, 2, 3);
        assert!(!sky[6 * width + 3], "cells below the surface are not sky");
        assert!(!sky[3 * width + 3], "enclosed air is not sky");
        assert!(sky[width + 3], "open air connected to the top is sky");
    }

    #[test]
    fn transient_weather_does_not_raise_the_cloud_skyline() {
        assert!(!is_skyline_obstacle("air", Motion::Static));
        assert!(!is_skyline_obstacle("cloud", Motion::Gas));
        assert!(!is_skyline_obstacle("steam", Motion::Gas));
        assert!(!is_skyline_obstacle("smoke", Motion::Gas));
        assert!(!is_skyline_obstacle("rain", Motion::Fall));
        assert!(!is_skyline_obstacle("fire", Motion::Static));
        assert!(is_skyline_obstacle("tree", Motion::Static));
        assert!(is_skyline_obstacle("wood", Motion::Fall));
        assert!(is_skyline_obstacle("water", Motion::Liquid));
        assert!(is_skyline_obstacle("stone", Motion::Static));
    }

    #[test]
    fn skyline_smoothing_and_altitude_fade_keep_clouds_above_obstacles() {
        let smoothed = smooth_skyline(&[40.0, 40.0, 10.0, 40.0, 40.0]);
        assert!(smoothed.iter().all(|height| *height == 10.0));
        assert_eq!(cloud_altitude_weight(30, 40.0), 0.0);
        assert_eq!(cloud_altitude_weight(0, 40.0), 1.0);
        assert!(cloud_altitude_weight(20, 40.0) > 0.0);
    }

    #[test]
    fn coarse_cell_cloud_mask_averages_subcell_coverage() {
        let (x, y, offset, coverage) = (34.0, 18.0, 7.5, 0.42);
        let coarse = cloud_cell_mask(x, y, offset, coverage, true);
        let fine = cloud_cell_mask(x, y, offset, coverage, false);
        assert!((0.0..=1.0).contains(&coarse));
        assert_ne!(
            coarse, fine,
            "coarse rendering should integrate subcell edges"
        );
    }

    #[test]
    fn cloud_mask_has_a_soft_transition_at_both_edges() {
        let coverage = 0.1;
        let edge = (coverage * 0.65f32).max(0.045);
        assert_eq!(cloud_mask(coverage - edge * 0.5, coverage), 1.0);
        assert_eq!(cloud_mask(coverage + edge * 0.5, coverage), 0.0);
        assert!((cloud_mask(coverage, coverage) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn heavy_cloud_cores_are_darker_than_edges_and_light_clouds() {
        let light = Color32::from_rgb(224, 231, 240);
        let light_core = heavy_cloud_color(light, 0.0, 1.0);
        let heavy_edge = heavy_cloud_color(light, 0.7, 0.25);
        let heavy_core = heavy_cloud_color(light, 0.7, 1.0);
        let luminance = |color: Color32| {
            0.2126 * f32::from(color.r())
                + 0.7152 * f32::from(color.g())
                + 0.0722 * f32::from(color.b())
        };
        assert!(luminance(heavy_core) < luminance(heavy_edge));
        assert!(luminance(heavy_edge) < luminance(light_core));
    }

    #[test]
    fn critter_sprites_are_species_specific_and_buried_colors_are_subdued() {
        assert_eq!(life_sprite("beetle"), Some(LifeSprite::Beetle));
        assert_eq!(life_sprite("termite"), Some(LifeSprite::Termite));
        assert_eq!(life_sprite("fungus"), Some(LifeSprite::Fungus));
        assert_eq!(life_sprite("soil"), None);

        let beetle = buried_life_color("beetle");
        let termite = buried_life_color("termite");
        let fungus = buried_life_color("fungus");
        for color in [beetle, termite, fungus] {
            assert!(color.r() < 145 && color.g() < 140 && color.b() < 120);
        }
        assert_ne!(beetle, termite);
        assert_ne!(beetle, fungus);
        assert_ne!(termite, fungus);
        assert_eq!(buried_life_color("soil"), Color32::TRANSPARENT);
    }

    #[test]
    fn buried_life_pixels_are_muted_while_surface_life_keeps_its_base_color() {
        let mut world = World::new(5, 5, 7, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
        let soil = world.rules().id("soil").unwrap();
        let air = world.rules().id("air").unwrap();
        let beetle = world.rules().id("beetle").unwrap();
        let termite = world.rules().id("termite").unwrap();
        for y in 0..world.height() {
            for x in 0..world.width() {
                world.paint(x, y, soil);
            }
        }
        for x in 0..world.width() {
            world.paint(x, 0, air);
        }
        world.paint(2, 2, beetle);
        world.paint(1, 1, termite);

        let region = DirtyRegion {
            x: 0,
            y: 0,
            width: world.width(),
            height: world.height(),
        };
        let sky = sky_mask(&world);
        let image = region_image(
            &world,
            region,
            Overlay::None,
            0,
            0.0,
            &sky,
            &smoothed_skyline(&world),
        );
        assert_eq!(
            image.pixels[2 * world.width() + 2],
            buried_life_color("beetle")
        );
        assert_eq!(
            image.pixels[world.width() + 1],
            Color32::from_rgb(151, 124, 78)
        );
    }

    #[test]
    fn fungus_caps_mark_only_the_top_of_each_horizontal_colony() {
        let mut world = World::new(5, 4, 7, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
        let fungus = world.rules().id("fungus").unwrap();
        world.paint(1, 1, fungus);
        world.paint(2, 1, fungus);
        let sky = sky_mask(&world);

        assert!(fungus_cap_anchor(&world, 1, 1, &sky));
        assert!(!fungus_cap_anchor(&world, 2, 1, &sky));
        assert!(!fungus_cap_anchor(&world, 1, 2, &sky));
    }

    #[test]
    fn sprite_visibility_requires_neighboring_open_sky_and_handles_edges() {
        let (width, height) = (5, 4);
        let mut sky = vec![false; width * height];
        sky[width + 1] = true;

        assert!(has_sky_neighbor(2, 1, width, height, &sky));
        assert!(has_sky_neighbor(0, 0, width, height, &sky));
        assert!(!has_sky_neighbor(4, 3, width, height, &sky));
        assert!(!has_sky_neighbor(1, 1, width, height, &[false; 20]));
    }

    #[test]
    fn cloud_layer_changes_over_simulation_time() {
        let mut world = World::new(128, 96, 2026, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
        world.set_ambient(20, 50, 21);
        world.set_wind(WindDirection::East, 1);
        let sky = sky_mask(&world);
        let skyline = smoothed_skyline(&world);
        let region = DirtyRegion {
            x: 0,
            y: 0,
            width: world.width(),
            height: world.height(),
        };
        let before = region_image(&world, region, Overlay::None, 50, 0.0, &sky, &skyline);
        let after = region_image(&world, region, Overlay::None, 50, 10.0, &sky, &skyline);
        assert!(
            before
                .pixels
                .iter()
                .zip(after.pixels.iter())
                .any(|(a, b)| a != b)
        );
    }

    #[test]
    fn cloud_drift_follows_wind_sign_and_evolves_in_calm_air() {
        let east = cloud_drift_offset(
            Wind {
                direction: WindDirection::East,
                strength: 2,
            },
            10.0,
        );
        let west = cloud_drift_offset(
            Wind {
                direction: WindDirection::West,
                strength: 2,
            },
            10.0,
        );
        let calm = cloud_drift_offset(Wind::default(), 10.0);
        assert!(east > calm && calm > west);
        assert!(cloud_drift_offset(Wind::default(), 20.0) > calm);
    }

    fn weather_capture_coverage(world: &World, ambient_humidity: u8, sim_seconds: f64) -> f32 {
        const CAPTURE_X: usize = 8;
        const CAPTURE_Y: usize = 17;
        const CAPTURE_WIDTH: usize = 383;
        const CAPTURE_HEIGHT: usize = 102;
        let air = world.rules().id("air").unwrap();
        let color = world.rules().elements[air].color;
        let background = Color32::from_rgb(color[0], color[1], color[2]);
        let luminance = |pixel: Color32| {
            0.2126 * f32::from(pixel.r())
                + 0.7152 * f32::from(pixel.g())
                + 0.0722 * f32::from(pixel.b())
        };
        let image = region_image(
            world,
            DirtyRegion {
                x: CAPTURE_X,
                y: CAPTURE_Y,
                width: CAPTURE_WIDTH,
                height: CAPTURE_HEIGHT,
            },
            Overlay::None,
            ambient_humidity,
            sim_seconds,
            &sky_mask(world),
            &smoothed_skyline(world),
        );
        let sky_luminance = luminance(background);
        image
            .pixels
            .iter()
            .filter(|pixel| luminance(**pixel) > sky_luminance + 15.0)
            .count() as f32
            / image.pixels.len() as f32
    }

    fn hills_weather_level(ambient_humidity: u8) -> Level {
        let mut amounts = BTreeMap::new();
        amounts.insert("stone".into(), 100);
        amounts.insert("soil".into(), 100);
        amounts.insert("tree".into(), 75);
        Level {
            version: 1,
            width: 400,
            height: 250,
            fit_window: false,
            seed: 731,
            generator: Generator::Hills,
            mode: LevelMode::Fixed,
            wind: Wind {
                direction: WindDirection::East,
                strength: 1,
            },
            ambient_temperature: 20,
            ambient_humidity,
            ambient_oxygen: 21,
            amounts,
            spark: true,
            boat: false,
            boat_sand: 0,
            boat_burning: false,
        }
    }

    fn weather_coverage_through_thirty_seconds(ambient_humidity: u8) -> Vec<f32> {
        let mut world = hills_weather_level(ambient_humidity)
            .generate(Rules::load(DEFAULT_RULES).unwrap())
            .unwrap();
        let mut coverage = Vec::new();
        for frame in 1..=30 * 60 {
            world.step(12_000);
            if frame % 600 == 0 {
                let seconds = frame / 60;
                coverage.push(weather_capture_coverage(
                    &world,
                    ambient_humidity,
                    seconds as f64,
                ));
            }
        }
        assert_eq!(coverage.len(), 3);
        coverage
    }

    #[test]
    fn full_humidity_cloud_coverage_stays_substantial_through_thirty_seconds() {
        let coverage = weather_coverage_through_thirty_seconds(100);
        for (seconds, fraction) in [10, 20, 30].into_iter().zip(coverage) {
            assert!(
                fraction >= 0.18,
                "100% humidity cloud coverage at {seconds}s should remain substantial, got {:.1}%",
                fraction * 100.0
            );
        }
    }

    #[test]
    fn half_humidity_cloud_coverage_stays_visible_through_thirty_seconds() {
        let coverage = weather_coverage_through_thirty_seconds(50);
        for (seconds, fraction) in [10, 20, 30].into_iter().zip(coverage) {
            assert!(
                (0.03..=0.08).contains(&fraction),
                "50% humidity cloud coverage at {seconds}s should be a visible band of 3% to 8%, got {:.1}%",
                fraction * 100.0
            );
        }
    }

    #[test]
    fn cloud_coverage_rises_smoothly_with_humidity() {
        let mut previous = cloud_coverage(0.0);
        assert_eq!(previous, 0.0, "zero humidity must render clear sky");
        for step in 1..=200 {
            let humidity = step as f32 * 0.5;
            let coverage = cloud_coverage(humidity);
            assert!(
                coverage >= previous,
                "coverage should not fall as humidity rises: {:.3} at {humidity}% after {:.3}",
                coverage,
                previous
            );
            assert!(
                coverage - previous < 0.02,
                "coverage should rise without steps: {:.3} at {humidity}%",
                coverage - previous
            );
            previous = coverage;
        }
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
