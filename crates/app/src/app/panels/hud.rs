//! Optional in-world frame and simulation overlay.
use crate::app::Tinderbox;
use eframe::egui::{self, Color32, Painter, Rect};

pub(super) fn show(app: &Tinderbox, painter: &Painter, rect: Rect) {
    if !app.show_hud {
        return;
    }
    let frame_status = format!(
        "{:.1} ms/frame   sim {:.1}s   {:.2}x requested, {:.2}x achieved",
        app.frame_ms, app.sim_seconds, app.speed, app.sim_rate,
    );
    let work_status = format!(
        "focus {} carry {} dropped {} dirty {}",
        app.world.pending_work(),
        app.work.pending.ceil() as usize,
        app.work.dropped.ceil() as usize,
        app.world.pending_dirty_regions(),
    );
    let chunk_status = if app.level.mode == tinderbox_sim::level::LevelMode::Endless {
        format!(
            "   chunk {}",
            app.world_start_chunk
                + (app.camera_x as usize / tinderbox_sim::ENDLESS_CHUNK_WIDTH) as i64
        )
    } else {
        String::new()
    };
    let origin = rect.left_top() + egui::Vec2::splat(8.0);
    let font = egui::FontId::monospace(13.0);
    if rect.width() < 900.0 {
        painter.text(
            origin,
            egui::Align2::LEFT_TOP,
            frame_status,
            font.clone(),
            Color32::WHITE,
        );
        painter.text(
            origin + egui::vec2(0.0, 18.0),
            egui::Align2::LEFT_TOP,
            format!("{work_status}{chunk_status}"),
            font,
            Color32::WHITE,
        );
    } else {
        painter.text(
            origin,
            egui::Align2::LEFT_TOP,
            format!("{frame_status}   {work_status}{chunk_status}"),
            font,
            Color32::WHITE,
        );
    }
}
