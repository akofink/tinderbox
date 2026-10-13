# Tinderbox

Tinderbox is a native falling-element sandbox built to be played, not measured. Set a fire in a forest, pour water into the flames, and watch wind carry ash and smoke while soil, seeds, ice, oil, and lava add new chains to the world.

![MIT licensed](https://img.shields.io/badge/license-MIT-blue)

## Run

Install Rust with `rustup`, then start the native app:

```sh
cargo run --release -p tinderbox-app
```

The default world is 192 × 120 cells. The app starts in **Level setup**: choose a preset, size, terrain, seed, starting wind and element amounts, then press **Play level**. Open setup again from the toolbar at any time; **Wildfire** starts the dry-forest preset at the current size. See [Level recipes](docs/levels.md) for generators, human-readable save/load and sharing. Run `cargo run --release -p tinderbox-app -- --smooth-play` for a 900-frame native heavy-play check with continuous water/fire painting; it reports average and worst frame intervals.

## Play in the browser

The same sandbox is at <https://akofink.com/tinderbox/>. It uses WebGPU when the browser provides it, and WebGL2 otherwise. There is no local disk: **Download rules** saves the current tuned rules file, **Upload rules** validates and applies a file you pick, and **Stored rules** reloads the last valid copy from this browser. Dropping a text file onto the page also validates and applies it. Invalid imports keep the running rules unchanged.

## Play

- Choose sand, soil, wet or fertile soil, stone, water, ice, tree, plant, seed, fire, oil, lava, ash, smoke, steam, or oil fire in the left palette.
- Left-click or drag on the world to paint. The brush size is adjustable in the toolbar.
- Right-click to ignite at the pointer. Choose **Erase** or press `E` to paint air.
- **Pause/Play** or press `Space`; **Step** or press `.`; **Reset** or press `R` returns to the applied level's original seed and settings.
- **Level setup** pauses the game for editing. **Cancel / return to game** keeps the current world. Presets: **Dry forest, east wind**, **Island**, **Sandstorm valley**.
- Setup's **Save / load level (TOML)** saves and loads recipes by path on desktop, downloads/uploads and remembers them in a browser, and exports/imports copyable text everywhere. New ruleset elements automatically appear as amount sliders.
- Adjust simulation speed from 0.25× to 4×. Set wind direction and strength in the toolbar; select **Gust brush** and drag across the world to apply a local burst. Wind drifts ash, smoke, and steam and speeds fire spread downwind.
- Water wets soil; ash and water make fertile soil, where seeds sprout faster into plants. Ice melts near fire or lava. Lava ignites plants and turns water into stone and steam. Oil floats on water and burns quickly as bright, hot oil fire.
- Toggle the frame/simulation/pending-work HUD with **HUD** or `H`. **Ignite** or press `I` selects fire for painting.
- **Rules** opens the live physics panel. Toggle any interaction and adjust its chance without restarting the world.

## Rules and development

The default human-editable element and interaction definitions are in `crates/sim/rules/default.toml`. Elements can tune wind response and liquid density; interactions can add a downwind spread bonus. The **Rules** panel lists every loaded interaction as `source + neighbor -> source result + neighbor result`. Chance is a base probability per neighboring-cell visit, not per second; wind can add a downwind bonus. Disabled rules retain their tuned chance; they never fire, even with wind.

Use **Reset rules to defaults** to restore the shipped physics without resetting the world. On native desktop, **Save file** and **Load file** use the entered path (relative to the app's working directory or absolute). Saved TOML includes all element definitions, tuned chances, and enabled states. The **Rules TOML (copy/paste)** section also exports and applies editable text. Invalid files display an error and leave the current rules and cells unchanged. Successful loads remap existing cells and the selected brush by element name; elements removed by the loaded file become air. Sandbox shortcuts do not fire while typing in the panel.

Rules validate names, references, motion types, probabilities, and unknown fields; a live ruleset must define `air`. Old files without `enabled` keep every interaction enabled. The headless simulation API provides `World::set_interaction`, atomic `World::load_rules`, `World::reset_rules`, and `Rules::to_toml`. The simulation crate has no window, graphics, or wall-clock dependency and can run in headless tests.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Lineage

Tinderbox reuses and adapts ideas from [Cascade](https://github.com/akofink/cascade-game-demo), including bounded per-update simulation work, a headless core, and the winit/wgpu/egui native stack. The project is independently licensed under MIT and has no dependency on Cascade crates.

## License

MIT. See [LICENSE](LICENSE).
