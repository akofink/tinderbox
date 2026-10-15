# Capture and testing

## Run and build

Install Rust with `rustup` and start the native app:

```sh
cargo run --release -p tinderbox-app
```

To build or serve the browser app locally, install the WebAssembly target and Trunk:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
cd crates/app
trunk serve
```

Trunk serves the browser build locally with live reload. For a production bundle, run `trunk build --release --public-url /` from `crates/app`. Use the deployment's configured public URL when building for a subpath.

## Reproducible native captures

Fixed-step captures require a level recipe and an output PNG. They run to a deterministic simulation time or frame count, save the app view, and close:

```sh
cargo run --release -p tinderbox-app -- \
  --level level.toml \
  --capture-at-seconds 20 \
  --screenshot scene.png
```

Use `--capture-at-seconds 0` for the initial scene. `--capture-after FRAMES` captures after a fixed frame count instead; choose one fixed-step option, not both. Captures stop the simulation at the requested point and wait for texture uploads to finish before saving.

Capture options:

- `--level FILE` loads a human-readable level recipe. Recipes, built-in presets, and browser save/load are covered in [Level recipes](levels.md).
- `--camera-x CELLS` starts an endless world at a chosen world coordinate. It can be combined with `--level` and a capture option.
- `--inspect-at X,Y` turns on the magnifying-glass inspector at a world cell for the capture.
- `--drop-boat-at X,Y` stamps a test boat at a world coordinate before capture.
- `--screenshot FILE.png` saves the screenshot; it must be paired with a timed capture or measurement.
- `--world-size WIDTHxHEIGHT` overrides the initial native world size when not loading a recipe.
- `--speed RATE` selects a simulation speed from 0.25× through 10×.
- `--measure-real-seconds SECONDS` runs interactively for a measured real-time interval and reports achieved speed and frame timings. Add `--screenshot FILE.png` to save the final frame.
- `--span-evidence DIR` runs the scripted endless-world fire demonstration and saves the burning view, far-east view, and return view.
- `--smooth-play` runs a 900-frame native heavy-play check. Add `--world-size=1024x640` to exercise a larger grid. The report includes simulation and texture-upload caps, frame intervals, and timing data.

Interactive simulation work adapts to a 10 ms target per frame and caps carried work at three frames at the requested speed. The HUD reports requested and achieved speed, pending work, and discarded excess work. Native windows start at 1360 × 860; worlds fit the visible canvas by default at 3.5 logical points per cell, and the **Cell size** control zooms from 2 to 8 points per cell. Resizing preserves existing cells and regenerates newly exposed terrain from the same level seed after a 120 ms debounce.

For native fixed-step captures and desktop checks, use a stable recipe, seed, and requested time. Only describe properties visible in the resulting image.

## Checks

Run the workspace quality gates before publishing changes:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The browser smoke test builds the WebAssembly app, serves it, and checks that it starts and renders in Chromium. Run it in an environment with Trunk, Chromium, Node, and Python installed:

```sh
cd crates/app
trunk build --release --public-url /
cd ../..
python3 -m http.server 4173 --directory crates/app/dist
```

In another shell at the repository root, run `node scripts/web-smoke.mjs` against the local server, followed by `python scripts/assert-web-smoke-image.py` to check the captured image. The smoke test and image assertion are also part of CI. The simulation crate is headless and deterministic; its tests do not require a GPU or browser.
