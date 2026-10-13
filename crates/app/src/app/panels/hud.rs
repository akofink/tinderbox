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
            "{:.1} ms/frame   sim {:.1}s   pending {}",
            app.frame_ms,
            app.sim_seconds,
            app.world.pending_work()
        ),
        egui::FontId::monospace(13.0),
        Color32::WHITE,
    );
}
