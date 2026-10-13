# Tinderbox

Tinderbox is a native falling-element sandbox built to be played, not measured. Set a fire in a forest, pour water into the flames, and watch wind carry ash and smoke while soil, seeds, ice, oil, and lava add new chains to the world.

![MIT licensed](https://img.shields.io/badge/license-MIT-blue)

## Run

Install Rust with `rustup`, then start the native app:

```sh
cargo run --release -p tinderbox-app
```

The default world is 192 × 120 cells. The app starts playing immediately in a small forest-fire scene; click **Wildfire** to reseed the larger showcase. Run `cargo run --release -p tinderbox-app -- --smooth-play` for a 900-frame native heavy-play check with continuous water/fire painting; it reports average and worst frame intervals.

## Play in the browser

The same sandbox is at <https://akofink.com/tinderbox/>. It uses WebGPU when the browser provides it, and WebGL2 otherwise. There is no local disk: **Download rules** saves the bundled rules file, **Upload rules** checks a file you pick, and **Stored rules** reloads the last valid copy from this browser. Dropping a text file onto the page does the same check.

## Play

- Choose sand, soil, wet or fertile soil, stone, water, ice, tree, plant, seed, fire, oil, lava, ash, smoke, steam, or oil fire in the left palette.
- Left-click or drag on the world to paint. The brush size is adjustable in the toolbar.
- Right-click to ignite at the pointer. Choose **Erase** or press `E` to paint air.
- **Pause/Play** or press `Space`; **Step** or press `.`; **Reset** or press `R`.
- Adjust simulation speed from 0.25× to 4×. Set wind direction and strength in the toolbar; select **Gust brush** and drag across the world to apply a local burst. Wind drifts ash, smoke, and steam and speeds fire spread downwind.
- Water wets soil; ash and water make fertile soil, where seeds sprout faster into plants. Ice melts near fire or lava. Lava ignites plants and turns water into stone and steam. Oil floats on water and burns quickly as bright, hot oil fire.
- Toggle the frame/simulation/pending-work HUD with **HUD** or `H`. **Ignite** or press `I` selects fire for painting.

## Rules and development

The default human-editable element and interaction definitions are in `crates/sim/rules/default.toml`. Elements can tune wind response and liquid density; interactions can add a downwind spread bonus. Rules are validated when loaded; unknown element references, invalid motion types, and out-of-range probabilities produce actionable errors. The simulation crate has no window, graphics, or wall-clock dependency and can run in headless tests.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Lineage

Tinderbox reuses and adapts ideas from [Cascade](https://github.com/akofink/cascade-game-demo), including bounded per-update simulation work, a headless core, and the winit/wgpu/egui native stack. The project is independently licensed under MIT and has no dependency on Cascade crates.

## License

MIT. See [LICENSE](LICENSE).
