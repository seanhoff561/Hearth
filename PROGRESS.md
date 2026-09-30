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
- [ ] M2 — Worldgen
- [ ] M3 — Near-field rendering
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
M2 — world generation.

## Next steps
1. `hearth_worldgen` noise library (sphere + plane, batched).
2. Planet grid: plates, crust, boundaries, elevation profile, erosion, drainage, climate.
3. `tools/bench worldmap` to visualise the planet.
4. Regional sampler, column cache, cube generator, caves, ores, features; tests.

## Known issues
- With Vulkan FIFO on this Optimus laptop the clear-only loop reported ~2.8k FPS during the
  4 s smoke run (window possibly occluded). Re-check present pacing once real frames exist;
  consider DX12 as the default backend on Windows if Vulkan presentation misbehaves.

## Deferred
(none)

## Environment notes
- Rust was installed via rustup in `%USERPROFILE%\.cargo`; shells started before the install
  need `export PATH="$HOME/.cargo/bin:$PATH"`.
- Machine: Ryzen 7 7840HS (8C/16T), RTX 4060 Laptop + Radeon 780M, 16 GB RAM.
