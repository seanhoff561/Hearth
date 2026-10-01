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
- [x] **V2-2 — Geology, soils, hydrology & resources** (2026-10-01)
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
  - (d, part 2b) Finite water (D61, `hearth_world::water`): natural water stays a sustained,
    never-simulated reservoir that feeds what opens beside or under it and drains what comes
    down onto it; finite water is integer litres with salinity, germ risk and temperature per
    block, shown in eighths (`water[level=1..8]`; level 0 = natural). It falls, evens out with
    lower neighbours, and at rest levels per connected body (connected wells, flat ponds,
    channels at their river's level); wells fill by seepage below the water table (rate by rock
    permeability); water mixes, evaporates by the mass-transfer law (`water_env`) leaving salt as
    `salt_crust` blocks with their kilograms (dissolved again by water), takes the air's
    temperature and grows germs when still and warm. `pour`/`take`/`block_changed`/`restore`
    for the world loop of V2-3; `hearth::water_env::WorldWater` gives it the world. Coastal
    salt pans (sabkhas) on hot desert coasts. Tests: levelling and conservation, downhill flow,
    connected vessels, a channel fed from the sea and water draining into it, a well filling to
    the table and refilling, mixing/evaporation/crust/dissolving, displacement and spilling,
    taking from finite and natural water; in a generated world a channel from a river fills to
    its level with river water and a well fills to the table (`tests/finite_water.rs`); salt
    pans only on hot dry coasts with rock-salt crusts (`tests/coasts.rs`).
  - (e) Water rendering (v1 M7, D62–D64): wind-driven waves, Fresnel sky reflection, sun
    glitter, refraction of a scene copy absorbed by depth, shore foam, Snell's window; distant
    water shaded to match; under water, light by real depth with caustics from a map of water
    surfaces around the camera and the view through the water; screen-space reflections at
    High; glossy translucent ice. Tiers by `shader.water` (`hearth bench --water`).
  - Interjected rendering performance audit (`docs/perf-audit.md`, D56–D60): the benchmark
    (`hearth bench`), distant trees like Distant Horizons, screen-space-error LOD selection,
    LOD culled on the GPU and grouped by facing, render scale with FSR 1, the performance gate.
  - (f) `bench worldmap` soil layer with each soil's land share; the minimal globe spawn picker
    (world-map key M in the preview; `--screenshot "globe=1"`): drag, zoom, the place under
    the cursor in the title, a click goes there (from the sea to the nearest coast,
    `Terrain::spawn_near`).
  - Acceptance (`docs/design/geology.md`, "Acceptance (V2-2)"): `worldmap` geology, soil and
    deposit layers; deposits only in their provinces at the content's frequencies
    (`tests/deposits.rs`); every Era 0–2 need within reach of every continent on seeds 1–6,
    the coverage lint flagging the gaps for eras 0–5. Performance gate (D65): average FPS
    forest −9.0 %, summit −8.0 %, cave −3.2 % (1 % lows −6.5, −3.2, −3.3 %) against the end of
    the rendering audit — the water's shading at Medium, accepted; the benchmark's map
    building moved off its frame thread and the water's scene copy trimmed on the way. The
    baseline moved to this commit.
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
V2-3 — player: character, body & physiology. Done so far:
- (a) The body (`hearth_body`, D66): heat balance of a core and eleven skin regions (after
  Gagge; clothing per region, wind and wetness, sweat, rain, immersion, ground, sun and fire;
  shivering, sweating, blood flow; frostbite), food energy (stomach, glycogen, fat, protein
  share, fresh-food vitamins), body water (sweat, insensible, kidneys, salt), sleep (two
  processes, wake reasons), stamina, injuries (bleeding, clotting, infection, healing on their
  scales, treatments), illness (causes, onset, course, fatal courses turned by treatment),
  death, effects for movement and actions, and feelings for the Body panel. Data: stamina in
  `human.ron`, fur leggings, fur mittens 1.5 clo, closed fractures. Acceptance tests
  (`tests/acceptance.rs`: hypothermia naked in cold rain vs furs by a fire, death by thirst,
  sprain vs fracture) and realism tests against human data (`tests/realism.rs`, twelve).
- (b) Movement (`hearth_physics`, `hearth_player`, D67, `docs/design/movement.md`): the box
  swept against block shapes, gaits at human speeds (sprint on stamina above the aerobic
  threshold), jumps of 0.45 m, steps in stride, scrambles up a block, ledges to head height
  climbed with both hands, crouch (edges kept) and crawl (low gaps), wading and swimming with
  breath, ladders, ice; landings by speed softened by snow, leaves or water, judged by the body
  (`Body::land`); the player joining body and mover (effects → ability, motion → activity and
  immersion, drowning, limp bodies sink). Blocks gained `cushion`; treading water (3.5 METs).
  Tests: eleven of movement, five of the living player, falls by height.
- (c) The world loop (D68, `docs/design/world-loop.md`): `hearth_protocol`; the integrated
  server (`server.rs`, replacing the streamer) at 20 TPS owning the world, the clock, the
  player's body in the exposure where they stand (weather, sun, night sky, shelter, water) and
  the finite water (ticked, restored in cubes that load again, woken by the seasonal cover),
  streaming cubes with their blocks and meshes, saving `level.json` and `player.json` every
  five minutes and on stop; the client (`client.rs`, replacing the preview) mirroring cubes,
  moving the player every frame and reporting at 20 Hz, first-person camera, free camera
  (F3+N), crawl key (C), double-tap sprint, respawn after death, `--world NAME`. The first
  spawn is warm-temperate. Test: a world lives, saves and comes back (`tests/server.rs`).
- (d) The interface (in parts): `hearth_ui` with a clean-room pixel font drawn for the game,
  draw lists at a whole-number interface scale, the words from `lang/<code>.json` (English
  under every language), immediate-mode widgets (buttons, sliders, switches, cyclers, text
  fields, lists; keyboard focus); `hearth_render::ui`; the screens (`menus.rs`): title, worlds,
  new world, pause (stops the clock and body), options (video, controls with rebinding,
  accessibility, language), the F3 debug screen and the death message. Controllers (gilrs,
  `gamepad.rs`): the left stick walks (full lean jogs, click sprints), the right stick looks
  (squared response), South jumps, East crouches, West crawls, Start pauses, Select opens the
  globe; in the menus the D-pad or stick moves the focus, South presses, East goes back.
  Sound (D69, `docs/design/audio.md`): `hearth_audio`, every sound synthesised as it plays
  (footsteps on twelve surfaces by gait, landings, splashes, strokes, gasps, hurts, clicks;
  wind with gusts and whistle, rain under the sky, a roof or rock, the hush under water, the
  heart and the breath from the body), mixed by the options' categories, muffled under water,
  echoing when shut in, paused with the world, through cpal; `hearing.rs` turns movement, the
  ground's sound groups (natural blocks by material), the weather and the body into sounds;
  the Sound options (volumes, output device, captions) and captions on screen. Tests: the
  mixer offline (every sound heard and ended, volumes, wind, rain, water, rhythms, echo,
  limiter), hearing (surfaces, enclosure, footstep pace, splashes); `bench-out/sounds/*.wav`.
- (e) The person (D70, `docs/design/character.md`): `hearth_character` with appearances
  (body, height, build, skin tone and undertone, eleven hair styles, facial hair, hair colour
  by name or hue, eyes, name, loincloth) kept as profiles (`characters.json`) and in each
  world's `player.json`. A seventeen-joint box rig in anthropometric proportions. Procedural
  movement with real gait phases: stance and swing, a flat foot rolling to the toes, flight
  between running strides, a bob of 4 cm walking, footsteps heard and seen in step. Also
  crouch, belly crawl, breaststroke, treading water, ledge and ladder climbing, falling,
  lying, a shivering hug. Drawn as instanced boxes lit like the terrain
  (`hearth_render::figure`): the first-person body seen looking down (eyes in the posed head),
  third person behind or in front (F5), a character screen with a turning preview under four
  lights, and the person chosen for a new world. A female body starts with a chest band (D70,
  a garment in the data). Tests: proportions, poses in their boxes, gaits that stride and
  bob as people do, skin tones in order, profiles, hair, first person;
  `bench-out/character_*.png`; `--screenshot person=4` / `body=true`.

## Next steps
0. Every milestone ends with `scripts/perf-gate.sh` (≈10 min: builds the baseline commit in
   `perf/baseline` in `bench-out/gate`, three alternating rounds of the quick scenes); a fall
   of more than 5 % in average FPS or 1 % lows is fixed or justified in DECISIONS.md and the
   baseline moved with `--accept` on a committed tree (then commit `perf/baseline`). For
   one-off claims, A/B alternate builds as the gate does (or `--lod-error` / `--render-scale`
   / `--water` within one build); capture golden images with `hearth bench --golden DIR`
   before comparing looks.
1. V2-3 — Player: character, body & physiology (PLAN.md). Done: (a) the body, (b)
   movement, (c) the world loop, (d) the interface, controllers and sound, (e) the person
   (appearance, rig, movement, views, character screen). Next: (f) the diegetic HUD, Body
   panel (B), Guided HUD, sleep with time acceleration, death and respawn rules, the
   acceptance review and the performance gate.

## Known issues
- The preview passes no firelight to the eye's adaptation (torch-lit caves at night would be
  overexposed in the preview; the benchmark passes it).
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
- Death under Legacy rules leaves no body or belongings where the player fell, and no journal
  of what the dead knew: both need items (V2-4) and knowledge (V2-5). Hardy comes back at the
  world's first spawn until there are camps (V2-8).
- Sounds are all the player's own or around them: sounds placed in the world (direction,
  distance, occlusion) come with fauna (V2-7) and the work of the hands (V2-4/V2-5); no
  music, thunder, fire, flowing water or animal sounds yet (their volume sliders are hidden);
  the echo has one character, not measured from the cave's size; captions show no direction.

## Environment notes
- Rust was installed via rustup in `%USERPROFILE%\.cargo`; shells started before the install
  need `export PATH="$HOME/.cargo/bin:$PATH"`.
- Machine: Ryzen 7 7840HS (8C/16T), RTX 4060 Laptop + Radeon 780M, 16 GB RAM.
