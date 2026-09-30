# Progress

Direction: **v2** (`docs/spec/v2-direction-change.md`) since 2026-09-30; see `MIGRATION.md`.

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

v1's remaining milestones (M4–M14) are folded into the v2 plan (see `MIGRATION.md`, `PLAN.md`).

### v2 milestones
- [x] **V2-0 — Migration & content platform** (2026-09-30)
  - `MIGRATION.md` (every v1 milestone/subsystem: Keep/Modify/Replace/Drop) and the v2 `PLAN.md`.
  - `hearth_content`: pack loader (RON/JSON, bare ids namespaced by pack, unknown fields and
    schema versions checked, file:line diagnostics), typed tables with cross-pack overrides,
    schemas for every §3.1 domain, form × material item generation, units, the two time scales
    (`TimeScales`), balance layer with Authentic/Hardy presets + Custom overrides.
  - `hearth content lint | graph | uncertain | status`; lint (unit ranges, references, knowledge
    cycles/era order, reachability fixpoint, habitats, food webs, effort-from-scratch rollup) runs
    in `scripts/check.sh`; graphs as DOT/SVG/HTML in `docs/generated/`.
  - Seed dataset: 113 materials with real properties, 19 rocks, 26 minerals, 9 provinces,
    7 deposit models, 4 soils, 11 plants, 6 animals, 3 ecosystems, Australopithecus, 15 item
    forms, 21 Era 0–2 processes, 176 knowledge nodes (Eras 0–8, discovery routes for 0–2),
    workstations, construction pieces, garments, injuries, illnesses, 8 eras.
  - `hearth_save`: world dir, `level.json` (format 3) with world settings (planet, life & time,
    era), clock and block-state palette; region files (8³ cubes, zstd); step-by-step migrations
    (2→3); v1 (format 1) refused clearly; unknown states kept as named placeholders.
  - Hot reload: content loaded at startup, F3+T reloads and logs what can't reload live.
  - Removed dropped v1 content (ores and ore bands, MC workstations/utility/building blocks,
    crops, mining tiers); engine tests moved to a test block pack.
  - `docs/design/` skeleton (one doc per system), interaction matrix, future-systems and
    future-humanity designs.
  - Tests: content loads and lints clean; time-scale examples from §4.2; parser/diagnostics;
    format-2 fixture world loads after the format change with a removed block preserved;
    format-1 world refused; region round trips.
- [x] **V2-1 — Time, calendar & seasons** (2026-09-30)
  - `hearth_env` crate: calendar from `time.ron` (ticks → days, local solar time, year
    fraction, hemispheres, moon phase scaled to the year), sun/moon/star positions and day
    length by latitude (polar day/night), seasonal climate from the planet normals (temperature
    wave with lag, diurnal cycle, precipitation seasonality: monsoon/savanna, mediterranean,
    continental, equatorial double peak), year-scale snowpack and lake ice (degree-day and
    Stefan models), permafrost and growing degree days, day-scale weather (advected field +
    convective cells: clouds, rain/sleet/snow, thunder chance, wind, humidity), generic
    phenology, climate codes for GPU tints, sky radiometry in lux (the GPU atmosphere mirrored).
  - Rendering (v1 M6 folded in): Hillaire LUT atmosphere with the planet's shadow and a
    log-space multi-scattering table, calibrated to measured clear-sky and twilight
    illuminance; HDR pipeline with physical lighting (sun/moon, sky, starlight floor, firelight),
    adapting eye with a low-light key, GPU highlight metering, ACES + night shift; sky pass with
    sun and moon discs (phases), rotating stars, drifting cloud layer; aerial perspective;
    overcast grey sky and fog under cloud and precipitation.
  - Weather rendering (v1 M10 part): GPU-generated rain and snow in a camera-wrapped box,
    hidden under cover by a streamed sky-height map; precipitation reduces visibility.
  - Seasons on terrain: grass and leaf colours from climate codes and the date (no remeshing),
    deciduous leaf fall as shader thinning with twigs, dry-season curing and drought leaf loss;
    seasonal snow (layers, buried plants, canopy rules) and lake/river ice laid on loading and
    refreshed every five days of the year on loaded terrain.
  - Preview: world clock, time controls (F3+arrows/W), season and local time in the title;
    screenshots take `lat`, `season`, `yf`, `hour`, `clouds`, `rain`/`sleet`/`snowfall`, `dry`,
    `snow` and log the place's climate.
  - Acceptance: `tools/shots/v21_seasons.shots` (4 seasons × 62°N/42°N/12°N, southern winter,
    sunset, dusk, night, rain, snowfall; 18 shots in ~14 s); tests for solar position and day
    length against known values, calendar, climate, weather, snowpack; headless year on a
    generated planet (snow/ice/wet-dry timing by latitude and hemisphere); GPU vs CPU sky
    consistency; twilight illuminance vs published values; seasonal cover reversible block for
    block.
- [ ] V2-2 — Geology, soils, hydrology & resources
- [ ] V2-3 — Player: character, body & physiology
- [ ] V2-4 — Inventory, carrying & clothing
- [ ] V2-5 — Interaction, process crafting & knowledge
- [ ] V2-6 — Flora framework (temperate first)
- [ ] V2-7 — Fauna framework (temperate forest first)
- [ ] V2-8 — Structural building & shelter
- [ ] V2-9 — Vertical slice review
- [ ] V2-10 — Ecosystem expansion waves
- [ ] V2-11 — Australopithecus & the agent framework
- [ ] V2-12 — Neolithic
- [ ] V2-13 — Metallurgy & mining
- [ ] V2-14 — Late scope: Iron Age & Classical
- [ ] V2-15 — World creation & menus
- [ ] V2-16 — Long-run balance, performance & cohesion QA

## Content Status
Generated by `hearth content status` (Implemented = used by a game system; Planned = data only).

| Domain | Implemented | Planned (data only) |
|---|---|---|
| Materials | 87 | 58 |
| Rock types | 35 | 0 |
| Minerals | 27 | 3 |
| Geological provinces | 14 | 0 |
| Deposit models | 52 | 0 |
| Soils | 16 | 0 |
| Plant species | 0 | 11 |
| Animal species | 0 | 6 |
| Ecosystems | 0 | 3 |
| Hominin species | 0 | 1 |
| Item forms | 0 | 15 |
| Processes | 0 | 21 |
| Knowledge nodes | 0 | 176 |
| Workstations | 0 | 2 |
| Construction pieces | 0 | 5 |
| Garments | 0 | 6 |
| Injuries | 0 | 10 |
| Illnesses | 0 | 5 |
| Eras | 0 | 8 |

## In progress
V2-2 — geology, soils, hydrology & resources. Done so far:
- (a) Rock types as blocks generated from content (33 rocks, textures from material
  appearance); 14 geological provinces assigned from the tectonic history with climate and age
  conditions; stratigraphy (varying thickness, pinch-outs, folds in collision belts, domes and
  basins elsewhere) cut by today's terrain with exhumation of uplifted land; plutons;
  basement; geothermal gradient; `stone`/`deepslate`/rock veins removed; `bench worldmap`
  province and rock layers and block-scale `--geo-area` maps and sections. Generation speed
  unchanged (≈48k surface, ≈80k deep cubes/s).
- (b) Soils: 16 soil types chosen per column by formation fit (climate, parent rock,
  vegetation and landform, drainage, slope) with horizons as blocks, slope-thinned; soils and
  sediments as blocks generated from their materials (v1 dirt/sand/gravel/clay family
  removed); beaches, beds, salt flats and glaciers; loose stones of the local rock and scree;
  `--geo-area` sections draw soil profiles and print soil statistics.
- (c) Deposits: 52 models covering every Appendix D resource (veins, seams, nodule bands,
  placers, disseminated bodies, crusts, flows, bog ores, evaporites, pipes) placed per
  256-block cell by province, host rock and conditions, cached per cell and drawn per cube;
  surface indicators (stains, gossans, float, crusts, placer gravel and cobbles) drawn on
  slopes too; ore, crust, placer and float blocks generated from the content; panning as a
  pure function (`Deposits::pan`); planned metal materials (tin, iron, lead, zinc, mercury,
  aluminium) and ore yields; per-province pluton share and roof depth so shields and old
  orogens expose granite; province climate conditions became preferences (D45); resource eras
  aligned with the technology eras (D47). Coverage (`hearth_worldgen::coverage`, D44): every
  Era 0–2 need is within reach of every continent on seeds 1–6 at Standard; the lint
  (`hearth content lint --coverage`) flags the regional gaps (kimberlite, coal, fire clay,
  travertine, volcanic ash…). Tests: bodies only in their provinces, host rocks and climates,
  frequencies match the content, early resources reach every continent, panning.
  `bench deposits`, `bench worldmap` deposit layer and coverage table,
  `tools/shots/v22_deposits.shots` (ochre, laterite, limonite, salt pan, obsidian flow,
  copper stains, gossan, placer gravel, river cobbles, fumarole sulfur).
  Fixed a v1 bug: tree and debris placement drew their chance from 8 and 4 hash bits, so any
  biome with a non-zero tree density (steppe, savanna, plains, scrub) grew a closed forest.
- **Interjection — distant terrain (done):** screenshots faded the terrain out at a fixed
  distance. Cause: a render-distance fog in the terrain shader and, at the root, no LOD
  terrain at all (v1 M8 had been left for V2-6). Fixed by building the core of v1 M8 now
  (`hearth_lod` quadtree, fast surface sampling, meshes with skirts, `hearth_render::lod`,
  preview streaming on its own thread pool, screenshots waiting for every tile with a loud
  timeout; the LOD always reaches the horizon), replacing the fog with physical aerial
  perspective (Rayleigh per colour and humidity-driven aerosol through an exponential
  atmosphere, precipitation extinction), planet curvature, and a dithered handoff with the LOD
  just behind the cubes and forest floors under LOD canopies. Regression test `lod_horizon`
  (depth readback from a peak at LOD 512, fog on and off: 43 % of the ground below the horizon
  beyond the full-detail area, 0.01 % empty); `tools/shots/lod_horizon.shots`. Also fixed:
  reversed-edge `smoothstep` in the sky shader (undefined on Vulkan). D52–D54.
- (d, part 1) Groundwater and coasts: water table from the drainage base, the smoothed land,
  rock permeability (new rock data) and climate; cave voids below it flooded (replacing v1's
  random aquifers); springs where the table meets slopes, desert oases and mineral springs
  over deposits (salt, sulfur, hot, travertine), each with a pool and a downhill brook; water
  quality of sea, salt and fresh lakes, streams, springs and groundwater. Coasts: sheltered
  shores (noise and river mouths) become mangrove (new biome, prop-rooted trees in the
  shallows) or salt marsh (cordgrass on mud) with mudflats; fringing, barrier and atoll reefs
  in warm clear shallows (coral over reef limestone, coral fans); kelp on rocky floors in cool
  water, wrack on cool rocky shores, tide pools on stony shores; sea ice in the seasonal cover
  (below −4 °C air). Tests: water table shape (deeper under hills, in dry country; shallow in
  humid lowlands), no dry void below the table, springs and brooks, water quality, reefs only
  in warm clear shallows, marsh/mangrove by climate, kelp only in cool water, sea ice by
  climate. `bench deposits --springs/--find`, `tools/shots/v22_coasts.shots`. Fixed a v1 bug:
  underwater plants were never placed (the feature writer never let anything replace water).
  Generation cost of the new passes ≈ 5 % of surface cubes.
- (d, part 2a) Seasonal rivers: river flow regimes from the year-scale water balance
  (landscape melt bands, evaporation, storm runoff, fast and ground stores with baseflow by
  wetness), summed over each reach's upstream basin with flood-wave travel times in the
  frequency domain (`hearth_env::rivers`, D55); river columns rise and fall with them in the
  seasonal cover (Manning stage, floods over the banks that thin out to the surrounding land,
  low water baring bars, small rivers running dry; drowned plants and stranded water plants
  restored); `SeasonCover` now holds the cover's states, the regimes and what it buried.
  Tests: regimes by climate (snowmelt, savanna, oceanic), a desert reach flooding late from
  distant rains, confluences mixing by water, the full spectrum keeping the year, cover
  floods/low water/dry beds restored block for block, and a year on a generated world (a
  reach floods and falls and returns to the same blocks). `bench deposits --rivers`,
  `tools/shots/v22_rivers.shots` (snowmelt flood, summer, winter low; savanna wet and dry).

## Next steps
0. **Interjected — rendering performance audit (in progress; resume here).** Read
   `docs/perf-audit.md` (the audit table, plan and results) and `BENCHMARKS.md`. Done and
   committed: the benchmark (`hearth bench`, 1e04b39, f2378b4), distant trees in the LOD like
   Distant Horizons (D56, 038759c, an interjection the user asked for), and results 1–6: sky
   light cache (bae343b), no per-frame allocations in our frame code + cached Hi-Z bind group
   (564e0bc), LOD quads as pooled 16-byte records drawn with one multi-draw (5556f36), LOD
   occlusion-culled on the GPU against the near terrain's Hi-Z (37014f2), LOD streaming without
   holes and in-view first (f0b9339), dithering in the final pass (4ce9fa8), and (7)
   screen-space-error LOD selection (D57; `lod_detail`: Fancy 2 px, Fabulous 1 px, Fast 4 px;
   `hearth bench --lod-error PX` overrides it, 0 = the old distance rule). A staging belt for
   per-frame uploads was tried and reverted (no gain). Remaining, in order:
   - (8) LOD quads grouped by face direction (sort each tile's quads by face; per-face counts
     in `GpuTile`; `lod_cull.wgsl` emits one draw per front-facing, non-empty group — for a
     tile wholly on one side of the camera the two side groups facing away are skipped, and
     the tops when the camera is below the tile). Pixel-identical; wins back part of (7)'s
     cost (the LOD pass is vertex-bound: ~0.33 ms per million triangles).
   - (9) Render scale with a spatial upscaler (FSR 1 EASU + RCAS, MIT) as an option, off by
     default; the `render_scale` option exists but is not applied. Temporal upscaling waits
     for TAA/motion vectors (V2-6).
   - (10) Performance gate `scripts/perf-gate.sh`, run at the end of every milestone: `hearth
     bench --scenes quick` against a stored baseline, failing on >5 % lower average FPS or
     1 % lows, recorded or justified in DECISIONS.md. Measured noise: about ±3–10 % between
     identical runs of one build (laptop clocks), so the gate needs repeated runs (median of
     3+) and pooled 1 % lows, and the baseline must be recorded per machine (the JSON holds
     the adapter name; skip with a warning on another GPU). For claims, A/B alternate builds
     (a `git worktree` of the previous commit in the scratchpad, built with `cargo build
     --release -p hearth`, run alternately with `bench --report none --json none`).
   - Then update `docs/perf-audit.md` (final table), DECISIONS (the gate rule), PROGRESS, and
     resume V2-2 at (d, part 2b) below.
   Golden images for SSIM checks: `bench-out/p7/golden0` (at result 7's commit with
   `--lod-error 0`, i.e. result 6's look); capture the current look with `hearth bench
   --golden DIR` before comparing new work. Submission hitches of 40–50 ms (driver) make
   single-run 1 % lows unreliable, underwater especially.
   Known issues found on the way: underwater views are dark with an empty region beyond the
   full-detail area (no underwater light/fog, no LOD sea floor — V2-2e); the preview passes
   no firelight to the eye's adaptation (the render thread lacks the map; torch-lit caves at
   night would be overexposed in the preview — the benchmark passes it).
1. (d, part 2b) Finite conserved player-moved water with levelling and flow (v1 M5 fluids)
   and per-block quality; coastal salt pans.
2. (e) Water rendering (v1 M7) and ice.
3. (f) Minimal spawn picker; `worldmap` soil layer; V2-2 acceptance review and commit.

## Known issues
- In this environment presents never block (FIFO on both Vulkan and DX12 ran at ~1.5–2k FPS
  with terrain), most likely because the window is occluded. Re-check pacing on a visible
  window; the frame limiter covers the vsync-off case.

## Deferred
- Weather visuals not drawn yet: lightning flashes and thunder, fog banks, wet and snowy
  surface shading, splashes and puddles; rain streaks alias at a distance (V2-16 polish).
- No shadow maps: direct light reaches faces open to the sky; cast shadows come with the
  rendering polish (V2-16 at the latest).
- Snow is laid per column (no drifts against obstacles, no snow on steep faces, no avalanche);
  species phenology calendars replace the generic plant types in V2-6.
- Walking/boating across the seam and over a pole is verified in M4 when physics exists
  (coordinate-level seam/pole tests pass in `hearth_math` and `hearth_worldgen`).
- Biome parameters (plant/tree weights) are in code; making them data-driven is scheduled
  with the data-pack work in M12.
- Panning is a world query only: the panning action, its pan-concentrate items and ore
  washing come with the process engine (V2-5) and ore processing (V2-13). Placers do not yet
  trace back along the river network to their source body, and deposit systems have no
  zoning (oxide cap over sulfide ore) — both with V2-13 prospecting.
- Fumarole ground is snowed over like any other (warm ground melting snow comes with the
  thermal model, V2-5); the dead-bush sprite reads as a dark wedge from above (V2-6 flora).

## Environment notes
- Rust was installed via rustup in `%USERPROFILE%\.cargo`; shells started before the install
  need `export PATH="$HOME/.cargo/bin:$PATH"`.
- Machine: Ryzen 7 7840HS (8C/16T), RTX 4060 Laptop + Radeon 780M, 16 GB RAM.
