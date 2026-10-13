# Tinderbox charter

- Date: 2026-10-13 (US Eastern)
- Status: v0.1 charter, from the operator's 2026-10-13 brief
- Repository: https://github.com/akofink/tinderbox (public, MIT; to be created)
- Lineage: an iteration of Cascade (https://github.com/akofink/cascade-game-demo)

## Purpose

Tinderbox is a falling-element sandbox built to be played, not measured.
Cascade showed that a bounded per-update work budget keeps frames smooth while player actions pile on work.
Tinderbox keeps that engine discipline and spends its effort on fun: many elements, many interactions between them,
and a player who can set up a world, run it, and keep poking it live.

Success is a session where someone builds a forest on a hillside, sets wind blowing east, drops a spark, then
douses the fire front with water while ash drifts downwind, and the frame rate never stutters.

## Pillars

1. **Play live.** While the simulation runs, the player paints any element, ignites, douses, erases, and blows wind
   gusts with the pointer. Play, pause, single step, reset to the level start, and simulation speed are always one
   click or key away.
2. **Set it up first.** A level setup screen chooses world size, terrain generator and seed, and the amount of each
   element (for example "forest density", "lakes", "lava vents"), with presets and save/load of level files.
3. **Configure the physics.** Elements and their interactions are data, not hard-coded branches. A shipped default
   ruleset is a human-editable file; an in-game rules panel toggles each interaction and tunes its rates, and the
   change applies to the running world. Invalid rule files fail with a clear message, never a crash.
4. **Emergent chains.** Interactions compose so one action ripples: fire turns trees to ash and smoke, wind carries
   ash, ash plus water makes fertile soil, plants grow back on it.
5. **Smooth under abuse.** Apply Cascade's lessons so heavy chains slow down effects instead of dropping frames.

## Starting elements and interactions

Elements (v0.1 target, adjust as play testing suggests): air, sand, soil, stone, water, ice, tree (trunk and leaves
may be one element or two), grass or plant, seed, fire, smoke, ash, steam, lava, oil.

World fields: wind (global direction and strength, plus a gust brush); heat or temperature if it makes interactions
simpler to express than pairwise rules.

Example interactions to seed the default ruleset:

- Fire next to tree, plant, or oil ignites it; burning material emits fire and smoke and finally leaves ash.
- Green or wet material ignites less readily than dry material.
- Ash and smoke are light and follow wind; ash settles, smoke dissipates.
- Water extinguishes fire and becomes steam; steam rises and condenses back to water over time (rain).
- Lava ignites flammables, melts ice, and with water becomes stone plus steam.
- Ice melts near heat; water may freeze next to ice when cold.
- Oil floats on water and burns hot and fast.
- Seeds sprout on wet soil; plants spread slowly near water; ash plus water enriches soil so regrowth is faster.
- Wind spreads fire faster downwind.

## Engine lessons carried over from Cascade

- Headless simulation crate with no graphics, windowing, or wall-clock dependency; deterministic for a seed;
  covered by fast unit tests.
- A fixed work budget per update; pending work carries over instead of stretching the frame.
- Prioritize work near the player's cursor and recent actions so their effects show up immediately.
- Bounded, chunked dirty-region texture uploads with a per-frame cap.
- No allocation on the hot path; preallocate world storage.
- Code may be copied and adapted from Cascade (same owner, MIT) with a README credit; do not take a Git or path
  dependency on Cascade crates.

## Performance bar (lightweight)

- 60 fps steady on the reference machine (MacBook Air M2, 16 GB) at the default world size during heavy play,
  including a large forest fire with wind and continuous painting.
- An in-game HUD toggle shows frame time, simulation time, and pending work.
- A headless benchmark or smoke test guards against regressions. No measurement matrices, policy comparisons, or
  research documentation site.

## Platform

Native desktop first (macOS primary), Rust with winit, wgpu, and egui like Cascade.
A browser build on GitHub Pages is a later milestone, so keep the simulation crate wasm-friendly.

## Milestones

1. **Playable sandbox.** Repository bootstrap and CI; simulation core with the data-driven rules model; at least the
   fire, tree, ash, smoke, water, steam, sand, and stone chain; live paint, ignite, erase; play, pause, step, reset,
   speed; brush size; HUD; smooth at the default size.
2. **Configurable.** Level setup screen with element amounts, generators, presets, and save/load; in-game rules panel;
   wind controls and gust brush; remaining starting elements.
3. **Shareable.** Browser build on GitHub Pages; README with screenshots or a short animation; more elements and
   interactions from play testing.

## Non-goals for v0.1

Rigorous performance evidence, bounded-versus-traditional comparisons, a documentation site, multiplayer, audio,
and modding beyond the ruleset file.
