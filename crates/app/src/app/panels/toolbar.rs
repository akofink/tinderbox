//! Play controls, brush controls, speed, reset, and scene setup.
use crate::app::Tinderbox;
use eframe::egui;

pub(in crate::app) fn show(app: &mut Tinderbox, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .button(if app.running { "⏸ Pause" } else { "▶ Play" })
            .clicked()
        {
            app.running = !app.running;
        }
        if ui.button("Step").clicked() {
            app.world.step(super::super::WORK_PER_FRAME);
            app.sim_seconds += 1.0 / 60.0;
        }
        if ui.button("Reset").clicked() {
            app.reset();
        }
        if ui.button("Wildfire").clicked() {
            app.wildfire();
        }
        ui.separator();
        ui.label("Brush");
        ui.add(egui::Slider::new(&mut app.brush, 1..=16).suffix(" cells"));
        ui.label("Speed");
        ui.add(egui::Slider::new(&mut app.speed, 0.25..=4.0).suffix("×"));
        ui.checkbox(&mut app.show_hud, "HUD");
        #[cfg(target_arch = "wasm32")]
        show_browser_files(ui);
    });
}

#[cfg(target_arch = "wasm32")]
fn show_browser_files(ui: &mut egui::Ui) {
    use crate::app::files;
    use tinderbox_sim::DEFAULT_RULES;

    ui.separator();
    if ui.button("Download rules").clicked() {
        match files::save_text("rules", "tinderbox-rules.toml", DEFAULT_RULES) {
            Ok(()) => files::set_notice("Downloaded the bundled rules."),
            Err(err) => files::set_notice(err),
        }
    }
    if ui.button("Upload rules").clicked() {
        if let Err(err) = files::begin_upload("rules") {
            files::set_notice(err);
        }
    }
    if ui.button("Stored rules").clicked() {
        match files::load_text("rules") {
            Ok(text) => files::finish_upload("rules", Ok(text)),
            Err(err) => files::set_notice(err),
        }
    }
    let notice = files::notice();
    if !notice.is_empty() {
        ui.label(notice);
    }
}
