# Tinderbox

A living falling-element sandbox where fire, wind, weather, and ecosystems reshape every world.

## [Play in your browser](https://akofink.com/tinderbox/)

![A wind-driven fire front moving through a forest, with east wind selected](docs/images/hero-fire.webp)

*East wind carries a fire front into standing trees.*

![MIT licensed](https://img.shields.io/badge/license-MIT-blue)

## Screenshots

| Endless world | Weather |
| --- | --- |
| ![Seeded hills and forest at chunk minus one, with east wind selected](docs/images/endless-world.webp)<br>Hills and forest at chunk −1, with east wind selected. | ![Clouds and rain over a wooded island bordered by water](docs/images/weather-rain.webp)<br>Clouds and rain cross a wooded island. |
| Inspector | Level setup |
| ![Magnifier and tooltip showing the inspected cell and local atmosphere](docs/images/inspector.webp)<br>The magnifier and tooltip show a cell and its local atmosphere. | ![Level setup panel with terrain, wind, atmosphere, and element settings](docs/images/level-setup.webp)<br>Choose a preset, shape terrain, and tune wind and materials. |
| Boats | |
| ![Four boats on a lake, including one with a burning hull](docs/images/boats.webp)<br>Four boats share a lake; one has a burning hull. | |

## Run locally

Install Rust with `rustup`, then launch the native game:

```sh
cargo run --release -p tinderbox-app
```

For browser builds, scripted captures, flags, and testing, see [Capture and testing](docs/capture-and-testing.md). See [Level recipes](docs/levels.md) for presets and saving, loading, and sharing worlds.

## Play

- Choose a preset or build a level, then paint materials into the world. Fixed and endless worlds are available.
- Set wind, inspect cells with the magnifier, open the live Rules panel, or speed the simulation up to 10×.
- Watch clouds, rain, fire, water, plants, trees, fungi, and critters interact. Build and load custom levels in the browser, or save and load tuned rules there.

## Lineage

Tinderbox reuses and adapts ideas from [Cascade](https://github.com/akofink/cascade-game-demo), including bounded simulation work with carry-over, preallocated scheduler storage, dirty-region texture uploads under a per-frame cap, a headless core, and the winit/wgpu/egui native stack. It is independently licensed under MIT and has no dependency on Cascade crates.

## License

MIT. See [LICENSE](LICENSE).
