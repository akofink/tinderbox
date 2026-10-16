//! Wasm entry point. WebGPU is preferred; the `wgpu` `webgl` feature is the WebGL2 fallback.
use super::Tinderbox;
use eframe::wasm_bindgen::JsCast as _;

pub(super) fn start() {
    let search = eframe::web_sys::window()
        .and_then(|window| window.location().search().ok())
        .unwrap_or_default();
    let toolbar_test_mode = search.contains("toolbar-test=1");
    let profile_mode = search
        .split(['?', '&'])
        .find_map(|part| part.strip_prefix("profile="));
    let profile_mode = profile_mode
        .filter(|mode| matches!(*mode, "fixed" | "endless"))
        .map(str::to_owned);
    super::panels::toolbar::set_test_mode(toolbar_test_mode);
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    let web_options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async move {
        let document = eframe::web_sys::window()
            .expect("window")
            .document()
            .expect("document");
        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("the_canvas_id")
            .dyn_into::<eframe::web_sys::HtmlCanvasElement>()
            .expect("canvas");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(move |cc| {
                    let mut app = Tinderbox::new_with_context(
                        cc.egui_ctx.clone(),
                        super::DEFAULT_WIDTH,
                        super::DEFAULT_HEIGHT,
                        profile_mode.is_some(),
                    );
                    if profile_mode.as_deref() == Some("endless") {
                        let mut level = app.level.clone();
                        level.mode = super::LevelMode::Endless;
                        level.fit_window = false;
                        app.set_level(level)
                            .expect("browser profile level must generate");
                        app.level_setup.open = false;
                        app.running = true;
                    }
                    Ok(Box::new(app))
                }),
            )
            .await;
        let loading = document.get_element_by_id("loading_text");
        match started {
            Ok(()) => {
                if let Some(loading) = loading {
                    loading.remove();
                }
            }
            Err(err) => {
                if let Some(loading) = loading {
                    loading.set_text_content(Some(&format!("Tinderbox failed to start: {err:?}")));
                }
            }
        }
    });
}

pub(super) fn publish_profile(app: &Tinderbox) {
    let Some(canvas) = eframe::web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("the_canvas_id"))
    else {
        return;
    };
    let mut sorted = app.stress_frame_intervals_ms.clone();
    sorted.sort_by(f32::total_cmp);
    let p99 = sorted
        .get((sorted.len() * 99 / 100).min(sorted.len().saturating_sub(1)))
        .copied()
        .unwrap_or_default();
    let steady = sorted
        .iter()
        .skip(super::SMOOTH_PLAY_WARMUP_FRAMES)
        .copied()
        .collect::<Vec<_>>();
    let steady_p99 = steady
        .get((steady.len() * 99 / 100).min(steady.len().saturating_sub(1)))
        .copied()
        .unwrap_or_default();
    let steady_avg = steady.iter().sum::<f32>() / steady.len().max(1) as f32;
    let steady_worst = steady.last().copied().unwrap_or_default();
    let mode = match app.level.mode {
        super::LevelMode::Fixed => "fixed",
        super::LevelMode::Endless => "endless",
    };
    let report = format!(
        "mode={mode};world={}x{};frames={};warmup={};avg_frame_ms={:.2};p99_frame_ms={p99:.2};worst_frame_ms={:.2};steady_avg_frame_ms={steady_avg:.2};steady_p99_frame_ms={steady_p99:.2};steady_worst_frame_ms={steady_worst:.2};max_pan_ms={:.2};max_extend_ms={:.2};max_texture_ms={:.2};max_upload_ms={:.2};max_pending_dirty_regions={};max_draw_ms={:.2};max_step_ms={:.2};max_update_work_ms={:.2}",
        app.world.width(),
        app.world.height(),
        app.stress_frames,
        super::SMOOTH_PLAY_WARMUP_FRAMES,
        app.stress_total_ms / app.stress_frames.max(1) as f64,
        app.stress_max_ms,
        app.stress_max_pan_ms,
        app.stress_max_extend_ms,
        app.stress_max_texture_ms,
        app.stress_max_upload_ms,
        app.stress_max_pending_dirty_regions,
        app.stress_max_draw_ms,
        app.stress_max_step_ms,
        app.stress_max_update_work_ms,
    );
    let _ = canvas.set_attribute("data-profile-report", &report);
}

pub(super) fn publish_frame(app: &Tinderbox, frame: &eframe::Frame) {
    let Some(canvas) = eframe::web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("the_canvas_id"))
    else {
        return;
    };
    let _ = canvas.set_attribute("data-frame-ms", &format!("{:.2}", app.frame_ms));
    if app.stress_play {
        let _ = canvas.set_attribute("data-profile-frames", &app.stress_frames.to_string());
    }
    if canvas.get_attribute("data-renderer").is_none() {
        let renderer = frame
            .wgpu_render_state()
            .map(|state| match state.adapter.get_info().backend {
                eframe::wgpu::Backend::BrowserWebGpu => "webgpu",
                eframe::wgpu::Backend::Gl => "webgl",
                _ => "other",
            })
            .unwrap_or("unknown");
        let _ = canvas.set_attribute("data-renderer", renderer);
    }
}
