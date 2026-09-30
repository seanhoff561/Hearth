# Plan

Milestones follow §14 of the build spec. Each milestone ends with `scripts/check.sh` green,
`PROGRESS.md` updated and a git commit.

## M0 — Foundation
Workspace, lints, profiles, `scripts/check.sh`, docs; winit window + wgpu clear; options.toml
load/save/sanitize with presets; input actions, bindings, conflicts, rebind capture, per-frame
state (unit-tested).

## M1 — Voxel core (cubic chunks)
`hearth_math`: `Planet` (wrap), BlockPos/CubePos/ColumnPos/LocalPos, directions, AABB, DDA
raycast. `hearth_core`: registries (namespaced → dense ids), events, tick scheduler.
`hearth_world`: block/property/state registry with precomputed flags, palette cube storage,
cube map, column heightmaps, lazily allocated light arrays. Tests: palette round trips,
coordinate conversions (negative, huge Y, seam), vertical raycasts across cubes.

## M2 — World generation
Noise library (SIMD-friendly batch evaluation, sphere + plane sampling). Planet grid: plates,
crust, boundaries, hotspots, elevation profile (§6.3), distance fields, global erosion,
priority-flood drainage, rivers, lakes, currents, climate, Köppen classes. Regional sampler,
biomes, surface materials, column cache. Cube generator with fast paths, caves (worms, giant
caverns, ravines, deep systems), aquifers, ores, features (trees per species/climate, boulders,
logs, cacti, flora). `tools/bench worldmap` (equirect + Mercator maps, slices). Statistical and
determinism tests listed in §14.

## M3 — Near-field rendering
`hearth_texgen` procedural pack; texture array with alpha-aware mips; binary greedy meshing
with AO/smooth light/tint; packed quads + vertex pulling; buffer sub-allocator + staging ring;
GPU culling (frustum, Hi-Z, face direction) with indirect draws and CPU fallback; cave
visibility graph; translucent sorting; `hearth --screenshot` headless mode.

## M4 — Player & interaction
`hearth_protocol`, `hearth_server` thread, `hearth_client` mirror; physics/collision shared with
entities; movement constants; breaking/placing; items & inventory model; HUD; crafting &
smelting; containers; saves (regions, metadata, players, autosave).

## M5 — Light & fluids
Sky light from heightmaps across cubes, block light BFS, incremental batched updates,
underwater attenuation; water flow, sources, currents, waterlogging; random ticks.

## M6 — Sky & atmosphere
Atmosphere LUTs, sun/moon/stars by latitude & local time, day/night, HDR, tonemap,
auto-exposure, realistic darkness + Purkinje, fog/aerial perspective.

## M7 — Water rendering
Waves, SSR + sky fallback, Fresnel, refraction, absorption, foam, underwater fog, caustics,
god rays, Snell's window.

## M8 — LOD
Quadtree, fast surface generator, cache, meshing, handoff, TAA, curvature; screenshot + fly
benchmark.

## M9 — Entities & AI
hecs world, models/animation, goal selectors, A* pathfinding, 8 mob types, spawning, combat,
drops, taming, riding, packs.

## M10 — Weather & effects
Weather cells, rain/snow/thunder, wet surfaces, wind sway, clouds (blocky + volumetric),
shadows, bloom, particles.

## M11 — UI & audio
All screens incl. planet preview globe and world map; options screens; controller; audio
engine, sounds, ambience, subtitles; localization plumbing.

## M12 — Modding
Data packs, resource packs (vanilla layout mapping, hot reload, PBR), WASM API + examples,
`MODDING.md`.

## M13 — Optimization & verification
Benchmarks, profiling, allocation counter, software adapter run, `BENCHMARKS.md`, cohesion
screenshot suite.

## M14 — Final QA
Fresh-clone build, README/BUILDING/MODDING/ASSETS_LICENSES, soak test, grep for stubs.
