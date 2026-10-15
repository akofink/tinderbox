//! Optional in-world frame and simulation overlay.
use crate::app::Tinderbox;
use eframe::egui::{self, Color32, Painter, Rect};

pub(super) fn show(app: &Tinderbox, painter: &Painter, rect: Rect) {
    if !app.show_hud {
        return;
    }
    painter.text(
        rect.left_top() + egui::Vec2::splat(8.0),
        egui::Align2::LEFT_TOP,
        format!(
            "{:.1} ms/frame   sim {:.1}s   {:.2}x requested, {:.2}x achieved   focus {} carry {} dropped {} dirty {}{}",
            app.frame_ms,
            app.sim_seconds,
            app.speed,
            app.sim_rate,
            app.world.pending_work(),
            app.work.pending.ceil() as usize,
            app.work.dropped.ceil() as usize,
            app.world.pending_dirty_regions(),
            if app.level.mode == tinderbox_sim::level::LevelMode::Endless {
                format!(
                    "   chunk {}",
                    app.world_start_chunk
                        + (app.camera_x as usize / tinderbox_sim::ENDLESS_CHUNK_WIDTH) as i64
                )
            } else {
                String::new()
            }
        ),
        egui::FontId::monospace(13.0),
        Color32::WHITE,
    );
}
