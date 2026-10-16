//! Optional in-world frame and simulation overlay.
use crate::app::Tinderbox;
use eframe::egui::{self, Color32, Painter, Rect};

pub(super) fn show(app: &Tinderbox, painter: &Painter, rect: Rect) {
    if !app.show_hud {
        return;
    }
    let frame_status = format!(
        "{:.1} ms/frame   sim {:.1}s   {}",
        app.frame_ms,
        app.sim_seconds,
        app.speed_status(),
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
    if rect.width() < 420.0 {
        let font = egui::FontId::monospace(11.0);
        let compact_frame_status = format!(
            "{:.0}ms/f  sim {:.1}s\n{:.1}x req / {:.1}x act",
            app.frame_ms, app.sim_seconds, app.speed, app.sim_rate
        );
        let compact_work_status = format!(
            "focus {} carry {} drop {} dirty {}{chunk_status}",
            app.world.pending_work(),
            app.work.pending.ceil() as usize,
            app.work.dropped.ceil() as usize,
            app.world.pending_dirty_regions(),
        );
        let galley = painter.layout(
            format!("{compact_frame_status}\n{compact_work_status}"),
            font,
            Color32::WHITE,
            (rect.width() - 16.0).max(1.0),
        );
        super::toolbar::record_test_panel_bounds(
            "hud",
            egui::Rect::from_min_size(origin, galley.size()),
        );
        painter.galley(origin, galley, Color32::WHITE);
    } else if rect.width() < 900.0 {
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
