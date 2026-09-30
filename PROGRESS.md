# Progress

## Status
- [x] **M0 — Foundation** (2026-09-30)
  - Workspace (edition 2024, resolver 3), lints, profiles, `scripts/check.sh`.
  - `hearth_core`: identity constants, `ResourceLocation`, `GameDirs`, full `Options` model
    (§4 settings incl. presets → Custom detection), TOML load/save (atomic), sanitize/clamp.
  - `hearth_input`: keys/mouse names, bindings with modifiers, action registry (categories,
    contexts, defaults per spec), `KeyBindings` (rebind, reset, reset-all, conflicts, map
    persistence), `RebindCapture`, `InputState` (hold/toggle, debug chords, context switches,
    scroll steps). 29 unit tests.
  - `hearth_render`: device/surface bring-up with optional-feature detection, present-mode
    selection, clear pass.
  - `hearth` binary: winit app, remembered window placement, display modes (windowed /
    borderless / exclusive with mode selection), F11 toggle, frame limiter, `--game-dir`,
    `--quit-after`. Verified: window opens on RTX 4060 (Vulkan), options.toml written.
- [x] **M1 — Voxel core** (2026-09-30)
  - `hearth_math`: `Planet` (the only wrap implementation: canonical X, shortest deltas, seam
    neighbours, Mercator latitude with north = −Z, sphere points, pole crossing, solar offset,
    planet size presets + auto vertical scale rule), block/cube/column/local positions,
    directions, AABB with swept clipping and ray tests, DDA voxel ray, deterministic hashing/RNG.
  - `hearth_core`: `Registry<T>` (dense ids, overrides, freeze) + `IdMapping` remap for saves,
    `EventQueue`, `FixedTimestep`, `ScheduledTicks`.
  - `hearth_world`: data-driven `BlockDef` (JSON) → `BlockRegistry` with SoA per-state tables
    (flags, light, fluid amount, collision/outline shapes), property parsing/formatting,
    shape families (slab/stairs/fence/door/...), `PalettedBlocks` (1/2/4/8-bit + direct,
    serialization with remap and corruption checks), lazily allocated light, `Cube`,
    `CubeMap` (Arc copy-on-write cubes, wrap-aware access, exact sky heightmap with worldgen
    estimate fallback), raycasts against real shapes, collision gathering.
- [x] **M2 — World generation** (2026-09-30)
  - Planet model (`hearth_worldgen::planet`) on an N×N Mercator grid with sphere metrics
    (2048² for Standard+, 1024² for Tiny/Small; 4.6 s at 2048² on 16 threads): warped
    weighted spherical-Voronoi plates with Euler poles; continental plates chosen for a
    realistic size mix (supercontinent cluster, colliding subcontinent); coasts from active/
    passive margins; elevation (shelf/slope/abyss with crust-age subsidence, collision ranges +
    plateaus, Andean ranges, trenches, discrete island arcs, rifts, fault valleys, old ranges,
    hotspot chains, polar ice plateaus) with slope-rule footprints; Priority-Flood+ε, implicit
    stream-power erosion + talus, protected lake basins (rift, scour, arid); climate from
    currents per basin, upwind continentality, zonal moisture advection with orographic lift
    and rain shadows, dry-season belts, Köppen classes; lakes vs endorheic basins by water
    balance; resolution-independent discharge; volcanoes; zstd save/load (lossless).
  - Regional sampler (`region`): block-resolution height (bicubic grid + hills/ridged/rough
    detail), meandering river channels with floodplains (never uphill), lakes, sea with smooth
    flood fractions and berms, craters, polar plateau, lapse-rate temperature, tree/snow lines,
    36 biomes from climate + highland zones + local conditions, surface materials, tree
    density, spawn search. ~0.1 µs/column amortised.
  - Cube generation (`cubegen`): empty/deep/surface classification, rock by depth (deepslate
    below ~50 blocks and below Y −512), 3D cliff overhangs, ores and rock blobs (27-neighbour
    veins, Y bands scaled by vertical scale, province bias), worm caves (humidity/orogen/depth
    dependent), ravines and slot canyons, flooded systems, giant caverns with pillars, lakes and
    sinkholes, procedural trees for 11 shapes, plants, underwater flora, logs, boulders, cacti;
    order-independent priority-lattice merging. 33–42k surface cubes/s, ~86k deep cubes/s.
  - Data pack: `data/hearth/blocks/*.json` (~190 blocks, templates), loader with overrides.
  - Tools: `bench worldmap` (relief, bathymetry, climate, biome, plates, currents,
    temperature, precipitation, rivers; Mercator + equirect; slices; stats), `bench region`,
    `bench gen` (throughput + top/slice renders).
  - Tests: determinism across threads/orders, cross-cube features, surface agreement ≥99%,
    fast paths, seam twins, rivers never uphill, temperate spawn, climate zone statistics over
    6 seeds, west/east continental patterns, windward vs leeward rain, hypsometry, cavern
    rarity, planet save/load.
- [x] **M3 — Near-field rendering** (2026-09-30)
  - `hearth_texgen`: procedural 16×16 texture pack for all base blocks (animated water strips,
    destroy stages, missing texture); `bench textures` contact sheet.
  - Texture array with coverage-preserving alpha mips; block model baking (cubes with
    overlays/rotation/tint, crosses, stairs/slabs/fences/doors/torches and other box models,
    fluids).
  - Mesher: greedy merging of uniform faces into 16-byte packed quads, 64-byte general quads
    for models/fluids/translucent, smooth lighting + AO, climate tints, cube face
    connectivity for cave culling.
  - Terrain renderer: sub-allocated storage buffers, vertex pulling, reverse-Z infinite
    projection, camera-relative origins, CPU cave-culling BFS + frustum, two-phase Hi-Z GPU
    occlusion culling with compute-emitted indirect-count draws (Vulkan; CPU draw lists
    elsewhere, D19), translucent back-to-front order plus re-sorting of nearby cubes.
  - `hearth --screenshot <spec>` / `--screenshot-list <file>` headless rendering with software
    adapter fallback and `verify_cull` pixel check; `tools/shots/m3.shots` suite.
  - Windowed free-fly preview (`hearth [--seed N]`): background streamer generates, lights and
    meshes cubes around the camera nearest-first and unloads far ones; mouse capture, WASD +
    jump/sneak flying, sprint boost, scroll for speed; status in the title bar.
  - Tests: GPU vs CPU culling pixel equality (`render_cull`), shot spec parsing, allocator.
  - Fixed on the way: WGSL `vec3<u32>` struct padding (64-byte general quads), discrete scroll
    reporting a step every frame (`f64::signum(0.0) == 1.0`), water-depth overflow in tints.
- [ ] M4 — Player & interaction
- [ ] M5 — Light & fluids
- [ ] M6 — Sky & atmosphere
- [ ] M7 — Water rendering
- [ ] M8 — LOD
- [ ] M9 — Entities & AI
- [ ] M10 — Weather & effects
- [ ] M11 — UI & audio
- [ ] M12 — Modding
- [ ] M13 — Optimization
- [ ] M14 — Final QA

## In progress
Direction change to v2 (`docs/spec/v2-direction-change.md`): migration plan next.

## Next steps
1. Write `MIGRATION.md` (v1 milestones/subsystems: Keep / Modify / Replace / Drop) and rewrite
   `PLAN.md` for the v2 milestones; commit.
2. V2-0: content platform (schemas, lint, graph export, hot reload), units, balance presets,
   save versioning + migrations, `docs/design/` skeleton.

## Known issues
- In this environment presents never block (FIFO on both Vulkan and DX12 ran at ~1.5–2k FPS
  with terrain), most likely because the window is occluded. Re-check pacing on a visible
  window; the frame limiter covers the vsync-off case.

## Deferred
- Walking/boating across the seam and over a pole is verified in M4 when physics exists
  (coordinate-level seam/pole tests pass in `hearth_math` and `hearth_worldgen`).
- Biome parameters (plant/tree weights) are in code; making them data-driven is scheduled
  with the data-pack work in M12.

## Environment notes
- Rust was installed via rustup in `%USERPROFILE%\.cargo`; shells started before the install
  need `export PATH="$HOME/.cargo/bin:$PATH"`.
- Machine: Ryzen 7 7840HS (8C/16T), RTX 4060 Laptop + Radeon 780M, 16 GB RAM.
