//! Read-only magnified view and plain-language description of the hovered cell.
use super::Tinderbox;
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
    let mut lines = vec![format!("Type: {}", motion_description(element.motion))];
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
    if element.motion != Motion::Static {
        lines.push(format!("Motion: {}", motion_description(element.motion)));
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
    if cell.is_in_body() {
        lines.push(if element.name == "wood" {
            "Boat hull: moves as one body with wind and currents".into()
        } else {
            "Boat cargo: moves with its hull as one body".into()
        });
    }
    Some(CellDescription {
        title: crate::labels::display_name(&element.name),
        category: crate::labels::group(&element.name),
        lines,
    })
}

fn motion_description(motion: Motion) -> &'static str {
    match motion {
        Motion::Static => "stationary",
        Motion::Fall => "falls under gravity",
        Motion::Liquid => "flows and settles",
        Motion::Gas => "rises and drifts with wind",
    }
}

pub(super) fn show(app: &Tinderbox, painter: &egui::Painter, canvas: Rect, pointer: Pos2) {
    let local_x =
        ((pointer.x - canvas.left()) / canvas.width() * app.world.width() as f32) as usize;
    let local_y =
        ((pointer.y - canvas.top()) / canvas.height() * app.world.height() as f32) as usize;
    let x = local_x.min(app.world.width().saturating_sub(1));
    let y = local_y.min(app.world.height().saturating_sub(1));
    let Some(description) = describe_cell(&app.world, x, y) else {
        return;
    };
    let cell_w = canvas.width() / app.world.width() as f32;
    let cell_h = canvas.height() / app.world.height() as f32;
    let lens_size = Vec2::new(
        cell_w * LENS_WIDTH as f32 * MAGNIFICATION,
        cell_h * LENS_HEIGHT as f32 * MAGNIFICATION,
    );
    let mut lens = Rect::from_min_size(pointer + Vec2::new(20.0, 20.0), lens_size);
    if lens.right() > canvas.right() {
        lens.set_left(pointer.x - lens.width() - 20.0);
    }
    if lens.bottom() > canvas.bottom() {
        lens.set_top(pointer.y - lens.height() - 20.0);
    }
    lens = lens.intersect(canvas);
    let half_w = LENS_WIDTH as f32 / (2.0 * app.world.width() as f32);
    let half_h = LENS_HEIGHT as f32 / (2.0 * app.world.height() as f32);
    let uv = Rect::from_min_max(
        Pos2::new(
            (x as f32 / app.world.width() as f32 - half_w).max(0.0),
            (y as f32 / app.world.height() as f32 - half_h).max(0.0),
        ),
        Pos2::new(
            (x as f32 / app.world.width() as f32 + half_w).min(1.0),
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
    let cell_center = Pos2::new(
        canvas.left() + (x as f32 + 0.5) * cell_w,
        canvas.top() + (y as f32 + 0.5) * cell_h,
    );
    painter.circle_stroke(
        cell_center,
        cell_w.max(cell_h) * 0.9,
        Stroke::new(1.5_f32, Color32::YELLOW),
    );
    let panel_width = 250.0_f32.min(canvas.width());
    let panel_height = (description.lines.len() as f32 * 16.0 + 34.0).min(canvas.height());
    let mut panel = Rect::from_min_size(
        Pos2::new(lens.right() + 8.0, lens.top()),
        Vec2::new(panel_width, panel_height),
    );
    if panel.right() > canvas.right() {
        panel.set_left(lens.left() - panel_width - 8.0);
    }
    if panel.bottom() > canvas.bottom() {
        panel.set_top(canvas.bottom() - panel.height());
    }
    panel = panel.intersect(canvas);
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
    for (line, text) in description.lines.iter().enumerate() {
        let pos = Pos2::new(panel.left() + 8.0, panel.top() + 26.0 + line as f32 * 16.0);
        if pos.y + 16.0 > panel.bottom() {
            break;
        }
        painter.text(
            pos,
            Align2::LEFT_TOP,
            text,
            FontId::proportional(11.5),
            Color32::WHITE,
        );
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
                .any(|line| line.contains("Boat hull: moves as one body"))
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
    }
}
