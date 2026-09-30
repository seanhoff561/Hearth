# Hearth — Architecture

This document is the design contract for the engine. It describes how the crates fit together,
which thread owns what, and the data flow of the major systems. Decisions and their rationale
live in `DECISIONS.md`; status lives in `PROGRESS.md`; per-system design notes (v2) live in
`docs/design/`.

## 1. Crate graph

```
hearth_math ─┐
hearth_core ─┼─> hearth_content (data packs → typed tables, lint, graphs, time scales, balance)
             │
             ├─> hearth_world ─> hearth_worldgen ─> hearth_lod
             │        │                 │
             │        │                 └─> hearth_env (calendar, sun/moon/stars, seasonal
             │        │                      climate, weather, snow & ice, phenology, sky
             │        │                      radiometry in lux) [math, content, worldgen]
             │        │                 │
             │        └──> hearth_entity (hecs ECS, physics, AI, pathfinding)
             │                 │
             │   hearth_save (level.json, regions, state palette, migrations) [world, worldgen, content]
             │                 │
             │   hearth_protocol (client<->server messages; serializable)
             │                 │
             │   hearth_modapi (stable API traits + WIT; wasmtime host)
             │                 │
             │   hearth_server (authoritative 20 TPS simulation thread)
             │
hearth_input, hearth_audio, hearth_ui (toolkit + screens, no GPU), hearth_texgen
             │
hearth_render (wgpu; consumes world meshes, LOD tiles, UI draw lists)
             │
hearth_client (client world mirror, interpolation, prediction, meshing jobs, HUD state)
             │
hearth (binary: launcher, main loop, window, options wiring)
tools/bench (headless benchmarks, worldmap PNGs, screenshot suite driver)
```

Rules:
* Lower crates never depend on higher ones. `hearth_math` and `hearth_core` depend on nothing
  engine-specific.
* Only `hearth_render` (and the binary) touch wgpu; only `hearth_input`/binary touch winit
  event types; only `hearth_audio` touches the audio backend.
* "Vanilla is a mod": base content is a data pack (`data/hearth`) loaded exactly like a
  third-party pack; code implements mechanisms only (v2 §3.1).

## 2. Threads

| Thread | Owns | Talks to |
|---|---|---|
| Main (winit event loop) | window, input, client state, renderer, UI, audio control | server via channels; workers via job queues |
| Server | authoritative world, entities, 20 TPS tick, saving triggers | client via `hearth_protocol` channels; workers |
| Worker pool (rayon + priority job system) | cube generation, lighting batches, meshing, LOD building, region IO compression | results returned through lock-free channels |
| IO thread | region file reads/writes, LOD cache writes | server/client via channels |
| Audio thread (kira backend) | mixing | control handles from main |

The main thread never blocks on workers: it polls completed-result channels with a per-frame
integration budget (bytes uploaded, meshes integrated).

## 3. Coordinates and the wrapping planet

* Block positions are `i32` on all axes; Y is unbounded in practice (soft limit ±2²⁴).
* The planet has circumference `C` (a multiple of 4096). X wraps modulo `C`; Z spans `[-C/2, C/2]`
  (Mercator latitude ±85°). `hearth_math::Planet` owns `C` and every wrap-aware operation:
  canonicalising X, shortest signed X delta, cube-neighbour lookup across the seam,
  wrap-aware distances. Nothing else implements wrapping.
* Cube coordinates `(cx, cy, cz) = floor(pos / 16)`. Canonical cube keys always have
  `0 <= cx < C/16`.
* Latitude φ(z) = atan(sinh(z / R)), R = C / 2π; longitude λ = 2π x / C.
* Rendering is camera-relative: the camera sits at the origin of render space; world positions
  are converted with f64 math and the shortest wrapped X offset before becoming f32.
* Crossing a pole edge (|z| > C/2) re-enters at `x + C/2` with mirrored z and reversed heading.
  The last degrees before each pole are a flat ice plateau in permanent whiteout so the
  transition is invisible.

## 4. World data (hearth_world)

* **Cube** = 16³ blocks. Block states are palette-compressed (`Uniform(state)` or bit-packed
  indices with 1/2/4/8/16 bits); uniform cubes (air, stone fill) allocate nothing.
* **Light**: sky and block light as nibble arrays, allocated lazily. Sky light is seeded from a
  per-column "highest light-blocking block" heightmap initialised from `surface_height` before
  real cubes exist, then corrected as cubes load. Cubes entirely above the heightmap are
  implicitly fully sky-lit.
* **Block states** are registered from data; each state has precomputed flags (opaque, full
  cube, light emission, light opacity, render layer, collision shape, occlusion faces).
* **Storage**: `FxHashMap<CubePos, CubeSlot>`; per-column records hold heightmaps and the loaded
  Y range.

## 5. World generation (hearth_worldgen)

Pipeline, from coarse to fine; every stage is a pure deterministic function of the seed and
position, so any cube can be generated alone in any order on any thread:

1. **Planet grid** (world creation, saved in the world folder): an `N×N` grid over the Mercator
   world square (default N = 2048) with sphere-correct metrics (cells are weighted by cos φ).
   Holds tectonic plates, crust type, boundary classes, macro elevation, global stream-power
   erosion, drainage (priority-flood, flow directions, discharge, lakes, rivers), ocean
   currents and the climate fields. Built in parallel with a progress callback.
2. **Regional sampling**: `surface_height(x,z)`, `water_level`, `biome`, `river`,
   `surface_material`, `tree_canopy`, `snow_line`, `tree_line` combine bicubic grid samples
   with block-space detail noise (analytic erosion-style ridges, hills) and river channel
   geometry refined from the grid's river network.
3. **Column cache**: per 16×16 column, the 2D sample set (heights, biome, materials, river
   info) is computed once and LRU-cached; cube generation and LOD share it.
4. **Cube generation**: cheap classification first (all air / all solid fill / needs full
   evaluation) using the column's min/max surface, the 3D-noise amplitude bound and cave
   bounding volumes. Full evaluation fills terrain, water, surface layers, caves (worm networks,
   rare giant caverns, ravines — all with explicit bounding volumes), aquifers and ores.
5. **Features** (trees, boulders, logs, cacti, plants): each feature is a pure function of its
   origin; a cube collects every feature whose bounds intersect it, sorts them
   deterministically, and writes only its own blocks. No neighbour dependency.

## 6. LOD (hearth_lod)

Quadtree over the wrapped XZ plane; level n = 2ⁿ blocks per column; tiles of 64×64 columns.
Columns store 1–4 vertical segments (bottom, top, material/colour, light). Far tiles are
sampled directly from the regional functions (never by generating cubes); real cubes replace
level-0 data when generated or edited and propagate upward. Tiles are meshed to column meshes
with skirts, culled on the GPU with the terrain path, crossfaded (dithered) at the near/far
handoff, and cached on disk (lz4).

## 7. Rendering (hearth_render)

Frame graph (HDR RGBA16F, reverse-Z infinite projection, camera-relative; `SceneRenderer`
records it, lighting arrives in lux from `hearth_env` and is pre-exposed by the adapting eye):
0. Atmosphere LUTs (compute): transmittance and multiple scattering when the aerosol density
   changes, the camera's sky view every frame (`sky.rs`, `atmosphere.wgsl`).
1. CPU: cave-culling visibility BFS over the cubes' face-connectivity bits + frustum test →
   candidate cubes (camera-relative origins uploaded as instances), front to back.
2. Opaque terrain, two-phase GPU culling (Vulkan): a compute pass emits indirect draws for the
   candidates that were visible last frame (per-face-direction culled) and they are drawn; a
   Hi-Z pyramid (reverse-Z: min depth per texel) is built from that depth; a second compute
   pass tests every candidate against it, draws the newly visible ones and updates each cube's
   visibility bit. Draws go through `multi_draw_indexed_indirect_count`. Other backends build
   the same draws on the CPU (`multi_draw_indexed_indirect`), because DX12 indirect-count
   draws don't apply the base vertex / first instance to the shader builtins (see D19).
   Then LOD and entities.
3. Sky pass where the depth is still clear: sky view, sun and moon discs, stars, the cloud
   layer, the overcast grey (`sky.wgsl`).
4. Translucent terrain (water, ice) back to front; rain and snow particles (`precip.rs`).
5. Highlight metering (compute histogram, no readback) → ACES tonemap with the night shift
   (`post.rs`). Planned: water shading (V2-2), volumetrics, TAA, bloom, grading, UI.

Terrain geometry lives in two large storage buffers managed by a first-fit sub-allocator:
16-byte packed quads for full-cube faces (greedy-merged where light, AO and tint are uniform)
and 64-byte general quads for models, fluids and translucent surfaces. The vertex shader
expands quads from the buffers (vertex pulling) with one shared quad index buffer. Translucent
quads of cubes near the camera are re-sorted back to front when the camera changes block.
`hearth --screenshot` renders the same frame graph offscreen (software adapter fallback) and
can verify that GPU culling never changes a pixel.

## 8. Client/server

The integrated server runs on its own thread at 20 TPS and talks to the client exclusively
through `hearth_protocol` message enums over crossbeam channels (the same types a network layer
would serialize). The client mirrors cubes it is sent, meshes them, predicts the local player's
movement with the shared physics code, and interpolates everything else between ticks.

## 9. Modding

* Data packs (`data/<ns>/…` JSON/RON) define blocks, items, recipes, loot, tags, biomes,
  features, spawn rules; loaded in pack order with overrides.
* Resource packs follow the standard vanilla folder layout; a mapping table converts vanilla
  names to `hearth:` ids. Packs stack; F3+T hot-reloads.
* WASM components (`mods/*.wasm`, wasmtime component model, WIT API) register content, hook
  events and query/modify the world through capability-checked host functions. Traps are
  contained per mod.
* Saves store the id→name palette so adding/removing mods never corrupts a world; states the
  content no longer defines load as named placeholders and are written back unchanged.

## 10. Saves

`saves/<world>/` holds `level.ron` (metadata, seed, planet settings, game rules, time),
`planet.bin.zst` (the planet grid), `region/r.X.Y.Z.hrg` (16³ cubes per region file, zstd with a
trained dictionary), `players/<uuid>.ron`, `lod/` (lz4 LOD cache) and `ids.ron` (registry
palettes).
