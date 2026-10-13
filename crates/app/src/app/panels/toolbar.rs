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
            app.request_step();
        }
        if ui.button("Reset").clicked() {
            app.reset();
        }
        if ui.button("Level setup").clicked() {
            app.open_level_setup();
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
        ui.toggle_value(&mut app.rules_panel.open, "Rules");
        #[cfg(target_arch = "wasm32")]
        show_browser_files(app, ui);
        super::wind::show(app, ui);
    });
}

#[cfg(target_arch = "wasm32")]
fn show_browser_files(app: &Tinderbox, ui: &mut egui::Ui) {
    use crate::app::files;

    ui.separator();
    if ui.button("Download rules").clicked() {
        let result = app
            .world
            .rules()
            .to_toml()
            .map_err(|e| e.to_string())
            .and_then(|text| files::save_text("rules", "tinderbox-rules.toml", &text));
        match result {
            Ok(()) => files::set_notice("Downloaded the current tuned rules."),
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
