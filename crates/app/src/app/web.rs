//! Wasm entry point. WebGPU is preferred; the `wgpu` `webgl` feature is the WebGL2 fallback.
use super::Tinderbox;
use eframe::wasm_bindgen::JsCast as _;

pub(super) fn start() {
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
                Box::new(|cc| Ok(Box::new(Tinderbox::new(cc)))),
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

pub(super) fn publish_frame(frame_ms: f32, frame: &eframe::Frame) {
    let Some(canvas) = eframe::web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("the_canvas_id"))
    else {
        return;
    };
    let _ = canvas.set_attribute("data-frame-ms", &format!("{frame_ms:.2}"));
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
