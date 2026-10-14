# Level recipes

**Level setup** appears at startup and is available from the game toolbar. Choose a preset, dimensions, seed, terrain, starting wind and element amounts, then **Play level**. Ambient oxygen defaults to 21%; set it to 0% to make an airless level. Opening setup pauses the game. **Cancel / return to game** preserves the previous world; loading settings does not replace it until you press Play.

- **Dry forest, east wind**: dense trees on rolling stone hills with a few sparks and a gentle east wind. Try pouring water across the fire front.
- **Island**: a wooded island and sandy shore surrounded by water. Try damming the shoreline with stone.
- **Sandstorm valley**: sandy slopes with ash, smoke, sparks and a strong east wind. Try adding water at the bottom or igniting a painted grove.
- **Empty canvas**: a blank world, ignoring amount settings unless the wooden boat option is enabled. A boat recipe adds a small water patch; optional sand cargo lowers it, and the burning option starts a fire in a hull plank.

Amounts are percentages within a material's terrain region, not fractions of the entire world. Stone makes the underlying terrain, soil and sand coat the ground, trees form clumps above it, water fills depressions, and fire starts along the surface. Other elements use their movement type to choose a region. Every non-air element in the active rules automatically gets a slider, including newly added ones. Zero disables generation of that element, but does not stop painting it or producing it through interactions.

Dimensions range from 16 to 2048 cells per side, with a maximum of 1,048,576 cells total. Large worlds take longer to evolve under the same fixed per-frame simulation work budget. Terrain setup is a one-time operation, outside the simulation hot path.

## Save, load and share

Expand **Save / load level (TOML)**. On desktop, enter a file path and use **Save file** or **Load file**. Relative paths are relative to the app's launch directory. Saving replaces an existing file at that path. Errors are shown in setup and leave the game untouched.

In a browser, **Download level** saves the recipe as a file and remembers it locally. **Upload level** or dropping a valid recipe on the canvas opens its settings without changing the running world; **Stored level** loads the last saved or uploaded recipe from this browser.

On desktop or in a browser, **Export settings to text** produces a human-readable recipe. **Copy text** copies it for sharing. Paste a recipe into the text area and press **Load text**, then Play. A recipe stores setup, not an ongoing simulation snapshot or a ruleset. Use the same ruleset to reproduce the same world and subsequent simulation.

```toml
version = 1
width = 384
height = 240
seed = "29"
generator = "island"
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

Supported generators are `empty`, `hills`, `island`, and `valley`. Set `boat = true` to add a small wooden hull over water; `boat_sand` stacks 0..8 sand cargo cells, and `boat_burning = true` starts a fire in a hull plank. These optional recipe fields default to no boat, no cargo, and no fire. Seeds are exported as decimal strings to preserve the full 64-bit range; non-negative TOML integers are also accepted. Missing element amounts default to zero. Wind directions are `calm`, `west` and `east`, with strength 0..4; recipes without a wind table default to calm. Older recipes without `ambient_oxygen` default to 21%. Unknown element names, unsupported versions, invalid dimensions and amounts outside 0..100 fail clearly. File loading is limited to 64 KiB.

**Reset** (or `R`) discards painting and evolution and regenerates the applied level using its original seed, wind and settings. It does not choose a new seed. To get a different world, open setup and use **Next seed**. Setup drafts and loaded files do not change the reset target until Play succeeds.
