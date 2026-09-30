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
- [ ] M1 — Voxel core
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
M1 — voxel core.

## Next steps
1. `hearth_math` coordinate types with the wrap-aware `Planet`.
2. `hearth_core` registries/events/tick scheduler.
3. `hearth_world` block state registry, palette cubes, cube map, heightmaps.
4. Tests listed in PLAN.md M1.

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
