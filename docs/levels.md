# Level recipes

**Level setup** appears at startup and is available from the game toolbar. Choose a preset, fit-to-canvas or advanced explicit dimensions, seed, terrain, starting wind and element amounts; the world behind the panel regenerates after edits settle. By default the fixed-size grid follows the visible canvas at 3.5 logical points per cell. Change **Cell size** in the toolbar to zoom; resize changes are debounced by 120 ms. In fixed mode, existing cells are kept, newly exposed cells use the recipe's same-seed terrain, and shrinking crops cells beyond the new right or bottom edge. Endless mode keeps an active span around the camera and generates terrain as the view approaches either edge. Ambient oxygen defaults to 21%; set it to 0% to make an airless level. Opening setup pauses the game. **Play level** keeps the current preview, while **Cancel / return to game** restores the previous world and run state. On native desktop, a recipe loaded from a file also reloads when that file changes on disk.

- **Dry forest, east wind**: dense trees on rolling stone hills with a few sparks and a gentle east wind. Try pouring water across the fire front.
- **Island**: a wooded island and sandy shore surrounded by water. Try damming the shoreline with stone.
- **Sandstorm valley**: sandy slopes with ash, smoke, sparks and a strong east wind. Try adding water at the bottom or igniting a painted grove.
- **Boat lake**: four boats share a stone-banked lake: one empty, one lightly loaded, one overloaded with sand and one burning. East wind moves the hulls while the lake retains the displaced water.
- **Density lab**: sand falls into a sealed near-freezing lake, ice and oil begin at the surface, and a submerged fire vent produces steam; oil can ignite on the water.
- **Empty canvas**: a blank world, ignoring amount settings unless the wooden boat option is enabled. A boat recipe adds a small water patch; optional sand cargo lowers it, and the burning option starts a fire in a hull plank.

Amounts are percentages within a material's terrain region, not fractions of the entire world. Stone makes the underlying terrain, soil and sand coat the ground, trees form clumps above it, water fills depressions, and fire starts along the surface. Other elements use their movement type to choose a region. Every non-air element in the active rules automatically gets a slider, including newly added ones. Zero disables generation of that element, but does not stop painting it or producing it through interactions.

Recipes default to `mode = "fixed"`, so existing and older recipes retain their bounded-world behavior. Set `mode = "endless"` to generate deterministic 128-cell chunks from the recipe seed and signed chunk coordinate. Hills, islands, and valleys continue across chunk seams. Arrow keys, scrolling, or Shift-dragging / middle-dragging move the camera. Only visible chunks plus a two-chunk margin stay resident and simulated. The span edge is sealed, so fire, water, sand, and wind-carried material do not spill into dormant chunks. Painted or simulated chunks return as they were; untouched chunks are regenerated from the seed. Woken chunks do not catch up. The recipe width sets the initial viewport scale, not a growing world limit. Pointer painting, boats, gusts, and inspection use world coordinates after the camera moves.

Explicit dimensions range from 16 to 2048 cells per side, with a maximum of 1,048,576 cells total. Expand **Advanced world size override** and disable **Fit world to canvas** to store an explicit recipe size. Recipes that omit both `width` and `height` fit the available canvas; specifying only one is invalid. Large worlds take longer to evolve under the same fixed per-frame simulation work budget. Terrain setup is outside the simulation hot path.

## Save, load and share

Expand **Save / load level (TOML)**. On desktop, enter a file path and use **Save file** or **Load file**. Relative paths are relative to the app's launch directory. Saving replaces an existing file at that path. Errors are shown in setup and leave the current preview unchanged; valid loaded recipes update the preview after the edit debounce.

In a browser, **Download level** saves the recipe as a file and remembers it locally. **Upload level** or dropping a valid recipe on the canvas opens its settings without changing the running world; **Stored level** loads the last saved or uploaded recipe from this browser.

On desktop or in a browser, **Export settings to text** produces a human-readable recipe. **Copy text** copies it for sharing. Paste a recipe into the text area and press **Load text**, then Play. A recipe stores setup, not an ongoing simulation snapshot or a ruleset. Use the same ruleset to reproduce the same world and subsequent simulation.

```toml
version = 1
width = 384
height = 240
seed = "29"
generator = "island"
spark = false
ambient_temperature = 20
ambient_humidity = 45
ambient_oxygen = 21
boat = false
boat_sand = 0
boat_burning = false

[wind]
direction = "calm"
strength = 0

[amounts]
stone = 100
sand = 65
tree = 50
water = 100
```

Supported generators are `empty`, `hills`, `island`, `valley`, `boat_lake`, and `density_lab`. The two showcase generators build exact, sealed stone basins and place their demonstrations directly; material amount sliders do not alter those authored scenes. Set `boat = true` to add a small wooden hull over water; `boat_sand` stacks 0..8 sand cargo cells, and `boat_burning = true` starts a fire in a hull plank. These optional recipe fields default to no boat, no cargo, and no fire. Seeds are exported as decimal strings to preserve the full 64-bit range; non-negative TOML integers are also accepted. Missing element amounts default to zero. Wind directions are `calm`, `west` and `east`, with strength 0..4; recipes without a wind table default to calm. `spark` defaults to `false`. Where the selected generation path applies it, it adds a small fire patch near the left edge, on the first tree found there when possible or at a fallback position otherwise; this is in addition to any `fire` amount. In fixed mode, `empty`, `boat_lake`, and `density_lab` do not apply `spark`. `ambient_temperature` sets the ambient temperature in °C and defaults to 20 (valid range -40..60); `ambient_humidity` sets ambient humidity as a percentage and defaults to 45 (valid range 0..100). Older recipes without `ambient_oxygen` default to 21%. Unknown element names, unsupported versions, invalid dimensions and amounts outside 0..100 fail clearly. Omit both dimension fields to make the recipe fit the canvas, or include both for an explicit size. File loading is limited to 64 KiB.

**Reset** (or `R`) discards painting and evolution and regenerates the applied level using its original seed, wind and settings. It does not choose a new seed. To get a different world, open setup and use **Next seed**. Setup drafts and loaded files do not change the reset target until Play succeeds.
