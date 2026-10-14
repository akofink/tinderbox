//! Read-only magnified view and plain-language description of the hovered cell.
use super::{Tinderbox, input};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use tinderbox_sim::{Motion, World};

const LENS_WIDTH: usize = 9;
const LENS_HEIGHT: usize = 7;
const MAGNIFICATION: f32 = 3.0;

#[derive(Clone, Debug, PartialEq)]
struct CellDescription {
    title: String,
    category: &'static str,
    lines: Vec<String>,
}

fn describe_cell(world: &World, x: usize, y: usize) -> Option<CellDescription> {
    if x >= world.width() || y >= world.height() {
        return None;
    }
    let index = y.checked_mul(world.width())?.checked_add(x)?;
    let cell = *world.cells().get(index)?;
    let element = world.rules().elements.get(cell.element)?;
    let mut lines = vec![format!("Behavior: {}", behavior_description(cell, element))];
    if element.carries_moisture || cell.moisture > 0 {
        lines.push(match cell.moisture {
            0..=20 => format!("Moisture: dry ({}%)", cell.moisture * 100 / 255),
            21..=127 => format!("Moisture: damp ({}%)", cell.moisture * 100 / 255),
            _ => format!("Moisture: wet ({}%)", cell.moisture * 100 / 255),
        });
    }
    if cell.burn_progress > 0 {
        lines.push(format!("Burning: {}%", cell.burn_progress * 100 / 255));
    }
    let age = cell.age();
    if element.lifetime > 0 {
        lines.push(format!(
            "Age: {age}; {} ticks left",
            element.lifetime.saturating_sub(age)
        ));
    } else if age > 0 {
        lines.push(format!("Age: {age} ticks"));
    }
    let atmosphere = world.atmosphere_at(x, y).unwrap_or_default();
    lines.push(format!(
        "Air: {:.0}°C, {:.0}% humidity",
        atmosphere.temperature, atmosphere.humidity
    ));
    lines.push(format!(
        "Pressure: {:.1}; oxygen: {:.0}%",
        atmosphere.pressure, atmosphere.oxygen
    ));
    let wind = world.local_wind_at(x, y).unwrap_or_else(|| world.wind());
    let direction = match wind.direction {
        tinderbox_sim::WindDirection::West => "west",
        tinderbox_sim::WindDirection::Calm => "calm",
        tinderbox_sim::WindDirection::East => "east",
    };
    lines.push(format!("Wind: {direction}, strength {}", wind.strength));

    let mut events = Vec::new();
    let mut water_neighbor = false;
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            if (dx == 0 && dy == 0) || (x == 0 && dx < 0) || (y == 0 && dy < 0) {
                continue;
            }
            let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) else {
                continue;
            };
            if nx >= world.width() || ny >= world.height() {
                continue;
            }
            let neighbor_id = world.cells()[ny * world.width() + nx].element;
            let Some(neighbor) = world.rules().elements.get(neighbor_id) else {
                continue;
            };
            water_neighbor |= neighbor.name == "water";
            for rule in &world.rules().interactions {
                if rule.enabled
                    && rule.chance > 0.0
                    && ((rule.when == cell.element && rule.with == neighbor_id)
                        || (rule.with == cell.element && rule.when == neighbor_id))
                {
                    let result =
                        crate::labels::display_name(&world.rules().elements[rule.result].name);
                    let self_result = rule
                        .self_result
                        .map(|id| crate::labels::display_name(&world.rules().elements[id].name));
                    let neighbor_name = crate::labels::display_name(&neighbor.name);
                    let is_actor = rule.when == cell.element;
                    events.push(match (is_actor, self_result) {
                        (true, Some(result_self)) if element.name == "seed" && result_self == "Plant" => {
                            format!("on {neighbor_name}: seed may sprout into {result_self}")
                        }
                        (true, Some(result_self)) => format!(
                            "next to {neighbor_name}: this cell may become {result_self}; neighbor becomes {result}"
                        ),
                        (_, _) => format!("next to {neighbor_name}: may become {result}"),
                    });
                }
            }
        }
    }
    if water_neighbor && element.motion != Motion::Static {
        if let Some(water) = world.rules().id("water") {
            let water_density = world.rules().elements[water].density;
            if element.density > water_density {
                events.push("in water: heavier than water, may sink".into());
            } else if element.density < water_density {
                events.push("in water: lighter than water, may float".into());
            }
        }
    }
    if matches!(element.name.as_str(), "beetle" | "termite") {
        events.push("critter: wanders, flees fire or water, and may breed".into());
    } else if element.name == "fungus" {
        events.push("fungus: spreads in humid conditions when food is nearby".into());
    }
    events.sort();
    events.dedup();
    lines.extend(events.into_iter().map(|event| format!("May: {event}")));
    Some(CellDescription {
        title: crate::labels::display_name(&element.name),
        category: crate::labels::group(&element.name),
        lines,
    })
}

fn behavior_description(
    cell: tinderbox_sim::Cell,
    element: &tinderbox_sim::Element,
) -> &'static str {
    if cell.is_in_body() {
        if element.name == "wood" {
            "boat hull moves with wind and currents"
        } else {
            "boat cargo moves with its hull"
        }
    } else if matches!(element.name.as_str(), "beetle" | "termite") {
        "wanders over nearby ground"
    } else {
        match element.motion {
            Motion::Static => "stays in place",
            Motion::Fall => "falls under gravity",
            Motion::Liquid => "flows and settles",
            Motion::Gas => "rises and drifts with wind",
        }
    }
}

fn place_overlays(canvas: Rect, pointer: Pos2, lens_size: Vec2, panel_size: Vec2) -> (Rect, Rect) {
    let lens_size = Vec2::new(
        lens_size.x.min(canvas.width() * 0.45),
        lens_size.y.min(canvas.height() * 0.42),
    );
    let lens_x = if pointer.x + 20.0 + lens_size.x <= canvas.right() {
        pointer.x + 20.0
    } else if pointer.x - 20.0 - lens_size.x >= canvas.left() {
        pointer.x - 20.0 - lens_size.x
    } else {
        (pointer.x + 20.0).clamp(canvas.left(), canvas.right() - lens_size.x)
    };
    let lens_y = if pointer.y + 20.0 + lens_size.y <= canvas.bottom() {
        pointer.y + 20.0
    } else if pointer.y - 20.0 - lens_size.y >= canvas.top() {
        pointer.y - 20.0 - lens_size.y
    } else {
        (pointer.y + 20.0).clamp(canvas.top(), canvas.bottom() - lens_size.y)
    };
    let lens = Rect::from_min_size(Pos2::new(lens_x, lens_y), lens_size);
    let panel_size = Vec2::new(
        panel_size.x.min(canvas.width() * 0.4),
        panel_size.y.min(canvas.height() * 0.5),
    );
    let panel_y = lens
        .top()
        .clamp(canvas.top(), canvas.bottom() - panel_size.y);
    let left_x = lens.left() - panel_size.x - 8.0;
    let right_x = lens.right() + 8.0;
    let (panel_x, panel_y) = if left_x >= canvas.left() && right_x + panel_size.x > canvas.right() {
        (left_x, panel_y)
    } else if right_x + panel_size.x <= canvas.right() {
        (right_x, panel_y)
    } else if left_x >= canvas.left() {
        (left_x, panel_y)
    } else {
        let above_y = lens.top() - panel_size.y - 8.0;
        let below_y = lens.bottom() + 8.0;
        let panel_x = lens
            .left()
            .clamp(canvas.left(), canvas.right() - panel_size.x);
        if above_y >= canvas.top() {
            (panel_x, above_y)
        } else if below_y + panel_size.y <= canvas.bottom() {
            (panel_x, below_y)
        } else {
            let panel = [
                Rect::from_min_size(canvas.left_top(), panel_size),
                Rect::from_min_size(
                    canvas.right_top() - Vec2::new(panel_size.x, 0.0),
                    panel_size,
                ),
                Rect::from_min_size(
                    canvas.left_bottom() - Vec2::new(0.0, panel_size.y),
                    panel_size,
                ),
                Rect::from_min_size(canvas.right_bottom() - panel_size, panel_size),
            ]
            .into_iter()
            .find(|candidate| !candidate.intersects(lens))
            .unwrap_or_else(|| Rect::from_min_size(Pos2::new(panel_x, panel_y), panel_size));
            return (lens, panel);
        }
    };
    (
        lens,
        Rect::from_min_size(Pos2::new(panel_x, panel_y), panel_size),
    )
}

pub(super) fn show(
    app: &Tinderbox,
    painter: &egui::Painter,
    canvas: Rect,
    pointer: Pos2,
    view_width: f32,
) {
    let (world_x, world_y) = input::screen_to_world(
        pointer,
        canvas,
        view_width,
        app.world.height(),
        app.camera_x,
    );
    let x =
        (world_x.floor() as isize).clamp(0, app.world.width().saturating_sub(1) as isize) as usize;
    let y =
        (world_y.floor() as isize).clamp(0, app.world.height().saturating_sub(1) as isize) as usize;
    let Some(description) = describe_cell(&app.world, x, y) else {
        return;
    };
    let cell_w = canvas.width() / view_width.max(1.0);
    let cell_h = canvas.height() / app.world.height() as f32;
    let lens_size = Vec2::new(
        cell_w * LENS_WIDTH as f32 * MAGNIFICATION,
        cell_h * LENS_HEIGHT as f32 * MAGNIFICATION,
    );
    let panel_width = 250.0_f32.min(canvas.width() * 0.4);
    let text_font = FontId::proportional(11.5);
    let text_color = Color32::WHITE;
    let line_galleys = description
        .lines
        .iter()
        .map(|line| {
            painter.layout(
                line.clone(),
                text_font.clone(),
                text_color,
                (panel_width - 16.0).max(1.0),
            )
        })
        .collect::<Vec<_>>();
    let content_height: f32 = line_galleys
        .iter()
        .map(|galley| galley.size().y + 2.0)
        .sum();
    let panel_size = Vec2::new(
        panel_width,
        (content_height + 30.0).min(canvas.height() * 0.5),
    );
    let (lens, panel) = place_overlays(canvas, pointer, lens_size, panel_size);
    let texture_width = app.world_texture.size()[0] as f32;
    let half_w = LENS_WIDTH as f32 / (2.0 * texture_width);
    let half_h = LENS_HEIGHT as f32 / (2.0 * app.world.height() as f32);
    let uv = Rect::from_min_max(
        Pos2::new(
            (x as f32 / texture_width - half_w).max(0.0),
            (y as f32 / app.world.height() as f32 - half_h).max(0.0),
        ),
        Pos2::new(
            (x as f32 / texture_width + half_w).min(app.world.width() as f32 / texture_width),
            (y as f32 / app.world.height() as f32 + half_h).min(1.0),
        ),
    );
    painter.image(app.world_texture.id(), lens, uv, Color32::WHITE);
    painter.rect_stroke(
        lens,
        2.0,
        Stroke::new(2.0_f32, Color32::WHITE),
        egui::StrokeKind::Outside,
    );
    let cell_center = input::world_to_screen(
        x as f32 + 0.5,
        y as f32 + 0.5,
        canvas,
        view_width,
        app.world.height(),
        app.camera_x,
    );
    painter.circle_stroke(
        cell_center,
        cell_w.max(cell_h) * 0.9,
        Stroke::new(1.5_f32, Color32::YELLOW),
    );
    painter.rect_filled(panel, 4.0, Color32::from_black_alpha(230));
    painter.rect_stroke(
        panel,
        4.0,
        Stroke::new(1.0_f32, Color32::LIGHT_GRAY),
        egui::StrokeKind::Inside,
    );
    painter.text(
        panel.left_top() + Vec2::new(8.0, 6.0),
        Align2::LEFT_TOP,
        format!("{} · {}", description.title, description.category),
        FontId::proportional(14.0),
        Color32::WHITE,
    );
    let mut text_y = panel.top() + 26.0;
    for galley in line_galleys {
        if text_y + galley.size().y > panel.bottom() {
            break;
        }
        painter.galley(
            Pos2::new(panel.left() + 8.0, text_y),
            galley.clone(),
            text_color,
        );
        text_y += galley.size().y + 2.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinderbox_sim::{DEFAULT_RULES, Rules};

    fn world() -> World {
        World::new(8, 8, 1, Rules::load(DEFAULT_RULES).unwrap()).unwrap()
    }

    #[test]
    fn describes_fire_adjacent_tree_and_wet_seed() {
        let mut world = world();
        let tree = world.rules().id("tree").unwrap();
        let fire = world.rules().id("fire").unwrap();
        let seed = world.rules().id("seed").unwrap();
        world.paint(3, 3, tree);
        world.paint(4, 3, fire);
        let tree_description = describe_cell(&world, 3, 3).unwrap();
        assert!(
            tree_description
                .lines
                .iter()
                .any(|line| line.contains("Fire: may become Fire"))
        );
        let wet_soil = world.rules().id("wet_soil").unwrap();
        world.paint(2, 2, wet_soil);
        world.paint(2, 1, seed);
        let seed_description = describe_cell(&world, 2, 1).unwrap();
        assert!(
            seed_description
                .lines
                .iter()
                .any(|line| line.contains("seed may sprout into Plant"))
        );
    }

    #[test]
    fn describes_boat_hull_behavior() {
        let mut world = World::new(12, 8, 1, Rules::load(DEFAULT_RULES).unwrap()).unwrap();
        let water = world.rules().id("water").unwrap();
        for x in 3..=9 {
            world.paint(x, 5, water);
        }
        assert!(world.place_boat(6, 4));
        let wood = world.rules().id("wood").unwrap();
        let hull_x = 6;
        let hull_y = 2;
        assert_eq!(world.cells()[hull_y * world.width() + hull_x].element, wood);
        let description = describe_cell(&world, hull_x, hull_y).unwrap();
        assert!(
            description
                .lines
                .iter()
                .any(|line| line.contains("boat hull moves with wind and currents"))
        );
    }

    #[test]
    fn describes_critter_behavior() {
        let mut world = world();
        let beetle = world.rules().id("beetle").unwrap();
        world.paint(3, 3, beetle);
        let description = describe_cell(&world, 3, 3).unwrap();
        assert!(
            description
                .lines
                .iter()
                .any(|line| line.contains("wanders, flees fire or water"))
        );
        assert!(description.lines[0].contains("wanders over nearby ground"));
        assert!(
            description
                .lines
                .iter()
                .all(|line| !line.contains("stationary"))
        );
    }

    #[test]
    fn lens_and_tooltip_stay_inside_canvas_without_overlapping_at_edges() {
        let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
        for pointer in [Pos2::new(8.0, 8.0), Pos2::new(992.0, 692.0)] {
            let (lens, panel) = place_overlays(
                canvas,
                pointer,
                Vec2::new(320.0, 220.0),
                Vec2::new(250.0, 200.0),
            );
            assert!(canvas.contains_rect(lens));
            assert!(canvas.contains_rect(panel));
            assert!(!lens.intersects(panel));
        }
    }
}
