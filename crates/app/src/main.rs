mod app;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    app::run()
}

#[cfg(target_arch = "wasm32")]
fn main() {
    app::start_web();
}
