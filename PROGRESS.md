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
- [x] **V2-3 — Player: character, body & physiology** (2026-10-01)
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
    third person behind or in front (mouse button 4), a character screen with a turning preview under four
    lights, and the person chosen for a new world. A female body starts with a chest band (D70,
    a garment in the data). Tests: proportions, poses in their boxes, gaits that stride and
    bob as people do, skin tones in order, profiles, hair, first person;
    `bench-out/character_*.png`; `--screenshot person=4` / `body=true`.
  - (f) What the body tells the player (`docs/design/hud.md`), sleep and death: lying down
    (Z) to rest, a sleepy body at ease dropping off in seconds and the world easing up to 90×
    while it sleeps, waking for a reason said as the eyes open (rested by the two-process model
    toward morning); sight through the body (colour drained by exhaustion, thirst and
    weakness, dimming, edges closing in red with the pulse, the cold's blue, heat's wavering),
    a rumbling stomach and dry swallows, muffled hearing when weak, a shivering view, breath
    fog in cold air; the Body panel (B: regions with injuries, bandages and splints, states and
    injuries in words); the optional Guided HUD; death by the world's rules (Legacy, Hardy,
    Permadeath with the life's tale and the world ended). Garments, injuries and illnesses
    are now used by the body (content status Implemented).
  - Acceptance review: naked in 5 °C rain the core falls below 35 °C after 1.0 h of body time,
    in furs by a fire it holds 36.4 °C; without water a hot, active body dies after 3.35 days
    (11 resting in the shade); a sprain heals in 3.7 game days on the day scale (89 real
    hours), a fracture in 3.4 game days on the year scale (39 real days)
    (`hearth_body/tests/acceptance.rs`). Every item of PLAN.md's V2-3 is in: the engine's
    protocol, integrated server, client mirror, shared physics and player saves; the interface
    toolkit with its own font, screens, options, controllers, words and sound; the character
    creator with profiles, the rig, the first-person body and the movements; the physiology
    with sleep and death modes; the diegetic HUD, Body panel, Guided HUD and the body's sounds.
- [x] **V2-4 — Inventory, carrying & clothing** (2026-10-01)
  - (a) Things (`hearth_items`, D71, `docs/design/inventory.md`): kinds from the content (form ×
    material, explicit items, garments × material) with mass, volume, footprint, box and colour;
    containers (bundle, pouch, water skin, basket, back basket) with grids and loads, nesting;
    stacks named by id; a log section (≈90 kg of oak).
  - (b) Carrying: a thing in each hand (12 kg) or both arms (half the body's mass), drags (three
    times it), garments by layer and region with attachment points (the loincloth's tie: one
    small thing; a belt's three loops), a back load; moves by path, all or nothing. The load on
    the body: walking slower from a fifth of the body's mass, no running above two fifths, no
    sprint above a quarter, lower jumps, no climbing with full hands, a drag's pace by the
    ground's friction (120 W of pull), the work by Pandolf's equation. The body's insulation
    from what is worn; new people dressed in real garments.
  - (c) Things in the world (`items.json`): put down (G), picked up (E), dragged (hold F) and let
    go; loose stones gathered as cobbles; belongings left where one fell (Legacy, Hardy); drawn
    as boxes, in the hands (held forward) and trailing behind; a crosshair that says what can
    be done.
  - (d) The inventory screen (Tab): the body's places and every carried container's grid, lift
    and place, turn and halve, put down, tooltips, the load in words.
  - (e) Clothing drawn on the person from what is worn (layers standing off the skin, the
    loincloth, chest band and belt in their shapes, hoods over the hair).
  - (f) Quick slots 1–6 (drawing from the attachment points takes half a second) and the quick
    choice wheel (hold Q); the v2 §10.6 controls.
  - Acceptance: capacity and encumbrance (`hearth_items/tests/items.rs`: containers hold what
    fits, the tie one small thing, loads slow, a log section is only dragged at 0.15–0.4 m/s on
    grass); the UI round trip (`hearth/tests/inventory_ui.rs`: two clicks move the hand axe
    into the basket); in a running world (`hearth/tests/server.rs`) a new person is dressed,
    things are put down and picked up, and a dragged log leaves under half a metre a second.
- [x] **V2-5 — Interaction, process crafting & knowledge** (2026-10-01)
  - (a) Knowledge (`hearth_craft::knowledge`, D72, `docs/design/knowledge-and-processes.md`):
    the technology graph as one person learns it. Insight comes only from triggers the game
    emits: `verb:key` for what was done to what (its form, material and their tags), `do:`,
    `use:`, `see:`, `throw:`, `infer:` once every prerequisite is known, and the systems' own
    (full hands, a wildfire seen, sleeping on a hide, wood afloat). Discoveries and hunches go
    into a journal with their routes; skills grow with diminishing returns (and slip after a
    month idle under Authentic); Knowledge Modes (Discovery, Guided, Open); under Legacy death
    what was known passes on as legends, heard four times as loud, that one attempt brings back.
  - (b) The process engine (`hearth_craft::engine`): a process names its target in the world (a
    block by id, material or suffix, a thing lying, water, a fire, open ground) and its effect
    (keep, remove, excavate, deplete, ignite, feed, bank, mend). Inputs come from what is at hand
    (the target, the hands, what is carried, what lies within reach); bulk materials are counted
    in units of a bulk form (a cut of meat, a handful, a hank, a lump, a hide). Tools are chosen
    by property and wear with use; time follows skill and tool; failures depend on the stone,
    the damp and the wood; quality, by-products and unattended work (drying) follow. Building a
    workstation is a process too.
  - (c) Fire and food (`hearth_craft::fire`, `food`, `docs/design/fire-and-food.md`): fuel burns
    from its surface in, leaving coals; fires can be banked; kindling catches from dying embers
    where a stick will not; rain drowns a fire; lamps burn fat; fires give radiant heat to
    the body. Each material has its nutrition; food spoils with warmth and wetness (drying
    slows it), and spoiled food carries its illness.
  - (d) The workshop (server, `docs/design/gathering.md`): work in hand runs at a warp. Stations
    are blocks that show their fire. Digging leaves spoil piles at their angle of repose;
    harvests are limited per block and year; lightning sets trees alight; kills turn up to
    scavenge. Eating, drinking (salt water is tasted and spat out), filling skins and throwing
    all work. The heat of fires, beds and the lee of trees reaches the body, and the player's
    changes to the terrain are kept over the generated terrain and saved (D73).
  - (e) The client: what can be done is listed by the crosshair (the wheel chooses, LMB does
    it, E does the first), with work shown as it goes. The journal (J) shows what is known by
    era, hunches, skills and notes. Knapping is done by hand: strike where the flake should
    come off, and the stone's quality shows. The eye adapts to firelight.
  - (f) Data: Era 0–2 knowledge is rewritten to emitted triggers (58 nodes: 51 implemented, 7
    planned with their systems). There are 117 processes, bulk, tool and weapon forms, and
    nutrition and keeping times. Nettle, hazel and bramble grow in the temperate woods. The
    campfire (out to blazing, with animated flames and embers), drying rack, fat lamp, grass
    and fur beds and spoil are drawn.
  - Acceptance: the graph (`hearth/tests/knowledge_graph.rs`: with the world's real blocks every
    Era 0–2 node and every process can be reached from nothing, and the mean steps from scratch
    rise by era: 3.8, 8.0, 10.4); the workshop in a running world (`hearth/tests/workshop.rs`,
    with the camp unchanged after going away and after saving); the screens
    (`hearth/tests/craft_ui.rs`); and the bot (`hearth/tests/acceptance_v2_5.rs`, about two
    minutes). From nothing, in a Discovery world (seed 7), the bot learns 29 techniques by their
    routes alone and reaches each goal: fire on day 0.2 (a lightning-struck tree, its fire
    carried home on a burning stick), a hafted stone-tipped spear on day 15, sewn moccasins on
    day 16 and dried meat on day 20. Along the way it learns the hand drill and lights its fire
    again with it, drinks from a river 210 m off as the summer lowers it, and dresses in its
    moccasins and a hide cape for the autumn.
- [x] **V2-6 — Flora framework (temperate first)** (2026-10-01)
  - `hearth_flora`: the 25 temperate tree species of Appendix A's tier 1 with growth forms and
    a pure growth model (Chapman–Richards height, a girth that thickens after the height
    levels off, seven living stages and the snag), a Weber–Penn skeleton voxelized into logs,
    joined branch bars 2–12 px and foliage, templates cached per species, stage and variant
    and turned and mirrored (D75). Forests by climate fit, biome and shade, crowns spaced by
    their size, with an understory of saplings and poles.
  - Foliage is passable (it slows and rustles), thick limbs are solid; felling is a physical
    fall about the stump, then limbing and bucking (`tests/felling.rs`).
  - The understory: 26 plants placed by climate, light under the canopy, ground and patches;
    gathering by season; willow bark, yarrow and plantain as medicine; poisonous look-alikes
    seen by their group's name until told apart; plant poisoning (`tests/understory.rs`).
  - The vegetation state (D76): trees age with the calendar, die and stand as snags and are
    replaced; felled trees leave stumps and are replaced; cleared and burned land grows back
    through the pioneers to the old forest; the server regrows the loaded terrain as it
    changes; `vegetation.json`.
  - Wildfire (D77): a block automaton near the player (fuel, Simard's fuel moisture, the dry
    season's curing, wind, slope), ecological cells far away, smoke plumes seen from afar; the
    burned ground kept exactly and entering succession; lightning sets fires; `set_alight`.
  - Distant terrain (D78): vegetation and the player's changes in the tiles, a disk cache of
    tiles stamped by a build fingerprint, the video-memory budget held by coarsening far tiles,
    TAA (an option; the Fabulous preset), a streaming fly-through benchmark.
  - Acceptance (`docs/design/flora.md`, "Acceptance (V2-6)"): silhouettes of every species at
    three ages (a contact sheet of screenshots); a cleared area through succession over
    simulated years (`tests/succession.rs`: herbs at once, shrubs by the third year, pioneers
    by the tenth, a young wood at forty, the shade-tolerant at two hundred); a dry-season
    wildfire that runs downwind, burns out and stays burned (`tests/wildfire.rs`); the horizon
    from the 420 m peak (screenshot); the fly-through benchmark (369 FPS average, 124 FPS 1 %
    low at 1280×720 on the RTX 4060 Laptop).
  - The V2-5 bot now dries meat in the summer after its first kill, fetching kills for hides
    while it dries: fire on day 0.2, dried meat 15.6, moccasins 16.7, spear 17.3.
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
| Materials | 87 | 117 |
| Rock types | 35 | 0 |
| Minerals | 27 | 3 |
| Geological provinces | 14 | 0 |
| Deposit models | 52 | 0 |
| Soils | 16 | 0 |
| Plant species | 51 | 5 |
| Animal species | 43 | 0 |
| Ecosystems | 3 | 1 |
| Hominin species | 0 | 1 |
| Item forms | 43 | 0 |
| Processes | 250 | 0 |
| Knowledge nodes | 55 | 125 |
| Workstations | 5 | 0 |
| Construction pieces | 0 | 5 |
| Garments | 9 | 0 |
| Injuries | 10 | 0 |
| Illnesses | 7 | 0 |
| Eras | 0 | 8 |

## In progress
V2-7 — Fauna framework (temperate forest first), in parts:
- [x] (a) Realms and content: biogeographic realms per landmass (D80; the trees of a realm
  are its own, stand-ins from one realm where it has none); the fauna schema's life history,
  ranging, coats, yields, tracks, calls and temperament; the Tier 1 temperate set, Palearctic
  and Nearctic (43 species: `fauna/temperate*.ron`), the freshwater ecosystem, the animal
  materials (earthworms, offal, fur pelts, feathers, tusk, horn, raw fish, honey, beeswax).
- [x] (b) Populations (`hearth_fauna`, D81, docs/design/fauna.md "Populations"): habitats of
  256 m cells from the generator and the vegetation (Miami production shared out by canopy,
  regrowth and mast trees, seasons, snow, yearly weather, mast years); groups for large animals
  and numbers per cell for small ones; hunting by calibrated functional responses (type III for
  generalists, prey refuges in cover), scramble competition for forage and carrion, condition,
  deaths by cause, births by season and condition, territories, crowding past capacity,
  dispersal and budding, immigration at the edges; regions of 16 km with each cell's own realm
  of animals. Tests: a uniform wood holds every species for 30 years; heavy hunting of red
  deer depletes and they recover; a generated temperate region feeds its animals. (Saving
  regions and the 50-year run on a generated region come with (c) and (j).)
- [x] (c) Animals in the world (D82, docs/design/fauna.md "Animals in the world"): the server
  keeps the 3 × 3 regions about the player, advances them with the calendar and saves them
  (`fauna.json.zst`); groups within 112 m come into the world as their members and fold back
  beyond 150 m (the dead staying dead), small walking species drawn from the cells within 80 m;
  grazing, wandering, resting by their hours and fleeing within their flight distance; the
  `Animals` and `Census` messages; the client eases and draws them (box bodies until (d)).
  Screenshots: `animal=`, `herd=`, `fauna=true`, `seek=<species>[:n]` (the camera where it
  sees the most of the nth nearest group). Test: walking to the groups about the spawn meets
  animals on the ground that move and are folded away when left behind. Deferred to (j):
  making regions on a worker (≈0.1 s each on the server thread now).
- [x] (d) Bodies (D83, docs/design/fauna.md "Bodies"): skeletons built from data at real
  dimensions for every body plan in the content (four-legged, birds, snakes, fish, frogs,
  insects), their `shape` proportions defaulting by plan (antlers grown and cast by season,
  horns, tusks, humps); gaits by the Froude number with Alexander's stride, every foot set by
  two-bone IK on the ground under it and the body pitched to the slope; head, jaw, ears, tail
  and breath; lying, asleep, grazing, drinking, grooming, alert (hares and squirrels sit up),
  rearing, attacking; birds hop, peck and fly, snakes wind and coil, fish swim, frogs sit and
  hop; coats painted per species and variant (winter, male, young) into one atlas by
  `hearth_texgen::coats`, read by the figure shader (one instanced draw with the players). All
  43 species and the 3 ecosystems they live in are implemented. Screenshot keys: `mow=`, every
  act on `animal=`. Tests: every species at its dimensions, every act poses whole, feet on a
  slope, gaits by speed, antlers by season, the atlas without overlaps. Deferred: posing on the
  GPU (the CPU poses a herd in well under a millisecond; revisit with the ambient flocks of
  (i)); coats from resource packs.
- [x] (e) Navigation (D84, docs/design/fauna.md "Finding the way"): ways searched by A* on the
  loaded blocks a column at a time with each body's step, wading and swimming rules, bounded
  per search and per step, drawn straight where the ground allows; walkers run from a person
  along them and swim where they must; climbers (squirrels, the black bear) run up the nearest
  trunk; birds big enough to see (crows, ravens, owls, capercaillie, turkey) forage, perch on
  crowns and fly over the trees; fish keep to the streams. The client draws swimming, climbing,
  perched and flying bodies. Screenshot keys: `run=` (the animals live on with the camera among
  them), `near=water`, and the `swim`, `climb`, `perch` and `fly` acts. Tests: ways round a
  wall, across a river for swimmers and not for others, fish in the water, flights over a
  crown to a perch; a deer swims a river to get away, a squirrel runs up a tree, a crow flies
  to a tree's crown, a trout keeps to its stream.
- [x] (f) Minds (D85, docs/design/fauna.md "Minds"): sight (field of view, daylight and night
  vision, how plain the person stands, cover along the line of sight), hearing (the noise of the
  player's gait and the ground underfoot), scent carried downwind, startle; suspicion rising and
  fading; watching, running when aware within flight distance, freezing for those that hide;
  the herd running with the first to run; the young following their mothers; thirst and going
  to water to drink; a utility choice of what to do at ease weighted by the species' `habits`
  (data). The server senses the player from their mover and the weather's wind and daylight.
  Tests: downwind against upwind, crouched against running, still at night against by day, the
  herd running together, a calf following, a roe deer freezing then running, a thirsty deer
  drinking. Packs hunting together come with the predators of (g).
- [x] (g) Danger (D86, docs/design/fauna.md "Danger"): animals turn on a person only for a
  cause — defending young or a kill, surprise, cornered, rut, a hunter's hunger in a lean season
  with the person seeming small, a snake stepped near — at the species' aggression by the
  world's Predator Behavior setting less what it has learned to fear; charges that close or stop
  short; blows as each body strikes (mauling, tusks, horns and antlers, forefeet, bites, venom)
  applied to the player's body; counterplay: fire, facing it upright and loud (the Shout key,
  H), backing off, not running from a hunter; every charge and blow told with its cause and
  logged. Hunters hunt their prey in the world (cats ambush, wolves run their prey down as a
  pack), prey sensing them as they sense people; kills lie dead. Bodies accelerate at bounded
  rates. Tests: a bear guarding her cub charges (mostly bluffs; fewer close when faced down;
  seldom in a tranquil world), hungry wolves at night in late winter but not by a fire, a boar
  surprised close, an adder stepped near bites with venom, a lynx ambushes a roe deer, a charge
  ends when the person backs off. Deferred: territorial defense at dens, habituation to people's
  food.
- [x] (h) Hunting, wounds, tracks, carcasses and butchering by species (D87, D88,
  docs/design/fauna.md "Carcasses and butchering", "Hunting and wounds", "Tracks and signs"):
  carcasses generated per species from its data (a grown one's, a male's where the sexes differ
  — the new `dimorphism` — a young one's where worth working; 61 for the temperate set) with
  butchering and hacking at each (yields by mass, fat by the season, antlers in their seasons,
  work by size); the craft engine's seasonal outputs, what is left of a carcass, decay passed on,
  freshest inputs first. The kill stand-in is gone: kills in the world lie with what the hunter
  ate gone; the populations keep remains where large animals die (predation, age, hunger, winter,
  crowding) that come into the world within 90 m and that ravens tell of within 2 km by day.
  Carcasses drawn as the animal dead on a flank. Hunting: throws stray by the `throwing` skill
  and strike the first animal on their flight; thrusts with what is in hand; wounds by part and
  depth (energy and sharpness against the body's width), bleeding to death, clotting flesh
  wounds, lameness, stunning; a wounded defender close by turns on the person; the person told
  when their quarry falls. Signs: prints in soft ground, blood trails, droppings, read with the
  new tracking knowledge. Screenshot keys: the `dead` act, `trail=`. Tests: carcasses by species,
  a red deer by sex, age and season, butchery enabling every species' butchering, seasonal fat
  and antlers, what is left and decay passed on, remains kept and found and told of by ravens, a
  lynx's kill eaten from, a dead deer found and butchered in the world, a spear behind the
  shoulder, a gut-struck deer lying up, a light wound healing, wooden and stone points, a stone
  at a hare, a wounded boar turning, a throw over the back, prints in snow and not grass, a blood
  trail, a hunter spearing an animal that lies where it fell. The V2-5 acceptance forces a roe
  deer's natural death near camp every day or two. Deferred: whole hides by species (hides
  come as sheets of a kilogram; a large animal's is many) and hide-working times by area;
  drying spoiled meat still makes good dried meat; prints spaced by the walking stride at every
  gait.
- [x] (i) Calls, the dawn chorus, ambient birds and insects (D89, docs/design/fauna.md
  "Calls and ambient life"): calls made from each species' data (19 kinds of call synthesized:
  roars, barks, howls, hoots, songs, caws, drumming, rattles…), heard by distance, air and
  direction with captions; animals call for alarm, distress, threat, the rut, contact,
  territory, the dawn and the night at real-time rates, a pack taking up a howl together; the
  populations' songbirds, owls and woodpeckers sing about the player (the dawn chorus), and far
  packs howl and stags roar from their groups; crickets on warm summer nights by Dolbear's law.
  Tests: every call heard and ending, low and high calls; a deer put to flight barks, a struck
  hare screams; a pack howls by night together and not by day; a stag roars in the autumn rut
  and not in spring; the chorus at dawn in spring against noon, night and winter, owls by night.
  Deferred: birds and insects seen (flocks crossing the sky, insects over flowers) and with them
  posing on the GPU.
- [ ] (j) Acceptance and the performance gate.

## Next steps
0. Every milestone ends with `scripts/perf-gate.sh` (≈10 min: builds the baseline commit in
   `perf/baseline` in `bench-out/gate`, three alternating rounds of the quick scenes); a fall
   of more than 5 % in average FPS or 1 % lows is fixed or justified in DECISIONS.md and the
   baseline moved with `--accept` on a committed tree (then commit `perf/baseline`). For
   one-off claims, A/B alternate builds as the gate does (or `--lod-error` / `--render-scale`
   / `--water` within one build); capture golden images with `hearth bench --golden DIR`
   before comparing looks.
1. V2-7 — Fauna framework (temperate forest first, PLAN.md): the engine's v1 M9
   infrastructure (ECS integration, body plans with shared skeletons, procedural animation,
   pathfinding for walking, swimming, flying and climbing, instanced rendering, animal audio);
   species coats, senses with wind-carried scent, utility-AI behaviours, herds and packs,
   ecological cells with population dynamics, materialization and folding, biogeographic
   realms (which also end the Nearctic trees in every world), predators with real attack
   causes, tracking, hunting and butchering of real animals (replacing the kill stand-in),
   Tier 1 temperate fauna, ambient birds and insects.

## Known issues
- In this environment presents never block (FIFO on both Vulkan and DX12 ran at ~1.5–2k FPS
  with terrain), most likely because the window is occluded. Re-check pacing on a visible
  window; the frame limiter covers the vsync-off case.

## Deferred
- Flora (V2-6): trees regrow only on tree sites and one tree stands on a site at a time; a
  fallen trunk does not rot and a felled tree's foliage does not wither; a fire burning at a
  save is out on opening; fire does not spread from a campfire to the grass about it and
  cannot be beaten out; smoke does not blind, choke or dim the sky; climbing is only on solid
  thick limbs and ledges; coppicing, pollarding, seeds and planting wait (V2-12).
- Distant terrain: the player's changes reach it as each column's highest solid block (holes
  dug are not shown); the tile selection runs on the frame thread and is the fly-through's
  slowest frame (5–9 ms when the camera has moved 16 m), the first thing to move off it;
  FXAA (listed in the options since v1) is not built and counts as off.
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
  washing are processes of ore processing (V2-13). Placers do not yet
  trace back along the river network to their source body, and deposit systems have no
  zoning (oxide cap over sulfide ore) — both with V2-13 prospecting.
- Fumarole ground is snowed over like any other (the thermal model of V2-5 covers fires and
  things, not the ground; warm ground comes with V2-16 at the latest); the dead-bush sprite
  reads as a dark wedge from above (V2-6 flora).
- Death under Legacy rules leaves no body where the player fell (belongings are left since
  V2-4, and what the dead knew passes on as legends since V2-5). Hardy comes back at the
  world's first spawn until there are camps (V2-8).
- Containers lying in the world are opened by picking them up; things do not yet get wet in
  the rain or dry out (each keeps a wetness that spoiling and burning read); no rolling logs, travois,
  sledges or rafts yet (V2-8); heavy loads do not yet weigh a swimmer down.
- Sounds are all the player's own or around them: sounds placed in the world (direction,
  distance, occlusion) come with fauna (V2-7), and with them the work of the hands (knapping,
  scraping, digging) and fire's crackle; no music, thunder, flowing water or animal sounds
  yet (their volume sliders are hidden); the echo has one character, not measured from the
  cave's size; captions show no direction.

## Environment notes
- Rust was installed via rustup in `%USERPROFILE%\.cargo`; shells started before the install
  need `export PATH="$HOME/.cargo/bin:$PATH"`.
- Machine: Ryzen 7 7840HS (8C/16T), RTX 4060 Laptop + Radeon 780M, 16 GB RAM.
