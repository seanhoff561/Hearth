# Progress

Direction: **v2** (`docs/spec/v2-direction-change.md`) since 2026-09-30; see `MIGRATION.md`.
Since 2026-10-03 with **V2.1** (`docs/spec/v2.1-realistic-humans.md`, and its Addenda: A, the
player is born; B, births in multiplayer and life after death), which replaces V2-11 with
milestones H0–H13; see `MIGRATION_HUMANS.md` (D162). Amendment R (`dev/AMENDMENT_R.md`) waits
until V2-16 (D166).

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
- [x] V2-7 — Fauna framework (temperate forest first): 43 Tier 1 temperate species
  (Palearctic and Nearctic) living as populations in 256 m cells and 16 km regions, brought into
  the world as animals near the player with bodies, gaits, coats, minds, ways, danger, wounds,
  signs and voices; carcasses and butchering by species; the acceptance
  (`hearth_fauna/tests/acceptance_v2_7.rs`) and the performance gate (lowland forest −4.7 %
  average FPS against the end of V2-6, the other scenes within ±2 %). In parts:
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
    making regions on a worker (done in (j)).
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
  - [x] (j) Acceptance and the performance gate (D90, docs/design/fauna.md "Acceptance"):
    `acceptance_v2_7.rs` — fifty years of the 3 × 3 regions about the spawn stay within bounds for
    every species held ten of or more; heavy hunting of the roe deer there thins them to a sixth
    and they come back; every attack in seven kinds of encounter says its cause, each only where
    it can hold. The populations' fixes it called for: the small species' young die at the first
    year's rate only in their first year; crowding falls on the young and half-grown; prey reckoned
    over their own habitat; a predator on a realm's border hunts as at home. Regions are made on
    worker threads, three at a time. The performance gate run at the end.
- [x] V2-8 — Structural building & shelter: construction pieces × their materials as
  generated blocks put up and taken down by generated processes of real quantities, beside
  the face looked at and only where they rest, with a ghost; every piece a member reckoned
  as columns and runs on every change (collapses spread, debris lies where it fell); ground
  over openings by its material, shored by stout timber; rain through roofs by pitch, rot,
  erosion and thaw in stages; shelter from rays into wind and warmth; the builder's view;
  thatch, pit houses, wattle and daub, mudbrick, snow walls; the acceptance
  (`hearth/tests/acceptance_v2_8.rs`) and the performance gate (all scenes within ±2 %
  against the end of V2-7). In parts:
  - [x] (a) Pieces in the world: each construction piece × each material it may be made of is a
    generated block (`hearth:post/hazel_wood`), shaped by the piece (post, beam, panel, layer,
    roof in eight steps, wall, block), turned by `facing`, textured from the material (bark has
    its own pattern now: birch white with lenticels); `place_<piece>` / `take_down_<piece>`
    generated with the piece's real quantities, time and knowledge (D91); placed beside the face
    looked at (`AimAt::Beside`), facing the builder's way, only where it rests (stages, D92);
    a ghost of edges where it will go; first set in `construction/shelter.ron` (brush, post,
    beam, bark roof and covering, hide wall and roof, dry stone, stone lintel). Tests:
    `hearth/tests/building.rs`; screenshot option `put=state@east:south:up`.
  - [x] (b) Stability: every piece a member (weight, moment, joint continuity, crushing,
    stiffness, reach) from its size and material (`building::member`, coverings on frames);
    `hearth_world::structure` reckons the connected structure whole on every change — columns
    carry down (crush, buckling), runs span between supports (moment at every piece and joint,
    linear time; reach for stiffness), loads top down — deterministically; the server reckons up
    to 8192 pieces a tick (`structure.rs`), breaks what fails (half its makings lie where it
    fell) and reckons on next tick (cascades); the client shows pieces tumbling with dust and
    plays them giving way (`Sound::Break`) and landing (D93). Tests: solver units (lean-to
    stands, overlong ridge breaks, one slab spans a doorway and two do not, dry stone over
    nothing falls, beam theory, 4300 pieces in ~10 ms), content members, and a server collapse
    (a third beam beyond reach falls; without its post the rest falls).
  - [x] (c) Excavation: natural ground roofs an opening as wide as its material allows
    (`building::self_span`: loose earth 0.5 m, firm soil 1.2, clay 1.5, rock 1 + UCS/15 m,
    `span_m` overrides: loess 3, laterite 2.5, frozen ground 3; turf 1 by its sound); the ground
    over the places a change opened is reckoned (`Structures::ground_falls`), and what is over too
    wide an opening falls in, broken (its cobbles or loose earth), to the floor below, chimneying
    up; pieces under ground hold it and bear its weight as deep as the opening is wide (≤ 8
    blocks), so poles buckle and stone or stout timber holds (D94). Tests: clay holds a 1-wide
    tunnel and not 2, granite a hall, loam nothing; posts under clay buckle.
  - [x] (d) Weather on buildings: rain through covers (`building::cover`: a roof as steep as its
    covering needs sheds it, flatter roofs and flat coverings let 0.4 drip); pieces of materials
    that wear away have `decay` 0–3 (members weakened by `DECAYED`), advanced a stage a day by
    chance (`Structures::weather`): wood rots in earth in 2 + 30·d² years (four times slower
    rained on, never dry on stone), earth erodes in the rain (3 years), snow and ice melt in a
    thaw; past stage 3 a piece crumbles (D95). Snow loads on roofs are left for later (snow is
    not laid on pieces yet).
  - [x] (e) Shelter quality: seventeen rays from the eyes give enclosure, sides and heat loss
    (`building::shelter`); the body's wind is cut by the sides, the air warmed by fires within
    3 m by their heat over the hut's losses (≤ 25 °C), rain by the cover (D95). Tests: open
    ground gives nothing; a hide hut with a bark roof stills the wind and a 5 kW fire warms it
    8–25 °C.
  - [x] (f) The builder's view (key V): pieces within 32 m outlined by stress (blue → red), the
    server telling stresses within 48 m each second and reckoning pieces not yet reckoned since
    their land loaded; the ghost reckons the structure it would join on the client
    (`Structures::would_bear`, cached half a second) and shows pale, amber or red. Screenshot
    option `stress=true`.
  - [x] (g) Era 0–3 techniques: thatching, pit house, wattle and daub and mudbrick knowledge
    implemented with discovery routes; pieces reed thatch (on rafters), wattle panel, daub on
    wattle, mudbrick wall, snow wall, log post and log beam (pit houses' frames, props and caps
    under ground); new forms mudbrick and block of snow with `make_mudbrick` (unattended, two
    dry days) and `cut_snow_block`; dried earth pieces textured paler and grainy. The lint now
    infers the materials of what is done to blocks by name (a log of any tree) and orders eras
    by their implemented nodes. Deferred: log walls (timber framing needs the ground-stone axe,
    V2-12).
  - [x] (h) Acceptance (`hearth/tests/acceptance_v2_8.rs`): a slab spans a doorway between
    dry-stone piers built by processes and two slabs end to end fall when their props go; a clay
    chamber three wide falls in unshored or on poles and stands on log sets; reed thatch keeps
    held rain off a body while flat bark lets 0.4 through; 8000 pieces reckon in ~10 ms.
- [x] V2-9 — Vertical slice review: a year in the temperate forest from a loincloth in spring,
  played by a scripted bot (`hearth/tests/slice_year.rs`, `scripts/slice-year.sh`), which lives
  it through in a lean-to of its own making; `docs/review/slice-1.md` (what holds up, the issues
  ranked, the bot's own failings) with a contact sheet of its moments rendered from saves; the
  top issues fixed and the year run again after each round. In parts:
  - [x] (a) Saved worlds in screenshots: `--screenshot save=DIR` renders a world as its player
    left it — its seed and planet, the date and hour of its clock, the player's changes (pieces,
    digging), felled and cleared vegetation, the things lying about, the camera at the player.
  - [x] (b) The year bot: the V2-5 bot's phases (shared in `hearth/tests/bot/`), a hide cape, a
    windbreak and a lean-to over the bed, then day by day (water, a kill fetched, butchered and
    cooked, the fire, firewood, sleep), logging each day and keeping copies of the world at its
    moments (D96).
  - [x] (c) The review: `docs/review/slice-1.md`.
  - [x] (d) Fixed, each measured: carcasses aged in the calendar's days (D97); fat carries
    shivering (D103); a joint roasted on a spit (D104); a tree's crown drips the rain through
    (D105); food eaten where it is carried (D100); the lean-to discoverable; hunger counts food
    only; a bough roof; freshness shown; a too-small fire says so; rotten food goes (D106);
    things lie on their broadest face (D98); brush and wattle looks (D99); pieces joined (D101)
    and roofs drawn as smooth slabs (D102), after the player asked for buildings that fit
    together.
  - [x] (e) The performance gate: averages within 4 %; the 1 % lows that fell were single
    driver stalls in one run of three (an A/B of the forest scene put the new build ahead,
    D107); the baseline moved to the end of V2-9.
- [x] V2-10 — Ecosystem expansion waves
- [x] V2-11 — Australopithecus & the agent framework (parts (a)–(d), 2026-10-03; superseded by
  V2.1, its part (e) folded into H0)
- [x] H0 — Framework migration (2026-10-03)
- [x] H1 — Genetics engine (2026-10-03)
- [x] H2 — Psyche and mind core (2026-10-03)
- [x] H3 — Life course and demography (the player is born) (2026-10-04)
- [x] H4 — Social systems (2026-10-04; tests run, D190)
- [x] H5 — Culture and language (2026-10-04; tests run, D190)
- [x] H6 — Knowledge and social learning (2026-10-04; tests run, D190)
- [x] H7 — Tiers and persistence (2026-10-04; tests run, D190)
- [x] H8 — History simulation and Paleolithic eras (birth options) (2026-10-05)
- [x] H9 — Observer mode and the player in society (2026-10-06)
- [x] H10 — Optional conversation backend (2026-10-07)
- [x] V2-12 — Neolithic (2026-10-08)
- [x] S0 — Baseline and prototype (2026-10-07; D222)
- [x] P0 — Triage and quick fixes (2026-10-08; D224, D225)
- [x] P1 — Menus and world management (2026-10-08; D226)
- [x] P2 — Game modes and Creative (2026-10-08; D229)
- [ ] P3 — Looking, highlighting and the hands
- [ ] P4 — Poses, animation and skipping waits; sleep and time
- [ ] P5 — Motion audit
- [ ] P6 — Learning to play
- [ ] S1 — Fill data and editing core
- [ ] S2 — Smooth terrain rendering
- [ ] S3 — Movement, collision and navigation
- [ ] S4 — Distant terrain
- [ ] S5 — Trees and foliage
- [ ] P7 — Natural generation without the grid; plants that look like plants (with S5)
- [ ] P7G — Grasses and ground cover
- [ ] S6 — Bodies and objects
- [ ] S7 — Water, snow, ice, caves and built-piece polish
- [ ] S8 — Performance and cohesion pass
- [ ] P8 — Playtest pass
- [ ] H11 — Neolithic society
- [ ] V2-13 — Metallurgy & mining
- [ ] H12 — Bronze Age society
- [ ] V2-14 — Late scope: Iron Age & Classical
- [ ] H13 — Iron Age / Classical society
- [ ] V2-15 — World creation & menus
- [ ] V2-16 — Long-run balance, performance & cohesion QA

## Smooth World Status
Amendment S (`docs/spec/amendment-s-smooth-world.md`): each S milestone and its targets (S §12.2)
met or not. Kept with `MIGRATION_SMOOTH.md`, Baseline-S and the latest numbers in
`BENCHMARKS.md`, and `docs/review/smooth-world.md`.

| Milestone | State | Targets (S §12.2) |
|---|---|---|
| S0 — Baseline and prototype | done 2026-10-07 (D222: Surface Nets with sharp features; biplanar shading with height blending) | Baseline-S's CPU half recorded; its GPU half (High and Low at 1440p, `scripts/baseline-s.sh`) needs the owner's PC. The prototype mesher: 3,553 surface cubes a second on one thread (S asks 2,000 on eight cores) |
| S1 — Fill data and editing core | planned | memory and saves ≤ 1.5 × Baseline-S |
| S2 — Smooth terrain rendering | planned | frame time (p50 +10 %, p99 +15 % at High; Low at or better), meshing, VRAM |
| S3 — Movement, collision and navigation | planned | 300 animals and 150 people within budget |
| S4 — Distant terrain | planned | v1 §8.4's LOD targets |
| S5–S7 | planned | the forest and vista scenes' frame times |
| S8 — Performance and cohesion pass | planned | every target met or recorded for the owner |

## Playability Status
Amendment P (`docs/spec/amendment-p-playability.md`): each P milestone and its acceptance, kept
with `dev/PLAYTEST.md` (every reported issue, its cause and its state).

| Milestone | State | Open issues in `dev/PLAYTEST.md` |
|---|---|---|
| P0 — Triage and quick fixes | done 2026-10-08 (plant drag by reach; clouds, waves, rain and stars at real speeds) | #6–#9 and #18 fixed; #1–#5 and #10–#17 open for P1–P7G |
| P1 — Menus and world management | done 2026-10-08 (pages that fit; worlds, trash, Create World, the birthplace, pause) | #1–#3 fixed; #4 the mode with P2 |
| P2 — Game modes and Creative | done 2026-10-08 (three modes enforced by the server; Creative's powers, inventory without people, time and weather, clear view, spectating) | #4, #5, #14 fixed |
| P3 — Looking, highlighting and the hands | planned | — |
| P4 — Poses, skipping waits, sleep and time | planned | — |
| P5 — Motion audit | planned | — |
| P6 — Learning to play | planned | — |
| P7, P7G — Natural placement; grasses | planned (with S5) | — |
| P8 — Playtest pass | planned (after S8) | — |

## Earth-True and Quality Status
Amendments E and Q (`docs/spec/amendments-e-q.md`, D227, D228): the order of work is `PLAN.md`'s.

| Row | State |
|---|---|
| Earth-True | E0–E7 planned; the human systems (H0–H10) to be removed in E0 and archived |
| Quality | no audit yet; Audit 0 after E0; open high-priority findings: none recorded yet |

## Humans Status
What of V2.1's people is implemented (used by the simulation) and what is planned (data or
design only). Updated with each H milestone.

| Part | Implemented | Planned |
|---|---|---|
| Species profiles | *Australopithecus*; *H. sapiens* as Wild Earth's few wandering families (H3) and the Middle and Upper Paleolithic's peoples; *Homo erectus* and *H. neanderthalensis*: bodies, life tables, cultures, a proto-language and Neanderthal languages, their days (H8) | — |
| Era profiles | Wild Earth (its *Australopithecus* bands, its wandering families as many as the players expected, H3/H8); Lower, Middle and Upper Paleolithic — their peoples, deep past and recent past, camps, seasonal rounds and gatherings, and the era selector (H8) | Neolithic (H11), Bronze Age (H12), Iron Age (H13); the ice-age world drawn (D195) |
| Calibrated traits | 29 heritable traits on ~460 loci: appearance, health, metabolism, HEXACO temperament and its narrower dimensions, aptitudes (H1); temperament read by the psyche (H2) | health by the life course (H3), aptitudes by learning (H6) |
| Routines | *Australopithecus*'s day, a forager's day (H2); *H. erectus*'s and the Neanderthals' days (H8) | cultures' own routines |
| Life tables | foragers': deaths by age, fertility, nursing, pairing, crowding, bands splitting (H3); *H. erectus*'s and the Neanderthals' (H8, D198) | the eras' (Neolithic H11, later H12–H13) |
| Norms | the foragers': another's things are theirs, food is shared with the hungry, with their sanctions; their ways with strangers and quarrels (H4) | cultures' own (H5) |
| Speech acts | every exchange a structured act with its words and a gesture, heard and made out by the player (H5); *erectus*'s proto-language of nouns, kin, pronouns and particles (H8); the optional conversation backend — a local or a provider's model phrasing what is said to the player within a closed vocabulary of what the speaker may say, and reading the player's typed words as a wheel's act (H10) | the AI bridge's deliberation, voices and agent protocol (Amendment R, V2-16+) |
| The player's birth | genome from two parents of the place, shown at birth; no appearance chosen (H1); born into a family of the place, its household shown, the childhood lived through its moments (H3); two to four households of an era's peoples offered, an *erectus* or a Neanderthal body and childhood where born among them (H8) | births in multiplayer (R3) |
| The player among people | what the player knows of a person (its name once learned, kinship, a rough age, how they take it, the ledger, what it has heard); the talk wheel of speech acts and gestures; being taught and teaching; staying with a band; courting and pairing, children (H9) | trade, invitations to a plan (V2.1 §16) |
| The Observer | watching alive or dead; following and lives; time from stopped to a century a second within its budget; the chronicle; the globe's overlays (H9) | in multiplayer: spectators without time controls (R3) |
| Life after death | the death an event of the world, the life told, living on as a grown kinsman with the "Who you are" briefing, watching with a free camera, beginning the world again (H3); living on as anyone of the world lived in full or as a household, a child's childhood taken up at its age, by the death screen's filters and the world's scope; born again as a baby into a household offered about where one died or anywhere (H8) ; knowledge after death, being born again a setting, the presets Authentic, Legacy, Hardy and Permadeath; watching as the Observer, following and reading lives (H9) | in multiplayer (R3) |

## Content Status
Generated by `hearth content status` (Implemented = used by a game system; Planned = data only).

| Domain | Implemented | Planned (data only) |
|---|---|---|
| Materials | 87 | 256 |
| Rock types | 35 | 0 |
| Minerals | 27 | 3 |
| Geological provinces | 14 | 0 |
| Deposit models | 52 | 0 |
| Soils | 16 | 0 |
| Plant species | 219 | 3 |
| Animal species | 362 | 0 |
| Ecosystems | 20 | 0 |
| Species of person | 4 | 0 |
| Heritable traits | 29 | 0 |
| Named loci | 10 | 0 |
| Gene pools | 4 | 0 |
| Behaviour tendencies | 13 | 0 |
| Feelings | 10 | 0 |
| Values | 8 | 0 |
| Routines | 4 | 0 |
| Life tables | 3 | 0 |
| Moments of childhood | 27 | 0 |
| Norms | 5 | 0 |
| Ways with strangers and quarrels | 3 | 0 |
| Culture generators | 3 | 0 |
| Language generators | 3 | 0 |
| Meanings | 0 | 103 |
| Ways of passing on knowledge | 4 | 0 |
| Item forms | 47 | 0 |
| Processes | 1485 | 0 |
| Knowledge nodes | 61 | 119 |
| Workstations | 5 | 0 |
| Construction pieces | 17 | 0 |
| Garments | 9 | 0 |
| Injuries | 10 | 0 |
| Illnesses | 7 | 0 |
| Eras | 4 | 6 |

## In progress
V2-10 — Ecosystem expansion waves, in parts (PLAN.md; each wave: its flora and fauna as data,
the ecosystems they make, what the systems lacked for them, a fifty-year stability run on
generated land of each of its biomes, and a screenshot suite):
- [x] (a) Boreal, tundra and polar: the boreal forest's own trees (larches, firs, spruces, pines,
  birches, aspen; Palearctic and Nearctic), shrubs, berries, feather moss, sphagnum and reindeer
  lichen; the tundra's dwarf shrubs, sedges and cottongrass, cushions and rosettes; 23 animals
  (moose, reindeer, wolverine, Canada lynx, snowshoe and mountain hares, the cone squirrel,
  sable, marten, ermine, willow ptarmigan, spruce grouse, great grey owl, the two jays; musk ox,
  arctic fox, two lemmings, arctic hare, rock ptarmigan, snowy owl, snow bunting) and the wolf,
  red fox, brown bear, lynx and black bear taught the north's prey; the tundra ecosystem and the
  boreal one made over. What the systems lacked (D108–D111): cone crops as mast and dwarf
  shrubs as browse; a hunter judged by its prey, ranging the wider where its prey are fewer;
  cold-blooded animals needing warm months, and a hunter's appetite set by its hungriest
  season (which also lifted the timber rattlesnake about the spawn from a tenth of what the
  land holds to nine tenths); migrants away while their land is frozen, and neither they nor
  hibernators met about; a lone herd not crowded by itself; reindeer cows' antlers, moose
  palms, musk-ox horns; the understory of a realm without its own plants drawn from the
  stand-in realm in full (D115); small plants drawn at their height; `--screenshot biome=` and
  `season=autumn@0.2`; meat on a drying rack goes off the slower the drier it gets (D114).
  Fifty-year runs of the boreal forest, snowy taiga, tundra and polar desert of seed 7 on a
  huge planet (`hearth_fauna/tests/cold_biomes.rs`, sharing `tests/biomes/` with the waves to
  come; the standard planet has only mountain patches of them, D112); screenshots
  `tools/shots/v210_cold.shots`. The shared bot dresses before the cold and finds resin on any
  resinous tree, the V2-5 world having shifted with the new content (D113). The polar bear
  comes with the oceans' wave (f), with its seals and the ice.
- [x] (b) Grassland, steppe and desert: the steppe's and the prairie's grasses and flowers
  (feather grass, steppe fescue, big bluestem, blue grama, fringed sage, pasque flower,
  coneflower, sunflower, yucca, flax) and the deserts' shrubs and succulents (creosote, prickly
  pear in a cactus sprite of its own, mesquite, saxaul, salt cedar, camelthorn, sagebrush,
  saltbush, joint pine), grown on the steppe and in the deserts and thinning with dryness, with
  their foods, woods and medicines; 39 animals (American and European bison, wild horse, saiga,
  pronghorn, prairie dog, bobak marmot, souslik, the common and meadow voles, coyote, corsac and
  swift foxes, American badger, steppe polecat, black-footed ferret, great bustard, sage-grouse,
  burrowing owl, golden eagle, meadowlark, skylark, prairie rattlesnake; Bactrian camel,
  dromedary, onager, addax, dorcas gazelle, jackrabbit, kangaroo rat, jerboa, fennec, kit fox,
  sand cat, roadrunner, western diamondback, horned viper, Gila monster, desert tortoise) and the
  wolf, red fox, hares, deer mouse, ermine and brown bear taught the open lands; the ecosystems
  of steppe and prairie, cold desert and hot desert. What the systems lacked (D116–D123): each
  ecosystem's reference land, by which its animals' densities are counted, and forage anchored
  by every reference land's animals; desert shrubs as browse; young animals settling where the
  land is as good as it gets; cold-blooded hunters and an owl with its voles as specialists, which
  take their fill of the young; cacti only in the New World, bare ground between a desert's
  plants, the mangroves kept. Fifty-year runs of the steppe, cold desert, hot desert and dune sea
  of seed 7 on a vast planet, where the cold lands' runs moved too, and of a uniform prairie and
  Nearctic hot desert (`hearth_fauna/tests/dry_biomes.rs`); screenshots
  `tools/shots/v210_dry.shots` (the prairie on seed 4, the Nearctic desert on seed 9). Grass gives
  way to a building; the bots lay their camp on bare ground and knap again when a point fails
  (D123).
- [x] (c) Savanna, tropical forest and Mediterranean scrub: the savanna's trees (umbrella
  thorn, baobab, marula, mopane, teak), grasses (red oat, guinea, elephant grass, wild sorghum)
  and shrubs and herbs (sickle bush, aloe, devil's claw); the rainforest's (African mahogany,
  kapok, strangler fig, oil palm, Brazil nut, rubber tree, cacao, açaí, meranti, durian, giant
  bamboo, wild banana, tree fern) and its floor (heliconia, philodendron, ginger, giant taro,
  arrowroots); the Mediterranean's evergreen oaks, wild olive, Aleppo pine, carob, strawberry
  tree and coast live oak over rosemary, lavender, rock rose, myrtle, thyme and the chaparral's
  chamise, manzanita, toyon and sage; the oasis's date palms — with their woods, foods, fibres,
  latex, cork and medicines. 92 animals of three realms' savanna and rainforest and of the
  scrub (docs/design/fauna.md) and the boar, red deer, fox, badger, wolf, golden eagle, coyote,
  bobcat, deer mouse and jackrabbit living in the scrub; the ecosystems of the savanna, the
  rainforest and the Mediterranean scrub. What the systems lacked (D124–D133): dry seasons; palms;
  elephants (trunk, fan ears, tusks on both sexes), giraffes' ossicones and rhinos' nasal horns,
  hoof and pad prints, primates' and lizards' own frames; reference lands of several realms;
  lands a species lives in without its density being measured there (`also_in`); the new
  animals' food and survival on the older ones' terms, savanna hunters as many as the herds
  feed; numerous omnivores' meat as carrion. Fifty-year runs of the savanna of Africa, India and
  South America, of the African and American rainforests and of the Mediterranean scrub about a
  realm's own heart on seed 7's vast planet, and of uniform Asian rainforest and chaparral
  (`hearth_fauna/tests/tropical_biomes.rs`); the runs name each species' killers. Screenshots
  `tools/shots/v210_tropical.shots`. The V2-5 bot takes its firebrand from the lowest flames of
  the tree lightning set burning, a tropical tree by its camp now burning from the crown; a felled
  trunk flattens the herbs and low shrubs it lands on (D132).
- [x] (d) Mountains and alpine: the montane forests' and the tree line's trees of the Alps and
  Siberia (European larch, Swiss stone pine, mountain pine, silver fir), the Rockies (Engelmann
  spruce, subalpine fir, lodgepole and whitebark pines, mountain hemlock), the Himalaya
  (Himalayan birch, deodar), the Andes (lenga, coihue, monkey puzzle, polylepis, frailejón) and
  East Africa (pencil cedar, yellowwood, kosso, giant groundsel); the alpine's shrubs, grasses,
  cushions and flowers (alpenrose, dwarf juniper, green alder, alpine fescue, moss campion,
  glacier buttercup, edelweiss, trumpet gentian, alpine aster, Himalayan blue poppy) and the
  tropical mountains' own (the Afroalpine's giant lobelia, tree heath, everlasting, lady's mantle
  and tussock fescue; the páramo's and the puna's ichu, páramo grass, lupine, chuquiragua,
  werneria and yareta), with their nuts, berries, woods, wool, fuel and medicines. 45 animals of
  the Old World's mountains (ibex, chamois, marmot, snow vole, bearded vulture, chough,
  nutcracker, snow leopard, bharal, tahr, argali, wild yak, chiru, kiang, plateau pika, snowcock,
  Tibetan fox, musk deer, monal), the Americas' (mountain goat, bighorn, hoary marmot, pika,
  golden-mantled ground squirrel, white-tailed ptarmigan, Clark's nutcracker, cougar; vicuña,
  guanaco, taruca, mountain tapir, pudú, mountain viscacha, leaf-eared mouse, culpeo,
  spectacled bear, Andean condor) and Ethiopia's (gelada, Ethiopian wolf, giant mole rat, walia
  ibex, mountain nyala, klipspringer, rock hyrax, Verreaux's eagle), the wild yak, the vicuña and
  the guanaco to become the herders' yak, alpaca and llama; and the golden eagle, fox, wolf,
  bears, wolverine, ermine, hares, ptarmigan, deer, lynxes, capercaillie, squirrels, deer mouse,
  raven, moose and leopard living in the mountains; the montane forest and alpine ecosystems.
  What the systems lacked (D134–D142): the alpine above a mountain's trees though its climate
  is a tundra's, and krummholz along its tree line (the alpine meadows had been a fringe of cool
  coasts); the lowland plants bounded by the summer's warmth; a realm's own plants judged over
  the place against the stand-ins'; the tree line's dwarfs kept from the forests' canopies; a
  pack's range its share of the land; horns that sweep up and back, curl and spiral, and a
  gazelle's flank band. Fifty-year runs of the Old World's montane forest about its heart on seed
  7's vast planet, and of uniform alpine land of the Old World, the Rockies, the Andes and
  Ethiopia (in its year-round cold) and montane forest of the Rockies and the Andes
  (`hearth_fauna/tests/mountain_biomes.rs`); the polar desert's run is the tundra's land in the
  high arctic's climate, its cold plateaus being alpine now. Screenshots
  `tools/shots/v210_mountain.shots` (the huge planet's Nearctic has no mountains: the Rockies'
  animals stand on the Old World's meadow). The felling test cuts a slim stem standing on the soil
  itself, the nearest having stood on a holm oak's flared foot, and the succession acceptance's
  wood, now a mountain forest of yellowwood and pencil cedar, grows back through the kosso
  (D142).
- [x] (e) Wetlands, rivers and lakes: wetlands where the land holds the water (marshes on the
  rivers' flat banks, bogs and fens on cool, wet lowlands too flat to drain, swamps on the
  tropics' rain-soaked lowlands), with pools in their hollows and sphagnum on the bogs, some 2 %
  of the land where they had been a twentieth of a percent; plants in the water, standing up out
  of the shallows on stalks under the surface (the common reed of every realm, cattail,
  papyrus, wild rice, arrowhead, yellow flag, watercress) or with their leaves afloat (white,
  fragrant and blue water lilies, the sacred lotus, the giant water lily), marsh marigold and
  bog cranberry on the wet ground, and the swamps' trees (bald cypress, raffia, moriche and sago
  palms), with their foods (cattail root, wild rice, arrowhead tubers, watercress, cranberries,
  lotus seeds and root, sago washed from the pith), papyrus, raffia, cattail down and cypress
  wood. 48 animals of the waters: the north's (beavers, muskrat, otters, mink, water vole,
  mallard, greylag and Canada geese, mute swan, grey and great blue herons, white stork, common
  crane, osprey, kingfisher, pike, carp, wels, eel, channel catfish, snapping and pond turtles,
  bullfrog, toad), Africa's (hippopotamus, Nile crocodile, sitatunga, fish eagle, shoebill,
  tilapia, sharptooth catfish), South America's (capybara, spectacled caiman, green anaconda,
  giant otter, marsh deer, wattled jacana, piranha, arapaima) and tropical Asia's (wild water
  buffalo, to be the paddy's buffalo; mugger, gharial, smooth-coated otter, fishing cat, sarus
  crane, snakehead), and the hunters of the land about them taught their prey; the temperate and
  the tropical waters' ecosystems. What the systems lacked (D143–D150): the wetlands; plants in
  the water; reference lands with water; the warm lands' animals bound by the coldest month
  they bear; the waterside's animals on the land along the water; a realm's run judging its
  own animals. Fifty-year runs of the Old World's wetland about its heart on seed 7's vast
  planet, and of uniform freshwater land of the Nearctic, Africa, South America and tropical Asia
  (`hearth_fauna/tests/wetland_biomes.rs`); screenshots `tools/shots/v210_wetlands.shots`.
  The hunting test follows its quarry down into a pool's hollow (D150). Mangroves come with the
  coasts (f).
- [x] (f) Oceans: coasts, reefs, kelp, the open ocean, the deep sea's light. The sea's life
  counted per km² of sea: its small life (plankton, krill, shellfish, weed) growing richest on
  the cold shelves and a third as rich over the deep, poorer under the ice, and the sea's
  animals living on it and on each other. 68 animals: the cold northern shelves' (herring, sand
  eel, cod, red king crab, harbour and grey seals, Steller sea lion, sea otter, harbour porpoise,
  puffin, gannet, herring gull, cormorant), the Arctic's (Arctic cod, ringed seal, walrus, polar
  bear, narwhal, beluga), the temperate and southern shelves' (sardine, chub mackerel,
  California and South American sea lions, Cape fur seal, bottlenose and common dolphins,
  loggerhead turtle, brown pelican, Magellanic, African and little penguins), the Antarctic's
  (silverfish, toothfish, emperor and Adélie penguins, crabeater, Weddell and leopard seals),
  the reefs' (bluestripe and yellowtail snappers, bumphead and stoplight parrotfish, giant and
  Nassau groupers, blacktip and Caribbean reef sharks, giant moray, green and hawksbill turtles,
  dugong, manatee), the mangroves' (fiddler crab, proboscis monkey, scarlet ibis, saltwater
  crocodile) and the open ocean's (lanternfish, flying fish, yellowfin and bluefin tunas, blue,
  great white and whale sharks, humpback, blue and sperm whales, orca, wandering albatross,
  anglerfish), with blubber, baleen, tusk and tortoiseshell to take from them. The coasts'
  plants: the sea's kelps, wrack and seagrasses (drawn by the generator's), red, loop-root and
  grey mangroves standing in the tide on their stilt roots, the coconut palm on the tropical
  shore with its nuts, coir, fronds and toddy, and the salt marshes', dunes' and sea cliffs' own
  (cordgrass, glasswort for samphire, sea lavender, marram grass, sea rocket, beach morning
  glory, thrift); glowing sea pens on the deep ocean's floor. The sea's ecosystems: the cold and
  the temperate shelves, the reefs, the open ocean, the polar seas, the mangroves and the coasts.
  What the systems lacked (D151–D159): the sea's populations; bodies for the sea (flippered seals
  and sea turtles, upright penguins, round whales with flukes beating up and down and their back
  fins and flippers, tusks, crabs on their legs with their claws); swimmers at their depths and
  pods in the water; the polar seas one about each pole; stilt roots and the tidal mud the
  mangroves' alone; beach trees; the coast's salt ground; the deep's light. Fifty-year runs of the
  North Atlantic's shelf, the Benguela's, an Indo-Pacific reef, the open ocean and the Arctic and
  Antarctic seas about their hearts on seed 7's vast planet, and of uniform seas of the North
  Pacific, the California current and the Caribbean's reefs and of the Old World's and the
  Americas' mangroves (`hearth_fauna/tests/sea_biomes.rs`); screenshots
  `tools/shots/v210_oceans.shots`.
- [x] (g) The performance gate: every scene within 2 % of V2-9's (the forest's average −0.2 %
  and lows −1.4 %, the peak's −1.2 % and −1.1 %, the cave's −1.9 % and +1.7 %); the baseline
  moved to the end of V2-10 (D160).

V2-11 — *Australopithecus* & the agent framework, in parts (PLAN.md, v2 §8); superseded by
V2.1 after part (d) (D162, `MIGRATION_HUMANS.md`):
- [x] (a) The population: *Australopithecus* as a population of the ecological cells (its diet,
  life and range as data, the hunters that take it), in the savanna–woodland and along the
  tropical waters of every realm (or Africa alone, by the Hominin range setting); fifty-year runs
  with its groups in the savannas of Africa, India and South America (D161).
- [x] (b) The agent framework (`hearth_agent`): kinds of agent from the hominin entries, with the
  player's body physiology at their size and in a coat of hair, knowledge, carrying and a utility
  mind over needs, fear and what is about; groups with their places, ties and techniques; work
  through the player's process engine, knowledge first (`tests/agents.rs`).
- [x] (c) Hominins in the world: groups drawn out as agents near the player and folded back into
  their numbers; their days — feeding at the fruit and nut trees in season and over the open
  ground, drinking, carrying marula stones to the anvil and cracking them, knapping — up to a
  crown's limbs at dusk to bend leaf nests (`leaf_nest`), the alarm at a hunter, mobbing it with
  enough grown ones near or fleeing up the trees, wariness of a person; drawn as small,
  long-armed, hairy bipeds (`Figure::hominin`); screenshots `tools/shots/v211_hominins.shots`
  (`hominin=` places one, `seek=australopithecus` draws out a real group).
- [x] (d) Traces, habituation and learning by watching: a group's site laid under a nut tree the
  first time it is drawn out (an anvil, hammers and cobbles of the place's rock, the flakes and a
  core of earlier work); a calm person comes to be let be within sight over a day or two of
  company (the tolerance kept with the group's numbers) and running at them undoes it; crouched
  or crawling, a person is noticed less far off; watching them work within 40 m is heard as the
  `watch:hominin_*` triggers of the knowledge the process rests on, a scatter of flakes and cores
  as `study:tool_scatter`; a scripted observer gains insight toward knapping
  (`tests/days.rs`: `a_watcher_learns_from_their_knapping`).
- (e) Folded into H0 (docs and the performance gate).

H0 — Framework migration (PLAN.md, V2.1 §20; `MIGRATION_HUMANS.md`):
- [x] The `hearth_people` crate (V2-11's `hearth_agent`, refactored): a **Person** is a record of
  components — life history (sex, day of birth, mother and father, birthplace, events, death),
  body, mind, knowledge, band, possessions, place, its own random stream (D165); ages from the
  calendar and continuous growth; the **species profiles** (`humans/species/`: *Australopithecus*
  implemented, *Homo erectus*, Neanderthals and *H. sapiens* as data) with body ranges by sex,
  cognition, life history and social defaults, replacing `hominins/`.
- [x] A step in two phases — deciding in parallel from the step's start, acting in the persons'
  order — so neither order nor thread count changes the people (D169; `tests/determinism.rs`).
- [x] The registry kept and saved (`people.json.zst`, a format version with migration steps); a
  band met again drawn out as the same persons, reconciled with what the cells did with its
  numbers (D170; `tests/persist.rs`).
- [x] Multiplayer-ready (Amendment R's rule, D166): the people take every player about them, each
  by identity — bands drawn out near any and folded when far from all, each player feared and
  tolerated in their own right; the client sent the people about its own player; new messages
  (`People`, `Inspected`, `Inspect`) serializable under `hearth_protocol::PROTOCOL`.
- [x] The developer's inspector (F3): look at a person within 40 m to see its record by component
  (`inspect.rs`; `tests/inspect.rs`).
- [x] *Australopithecus* on the new framework: V2-11's tests ported (`tests/persons.rs`,
  `tests/days.rs`), all passing; screenshots `tools/shots/v211_hominins.shots`.

H1 — Genetics engine (PLAN.md, V2.1 §4, Addendum A; `docs/design/humans/genetics.md`):
- [x] The architecture as data (`humans/genetics/`): 24 chromosomes with their genetic lengths,
  named loci of large effect (three skin loci, eye colour's near-recessive blue and its green
  modifier, red hair, lactase persistence, altitude, cold, starch digestion), 29 traits with their polygenic loci laid out
  under one seed, thirty recessive conditions, an HLA-like region; about 460 loci (D171).
- [x] Meiosis with crossovers by each parent's sex map, mutation, a child's sex as wished or by
  chance; phenotypes on one scale with chance's spread calibrated to each trait's heritability
  (`tests/genetics.rs`: Mendel's ratios, Haldane's map, heritabilities from offspring on
  midparent and from siblings, inbreeding's recessive conditions, the mutation rate).
- [x] Kinship and inbreeding on the pedigree (`lineage.rs`); a band's people endowed from its pool
  at the place's sunlight, its children the meiosis of their mother's and a father's not of her
  close kin; recessive conditions make one frailer; genomes saved (people format 2).
- [x] Ground rule 1: temperament (HEXACO, D172) and aptitude share one set of frequencies in every
  pool; the lint fails a pool or a sunlight gradient that names a behavioural locus or trait,
  and a test finds them alike under every sun.
- [x] Looks from phenotypes (`looks.rs`): skin, undertone, hair colour (lighter in a child,
  greying), curl, eyes, beard, build and height drive every person's figure; the inspector shows
  the genome and the phenotype's chain.
- [x] The player born (Addendum A, D173): a new world asks a name, daughter, son or chance, and a
  loincloth (`birth.json`); the parents are drawn from the pool of the place and the player is
  their child, shown on the birth screen with them; born again under Legacy; older saves draw
  their birth on loading. The character creator is gone.
- [x] Families of three generations (`tools/shots/h1_families.shots`).

H2 — Psyche and mind core (PLAN.md, V2.1 §5–6), in parts:
- [x] (a) The psyche (`docs/design/humans/psyche.md`, D174): thirteen behaviour tendencies as data
  curves on temperament, ten feelings appraised from events (threats and alarms, hurts, a kin's
  death by kinship, food, work done, company), fading on their own clocks, caught from those near
  at most half as strongly as seen, shown on the body; mood, stress and values; tendencies tilting
  flight distance, persistence, grooming, work and straying, courage the mobbing of a hunter;
  the inspector's Psyche section; people format 3 (`tests/psyche.rs`).
- [x] (b) Perception and memory (`docs/design/humans/mind.md`): sight by daylight (a quarter at
  night) and hearing (alarms carry 300 m), the others seen or remembered where last seen; each
  person's own mental map started from the band's and grown by its days and by watching the
  others (water, sleeping trees, food, anvils), places found wrong let go, danger believed where
  a hunter was seen and its water and food passed over for days; people known by sight, more
  familiar with time together; weighted episodes, the defining ones into the life history
  (`Met`, `Hurt`, `Mourned`); people format 4 (`tests/memory.rs`).
- [x] (c) The layered mind (`docs/design/humans/mind.md`, D175–D176): danger's reflexes, routines
  as data (`humans/mind/routines.ron`: sleeping hours as night, foraging, midday rest and
  grooming, work, evening company), utility selection with the routine's pulls and food weighed
  by its richness, and planning: a task network over the player's process engine to the species'
  depth (have it, pick it up, or make or gather it by a known process, tools held aside), steps
  done one by one and the plan made again when the world moves on, budgeted; tools taken from the
  basket into a hand; what is done to a target told to the world.
- [x] (d) The acceptance: a woman of *Homo sapiens* who knows how makes a stone-tipped spear from
  scratch in about a third of a day (`tests/plans.rs`); the sociable groom more, modestly
  (`tests/temperament.rs`); feelings caught from those near (`tests/psyche.rs`) and shown on the
  figures' postures (`tools/shots/h2_feelings.shots`).
- (e) Checked lightly at the user's asking (their machine froze under full checks): the changed
  crates' tests and lints (`hearth_content`, `hearth_people`, `hearth`); the performance gate and
  the feelings screenshots wait for a release build (H2 changes nothing drawn but the figures'
  postures).

H3 — Life course and demography (PLAN.md, V2.1 §7, §14.3; Addenda A and B;
`docs/design/humans/life.md`), in parts:
- [x] (a) The demography engine (D177): life tables as data (`humans/life/tables.ron`: the
  foragers' Siler mortality, fecundability by age, nursing, twins, deaths in childbirth, pairing
  ages, the land's density and crowding, the split size); every band lived in full lives its
  course a fifty-second of a year at a time — deaths by age, pairing with the nearest in age not
  of close kin (from neighbouring bands, the dispersing sex moving), conception and birth (the
  child's genome its parents' meiosis), bands splitting by households; partners, pregnancies and
  children in the inspector; people format 5. A 200-year run of six forager bands meets §14.3's
  targets (`tests/demography.rs`: e0 30.5, 5.1 children 4.1 years apart, near-zero growth once
  the land fills, bands of about thirty).
- [x] (b) Life stages and child bodies (D178): stages and growth curves as species data (ours
  and *Australopithecus*'s); bodies sized by age (metabolism, skin, gaits, blood, stomach); infants
  carried on the hip and nursed; nutrition holding stature back, attachment in infancy weighing on
  stress; children playing near their mothers and going to watch the grown at work, practising
  and taking in what it shows; child figures with their own proportions, made again as they grow;
  the inspector's stage and growth (`tests/growing.rs`, `tests/childhood.rs`).
- [x] (c) Death, mourning and inheritance (the dead lie a day; what they carried to their heir);
  Wild Earth's families (D179: our species a sparse population of the cells, its ways by
  climate); the player born into one (D180: the household drawn at birth and shown, the family
  set down as a band with the player's own person record); the childhood (D181: moments at the
  world's pace, the years between as a time-lapse, N / Ctrl+N to go on, the child safe, carried,
  kept and sized to its age, learning the family's ways) (`tests/death.rs`, `tests/family.rs`,
  `tests/childhood.rs` in the game crate).
- [x] (d) Death's choices (Addendum B, D182): the death an event of the world (the player's person
  dies with them, mourned), the death screen with the life told and the grown kin and people near
  to live on as (by who they are, never their looks), taking one up whole with the "Who you are"
  briefing, watching the world with a free camera, beginning the world again with the old save
  archived (`tests/afterlife.rs`).
- [x] (e) The acceptance: a 200-year forager run meets §14.3 and its families run to three
  generations and more (`tests/demography.rs`); children play and learn by watching
  (`tests/childhood.rs`); a scripted player is born, grows up through its moments and comes of age
  knowing its family's ways (game `tests/childhood.rs`); a scripted player dies, reads its life
  told and goes on as a grown kinsman (game `tests/afterlife.rs`); the family drawn
  (`tools/shots/h3_family.shots`). Checked lightly at the user's asking: the changed crates' tests
  and lints, a few of the game's bot tests; the performance gate waits (H3 draws nothing new
  in the benchmark scenes).

H4 — Social systems (PLAN.md, V2.1 §8; `docs/design/humans/social.md`), in parts:
- [x] (a) Kinship and households: who another is to a person read from the pedigree and the bonds
  (parents and children, brothers and sisters and half ones, grandparents, aunts and uncles,
  nieces and nephews, cousins, partners and in-laws; `kin.rs`); households, the hearths that
  share food — a pair and the children they raise, the unpaired grown with their mother — settled
  for every band, a pair's own when it bonds, a child born into its mother's, the young left
  without a grown one taken in by their nearest kin, a band splitting by them; the inspector's kin
  and household (`tests/social.rs`).
- [x] (b) Relationships and obligations (`ties.rs`): each person's ties to those they know —
  affection, trust, respect, fear, rivalry and the ledger of what was given and is owed — begun
  from kinship (a partner, parent or child close, a cousin less, band-mates a little), drawn closer
  by time near and much more by grooming, fading back without contact, the dead let go; band-mates
  all know one another; at most a hundred and fifty, the weakest let go; the inspector's closest
  ties (`tests/social.rs`).
- [x] (c) Cooperation: food carried brought to one hungry near — its household's first, then kin
  and those it is fond of, the generous the more readily, never while hungry itself — the gift
  eaten and both ledgers moved; one of a person's own hurt is stayed by (a child by its mother
  too), their fear eased and the tie warmed (`tests/social.rs`).
- [x] (d) Reputation, gossip, norms and sanctions (`repute.rs`, D183): what people believe of one
  another — how generous, how honest, how sure — from deeds seen by daylight (taking, keeping food
  from the hungry, sharing, tending) and from gossip in close company (trusted tellers believed
  more, the heard less sure, a little changed in the passing), fading monthly; norms as data
  (`humans/social/norms.ron`: another's things are theirs, food is shared with the hungry); a
  breach angers its victim and stirs indignation, and as its doer's name worsens the norm's
  sanctions follow — mockery that shames, keeping away, food withheld, casting out of the band;
  grooming and mockery go to their partner; property: what a player puts down stays theirs and
  the people leave it be; the inspector's views (`tests/social.rs`, game `tests/family.rs`).
- [x] (e) Status and group decisions (`council.rs`): standing as the respect a band's grown hold
  one in, earned by work done well before them and by sharing; camps kept (slept at, returned
  to); each evening a band whose camp has gone poor holds council — each grown one reckons the
  places remembered by food, water, danger, the way and staying, the places' cases are argued,
  and they come round toward the case made best, the many and the respected until three in five
  agree — and moves its camp (`tests/social.rs`: seven argue east and north and, the east's case
  made, all go east in four rounds).
- [x] (f) Conflict and strangers (`conflict.rs`, `strangers.rs`, `humans/social/ways.ron`, D184): a
  wrong leaves rivalry; a grievance near, angry enough for one's temper, is had out — argument,
  threats, rarely a short scuffle with bruises (the grown only; never blows with a player) — and
  most ease first: one backs down and keeps away, one of standing or kin to both talks them round,
  the one in the wrong makes amends with food, or the words are spent; a feud after three quarrels
  ends with the weaker household leaving for a country of its own. Strangers (those not trusted, of
  no band that hosts them) are watched, the young called in; one coming near is met and greeted —
  a guest, fed when hungry by the people's hospitality, known better by time near and by gifts,
  taken in by an evening's weighing when most of the grown trust it after two days; in crowded
  country, or with a bad name, warned off and, staying, threatened. Humans no longer flee a calm
  player; the player hands a thing to the person looked at within reach (E) as a gift, and is
  told when greeted, warned off, taken in or quarrelled with. Camps and councils are only for
  peoples that keep camp (`KeepCamp`); a band that splits or leaves keeps its own camp
  (`tests/social.rs`: a grievance had out and eased; a quarrel talked round by one respected; a
  feud parting the band; a stranger greeted and taken in; an unwelcome one warned off).
- [x] (g) The acceptance (`tests/social.rs`): word of a theft seen in the dusk by the one robbed
  spreads through a band of nineteen by talk within half a day, those who heard it less sure than
  the one who saw; a theft angers, is told, brings mockery and at last casting out; a band whose
  country has gone poor debates where to go and moves its camp; quarrels rise and ease; strangers
  are greeted and taken in, or warned off. Built with `clippy` checks only (the PC's memory was
  exhausted), these tests and (e)–(f)'s ran on 2026-10-04 in the cloud and pass after the fixes
  D190 tells; the perf gate waits for the PC's GPU.

H5 — Culture and language (PLAN.md, V2.1 §9–10; `docs/design/humans/culture.md`), in parts (built
while the machine's memory allowed only `clippy` checks; their tests ran with H4's, D190):
- [x] (a) The culture model and generator (`culture.rs`, `humans/culture/generators.ron`, D185):
  every band's culture — its lineage, four values, residence, descent, polygyny, the share of each
  work its women do, burial, greeting, taboos, motif and its own ways with strangers and quarrels
  — drawn from the foragers' cross-cultural spans on a stream of its own; daughters for bands that
  split off, leave or are cast out; the inspector's culture (`tests/culture.rs`).
- [x] (b) Culture in what people do: its ways in all of H4's strangers and quarrels; tightness
  moving when sanctions begin; honour raising quarrels and slowing backing down; residence
  deciding where a pair of two bands lives; the division of labour weighing each work by the
  chooser's sex; the dead laid to rest as their culture has it, grave goods kept from heirs;
  taboo foods left uneaten unless starving (`tests/culture.rs`).
- [x] (c) Transmission and evolution: each month a band's people take in its culture — their
  values toward six parts temperament, four parts culture, the young a tenth of the way a month,
  the grown a hundredth, the conforming the more; each year a culture drifts (values, work
  shares, now and then a custom, its motif) and meets its people's other bands within 20 km, the
  nearer the more, values drawn together and customs borrowed; daughters part where they do not
  meet (`tests/culture.rs`: five hundred years apart and in contact; the young taking a culture in
  sooner than the grown).
- [x] (d) Languages (`language.rs`, `humans/language/`, D186; `docs/design/humans/language.md`):
  each culture of a people with language speaks one drawn from the world's sounds by how common
  each is, its syllable shapes, word order and affixes, and a word for each of about a hundred
  and ten core meanings; everyone is named in it at birth (a player's person keeps the player's
  name); a daughter culture speaks a daughter language, and each year now and then a regular
  sound change runs through every word at once and a word gives way to a new one; neighbours
  lend words; the inspector shows a band's language and a few of its words (`tests/culture.rs`:
  two daughters five hundred years apart still mostly cognate, against a stranger's tongue).
- [x] (e) Speech acts and gestures (`speech.rs`, D187): every exchange a structured act — what
  it does, to whom, its words as meanings and names in the speaker's word order, a gesture —
  spoken through what H4 already does (greeting, warning off, quarrel, backing down, mediation,
  amends, sharing, gifts, mockery, gossip, the council, taking in), kept half a minute for whoever
  hears it.
- [x] (f) Subtitles and learning: what is said within 20 m of the player shown with the words as
  they sound and what the player makes of them (known words glossed, half-known doubted, unknown
  as dots); a born player's mother tongue its family's; other tongues learned by hearing — faster
  spoken to, twice with a gesture — and a related tongue partly made out by cognates.
- [x] (g) The acceptance (`tests/culture.rs`): a split people's two cultures two hundred years on
  differ in values and customs, their tongues related and not a stranger people's; a newcomer
  makes out little of its hosts' speech at first and most after half an hour among them. As with
  H4, the tests ran on 2026-10-04 and pass (D190).

H6 — Knowledge and social learning (PLAN.md, V2.1 §11; `docs/design/humans/learning.md`), built
while only `clippy` checks could run (its tests ran with H4's and H5's and pass, D190):
- [x] (a) Learning across a life and the collective brain (`learning.rs`,
  `humans/learning/transmission.ron`, D188): each year the young (and the grown, less) learn each
  technique whose groundwork they have from the knowers of their band and its neighbours, the
  deeper the slower; the curious now and then find something out; a band knows what its living
  members know, a technique none knows is lost (kept as a legend by its grown) and its processes
  with it; persons are offered only works they know, and a debug assertion guards every work.
- [x] (b) In play: one at its work shows how to one watching it of its band or trusted (four times
  watching's pace for our kind; `KnowledgeState::taught`, `Route::Taught`), saying so; the grown
  watch works they do not know; the young take a master each year; evening stories pass on places
  and, as legends, techniques; the player is shown how by people at work in front of it who would
  teach it.
- [x] (c) The acceptance (`tests/culture.rs`): a deep technique lost by a small band alone in a
  hundred and fifty years and kept by a large connected one; the player shown how learns a work in
  minutes that watching alone does not teach.

H7 — Tiers and persistence (PLAN.md, V2.1 §17; `docs/design/humans/tiers.md`, D189), built while
only `clippy` checks could run (its tests ran with H4's and pass, D190): a household tier between full and dormant — bands away from the
player but within 40 km, or holding one the player knows, kept whole and lived by the life course
(pairing, crowding and culture count them), lifted back into full about their camp as the same
persons; bands met for the first time founded with coherent families (mothers of age, births
spaced) and forebears (dead mothers kept as stubs, making brothers and sisters of the founders); a
save keeps bands in full as households; at most 300 in full and 20,000 as households; the long dead
pruned to genealogy stubs (`tests/persist.rs`).

H8 — History simulation and Paleolithic eras (PLAN.md, V2.1 §15; `docs/design/humans/history.md`
and `eras.md`; D191–D206), in parts:
- [x] (a) The archaic peoples: *Homo erectus* and Neanderthals implemented — species and
  population profiles, life tables that replace themselves (D198), culture generators, a
  proto-language for *erectus* (D186's generator gated by the kinds of word it has) and
  Neanderthal languages, their days, their childhoods' moments, their body plans
  (`Appearance::plan`: *erectus* long-legged and narrow-hipped, Neanderthals short, broad and
  barrel-chested).
- [x] (b) Era profiles and the selector: Lower, Middle and Upper Paleolithic playable, each naming
  its peoples, their band sizes, what they may come to know, their camps, seasonal rounds and
  gathering; Create World chooses the era, telling how its people live, and whom one may live on
  as after death.
- [x] (c) The deep-time layer (`hearth_people::history`, D191–D195): demes of each species on a
  history grid of the planet's own geography from 1.9 million years ago to the era's date — the ice
  ages' sea and cold, a Fisher wave along coasts and rivers, the sea crossed within reach, land
  bridges, competition between species, gene pools, a lineage tree whose cultures and languages
  are replayed along it, knowledge invented, spread and lost by numbers and contact, the chronicle
  and a census; kept with the world; `hearth history` runs and tells it
  (`hearth_people/tests/history.rs`).
- [x] (d) Peopling the world and the recent past (D196–D198): only the era's peoples live in its
  world, at deep time's densities (a small planet's up to thirty times real, so a Standard world
  holds a mating network of each); a century of households lived about the place a life begins;
  two to four households offered for a birth (the rule eased step by step where few live, the
  nearest anywhere where none do); the player born into one with their species' body.
- [x] (e) Camps, seasonal rounds and gatherings (D199): each band of a fire-keeping people keeps
  its fire at camp — the player's own campfire, lit from carried embers and fed while they are
  about — with beds of grass or furs for each household; camps on dry ground 800 m apart, moved
  with the seasons to the water, the uplands or the shelter of the woods the land offers (`Lands`),
  the same places year after year; a band split off goes to dry land of its own; the Upper
  Paleolithic's autumn gathering of the nearest six bands of a people for twenty days of the real
  year, who meet, marry across bands and learn from one another
  (`hearth_people/tests/rounds.rs`).
- [x] (f) After death and births (D200): living on as anyone of the world lived in full or as a
  household, children too (their childhood taken up at their age), by the filters family, group,
  near and anyone and the world's scope setting, never one fighting, fleeing, dying or giving
  birth; being born again as a baby about where one died or a place picked on the globe, into a
  household offered there; Wild Earth's families as many as the players a world expects
  (`tests/afterlife.rs`, `tests/eras.rs`).
- [x] (g) The era reviews (`docs/review/era-lower-paleolithic.md`, `era-middle-paleolithic.md`,
  `era-upper-paleolithic.md`): a sample week of each era's people about a life born among them,
  the player following its band (`tests/era_week.rs`, ignored by default); their camps in the
  afternoon and at dusk (`tools/shots/h8_eras.shots`, rendered on the cloud machine's software
  device); deep time on the Standard planet and on an Earth-sized one of the same seed (`hearth
  history`); the archaic peoples' lives against their tables (`hearth_people/tests/
  demography.rs`). What the first runs changed is D201; what the sample weeks found and what
  was done about it — the people living through the night at their camps (D202), keeping near
  their camps (D203), camping by water (D204), going to the fire when chilled (D205), finding
  out only what their era could (D206) and keeping in the camp's lee by day, its children held
  (D207) — the rest.

H9 — Observer mode and the player in society (PLAN.md, V2.1 §15.4 and §16, Addendum B §2;
`docs/design/humans/observer.md`; D208–D212), in parts:
- [x] (a) Life after death in full (D208): *knowledge after death* — theirs only, a head start
  (what earlier lives knew comes back as legends), keep everything — kept per player through
  every death; being born again a setting of its own; v2's death rules retired into the presets
  Authentic (the default), Legacy, Hardy and Permadeath, each setting changeable at Create World;
  saves of format 3 migrate to 4 (`hearth_save` migration test, `hearth_craft/tests/
  after_death.rs`, the server's death test).
- [x] (b) The player among people (D209): looking at a person tells only what the player knows of
  them — a name once learned, kinship or band, a rough age, whether paired, how they seem to take
  the player, the ledger, what it has heard of them; speakers named so in the subtitles; the talk
  wheel (K): greet, tell one's name, thank, praise, joke, apologise, ask to be shown how, offer to
  show them, ask to stay, ask to pair, beckon, embrace, threaten, insult — each answered as the
  person's ties have it; lessons asked for or offered, twice as fast as being shown
  (`hearth_people/tests/player.rs`, `tests/observer.rs`).
- [x] (c) A family: pairing mutual only; of an age by the people's life table; courting makes
  the player's intent known and she waits for it a year (D211); a player's person has children
  as anyone (abstracted); a body grown at a stroke keeps its blood and stores (D212: the
  children of the first runs died of "blood loss" at every skip of time).
- [x] (d) Joining a group: asking to stay makes the player a band's guest, taken in once most of
  its grown trust it; obligations as the ledger in what the player sees of a person.
- [x] (e) The Observer (D210): watching alive (the player put aside: still, unharmed, unseen) or
  dead, from the pause menu, the death screen or the worlds list; the world streamed and lived
  about the eye; following a person or an animal (E), a person's life read; time from stopped to
  a hundred years a second ([ and ]); the chronicle (J) of the living world's notable events and
  deep time's, each a place to go; the globe's overlays (1–4: people, cultures, knowledge, looks);
  stepping back in with Esc.
- [x] (f) The fast-forward's budget (V2.1 §17.2): at a month a second and faster the people at the
  demographic tier, the land's growth held, the animals as numbers, the slow work coarse — ten
  years in 0.8–0.9 s in a release build on the cloud machine (`tests/observer.rs`'s ignored
  benchmark); the fauna catch-up no longer saved up under the warp.
- [x] (g) The acceptance (`tests/acceptance_h9.rs`): a scripted player born into an Upper
  Paleolithic band asks one of its band to show it how and learns; courts girls of the band
  season by season until one is willing at twenty and pairs; their children are born and one
  grows up; the player dies and lives on as their grown child, told who it is.

H10 — Optional conversation backend (PLAN.md, V2.1 §10.4; `docs/design/humans/conversation.md`;
D213–D216), in parts:
- [x] (a) The backend (D213): `hearth_ai`, a trait with none (the default), a local server and a
  provider's model through the chat-completions and Anthropic Messages adapters; no model named
  by the game (typed, or chosen from the server's list); the key read from the variable the
  player names, never saved; asked on a thread of its own, a player's typed words first, a job
  waiting too long let go; Options → Conversation, with a Test and the provider's note
  (`hearth_ai/tests/adapters.rs` against a mock server of each API).
- [x] (b) What the model is told: a speaker's own state only (`hearth_people::converse`) — name,
  looks and age, nature, feelings, values and its people's, the techniques it knows, its kin,
  what befell it lately, whom it speaks to and how it stands with them, the names it may say, the
  act in its words; the prompts as data (`data/hearth/ai/prompts.ron`).
- [x] (c) The knowledge and anachronism filter (D214): a closed vocabulary of some 1,800 everyday
  words, the glosses, the words of the techniques a speaker knows (311 owned by techniques), names
  given it and kin it has; later ages' words and words never said labelled; a spoken line's form
  and the act's sense kept (`data/hearth/ai/words/`).
- [x] (d) Phrasing (D215): lines said to the player and nine tenths made out, overheard ones when
  the backend is idle; the templated line at once, the phrasing in its sense's place when it
  passes in time; the same act to the same listener said the same way.
- [x] (e) Reading typed words (D216): T opens a line to the person looked at; read as a wheel's
  act, made at once only when plain and not a proposal or an insult, else offered with the
  words' cue-word guesses (`data/hearth/ai/cues.ron`) to choose from.
- [x] (f) The acceptance: with no backend nothing changes (typed words not heard, no phrasing or
  choice sent; `tests/conversation.rs`); with one, a conversation suite of every act a band said
  over eight minutes — the prompts holding nothing the speaker does not know, 91 willing lines
  passing and 17,159 leaking ones refused — and two worlds of one seed, wheel in one, typed words
  and every line phrased in the other, ending byte for byte the same (`hearth_ai/tests/
  conversations.rs`); through the server a greeting phrased, a leaking phrasing never sent, typed
  words answered or offered (`tests/conversation.rs`).

V2-12 — Neolithic (PLAN.md, v2 Era 3; `docs/design/neolithic.md`; D217–D221), in parts:
- [x] (a) Pottery and kilns (D217): a pot coiled, dried to greenware (rain slumps it back to
  clay), fired in a campfire, a firing pit or an updraft kiln; fired in its clay's range by the
  heat its batch got (`hearth_craft::firing`): under-fired stays clay and is told so, over-fired
  slumps; a fired pot holds water (`tests/pottery.rs`).
- [x] (b) Fields and grain domestication (D218): tilled plots, sowing by the season, weeds,
  pests, irrigation, nitrogen drawn down and restored by fallow and dung, reaping with a sickle,
  threshing, a saddle quern, bread and porridge, storage pits; seed carrying its lot (tough
  rachis, grain weight, dormancy) through every step, changed only by the harvest's selection.
- [x] (c) Herds (D219): the mouflon and the bezoar goat of the Fertile Crescent's hills; kept
  animals with a heritable makeup and a learned tameness, tethered, led, bred in their rut,
  milked, plucked, slaughtered; milk curdled to cheese; kept animals saved apart from the
  populations; a bred fleece's coat (`hearth_fauna/tests/herds.rs`).
- [x] (d) Spinning and weaving (D221): flax retted and scutched, spindles, yarn, a warp-weighted
  loom weaving linen and woollen cloth by the length, tunics and cloaks sewn of it.
- [x] (e) Timber and houses: ground stone axes and adzes, wedges splitting planks, hewn posts
  and beams and plank walls and floors for the farmers' longhouses.
- [x] (f) Moving loads (D220): sledges on runners, the handcart on wheels, the dugout canoe
  paddled (the mover's boat mode), the potter's wheel (`tests/neolithic_crafts.rs`,
  `hearth_physics/tests/movement.rs`).
- [x] (g) The acceptance: a bot domesticates einkorn over twelve harvests and its yields rise
  (`tests/acceptance_v2_12.rs`, `--ignored`: tough ears 0.3 % → 99 %, grain 12 → 22 mg, dormancy
  50 % → 2 %, 0.03 → 0.105 kg a square metre); a herder's mouflon line becomes docile and woolly
  over sixteen years (`tests/acceptance_v2_12_herd.rs`, `--ignored`: the first born in the keeping tame 0.36, those born from year ten 0.45; wool 0.31 → 0.66 kg); under-fired
  pottery fails (`tests/pottery.rs`); the screenshots (`tools/shots/v212_neolithic.shots`).
  Found at the finish and fixed: a lamb born in the keeping shows its line's temper at its
  mother's side, so a herder can choose among the young (they were all equally tame, and
  breeding for temper went nowhere); the player and its partner share a hearth (a pair kept
  in two households had no children); the one the player lives on as is told of its parents'
  deaths; the sea turtles' hatchlings no longer crowd out the grown (a hundred eggs a clutch
  counted as turtles against what the sea holds: they are on the population model of the fish
  and the small, the young among themselves); a kill that ran past where its herd is folded
  back into the numbers before it fell no longer vanishes; saplings no longer drawn with the
  missing texture. The breeding is checked fast too: a dozen flocks bred sixteen years by the
  herder's rules in a fraction of a second (`hearth_fauna/tests/herds.rs`), the world's run
  taking an hour; some five generations make the grown line calmer by about a tenth.
  Known from V2-12: `hearth content lint` warns that era 3 is cheaper from scratch than era 2 —
  the Neolithic's crafts (a pot of clay, a field of seed, a tethered lamb) take few materials
  but seasons of time, which the measure does not count.

S0 — Baseline and prototype (Amendment S, `docs/spec/amendment-s-smooth-world.md`; PLAN.md;
`docs/design/smooth-terrain.md`; D222), done 2026-10-07:
- [x] `MIGRATION_SMOOTH.md`: every subsystem the smooth world touches, Keep / Modify / Replace,
  with the S milestone that changes it; `docs/design/art-direction.md`: stylized realism, its
  references by description, palette, material families with their sharpness, scale and
  detail by distance, the checks a screenshot must pass.
- [x] `hearth_smooth`: the fill (a voxel's signed distance to the surface, ±1.5 voxels in a
  byte, 1.2 cm steps) and three dual meshers — Surface Nets, Surface Nets with sharp features,
  Dual Contouring — with the feature solve (`qef`) and crease-keeping gradients. Meshed in
  cubes with a two-sample apron, every vertex computed in its own cell's coordinates, they meet
  bit for bit: `tests/meshing.rs` meshes a rough field and four random ones whole and in 27
  cubes and finds the same triangles and vertices for all three; a sphere closed and true, a
  tilted plane flat, determinism, weights summing to one heaviest first, rock keeping its crest
  and sand rounding it.
- [x] `bench smooth`: the eight test scenes as fill fields (rolling hills, sea cliffs, a cave, a
  dune field, a riverbank, a talus slope, a dug pit and its spoil, a mountain ridge), meshed,
  measured and rendered on the CPU against the blocky grid (`docs/review/s0/`), and the shading
  prototype on turf and limestone.
- [x] The decision (D222): Surface Nets with sharp features — sand, soil and snow soft,
  limestone and granite crisp, features narrower than the grid soft grooves instead of teeth;
  3,553 surface cubes a second on one thread (S §12.2 asks 2,000 on eight cores), 1.45 cm mean
  error, 2.4° normal error, 61 folded triangles across the scenes (Dual Contouring 542, Surface
  Nets 2.50 cm and melted rock). Shading: biplanar mapping with height blending (triplanar's
  third sample changed the image by 0.56 of 255).
- [x] Baseline-S (`BENCHMARKS.md`): today's near terrain on the CPU (`hearth bench
  --terrain-only`: 0.28–0.77 ms to mesh a surface cube on one thread; 8.5–43 KB of mesh and
  2–5.6 KB of memory a surface cube; an edit 43 bytes saved and a whole cube re-sent), the
  prototypes' numbers, and today's look (`tools/shots/s0_baseline.shots`,
  `docs/review/s0/baseline-*.jpg`, `docs/review/smooth-world.md`). `hearth bench` takes
  `--preset` and reports the median and worst frames, for the GPU half.
  Known from S0: the GPU half of Baseline-S (High and Low at 1440p) waits for the owner's PC
  (`scripts/baseline-s.sh`); cut soil's edges round under soil's sharpness (an edit's faces may
  get a sharpness that weathers, S1); on a huge planet a high view can overflow the distant
  terrain's quad arena (the alpine baseline shot, 10.6 M quads against 8.4 M; S4).

P0 — Triage and quick fixes (Amendment P, `docs/spec/amendment-p-playability.md`; PLAN.md;
`dev/PLAYTEST.md`; `docs/design/motion-timing.md`; D223), done 2026-10-08:
- [x] `dev/PLAYTEST.md`: the owner's reports and the amendment's findings as eighteen numbered
  issues, each with how to reproduce it, its cause and its status; the causes of the six fixed
  here reproduced (two of the amendment's guesses were not the cause: the clouds and the waves
  raced because a changing wind was multiplied by the world's whole age, and the stars flashed
  because they were cut at their grid cells' edges, not from their twinkle).
- [x] Plants slow a body by how high they reach on it (P §10.1): each column of plants the box
  overlaps slows it by its density × (its reach ÷ the body's height)² × its share of the box's
  footprint, a plant as tall as its species grows; every grass and herb below the knee costs a
  sprint at most 2 % (steppe fescue 0.5 %, heather 1.2 %), knee- to waist-high grasses 2–5 %,
  shrubs 10–40 % (`hearth_physics/tests/movement.rs`).
- [x] Motion at real speeds (P §8): the clouds summed frame by frame at the wind at their height;
  the waves at the deep-water phase speed of each scale (swell 3.5 m/s, chop 1.8 m/s); rain and
  snow drifting by the summed wind on a seamless clock; the stars summed over their neighbouring
  cells at their own crisp width, their light steady to 2–4 % where it swung 27–33 %, with a
  scintillation of a few hertz, stronger low and in wind (`docs/review/p0/`,
  `tools/shots/p0_motion.shots`); `docs/design/motion-timing.md` lists what moves, by which
  clock and how fast (completed in P5).
  Known from P0: the motion-timing check that renders sequences is P5's; cloud shapes do not yet
  evolve; the stars' width at render scales below about 1000 pixels of height is wider than at
  full size.

P1 — Menus and world management (Amendment P §4; PLAN.md; `docs/design/menus.md`; D226), done
2026-10-08:
- [x] Layout that fits: every page's rows in a scrolled area (clipped drawing, the pointer only
  within it, wheel, bar, keys and controller), the footer held at the bottom; Video in tabs
  (Display, Quality, Distance). The layout test (`tests/layout.rs`) lays out 21 screens at
  seven resolutions, five interface scales, full screen and windowed (1,470 layouts) and finds
  no widget out of reach, overlapping or with its text cut.
- [x] Worlds: name, era, the player's name and age, play time and last played; Rename,
  Duplicate, Back up (dated copies), Open folder, Delete (asked, naming the world) to a trash
  kept thirty days, restored from the Trash screen, emptied there or in Options
  (`worlds::tests`).
- [x] Create World: a name, a seed, the player's name and the era; More options for the planet's
  size, the height of the land, the day's and season's length and the starting season (saved
  with the world). The death and knowledge rules and the loincloth are gone from it (P2's modes
  set them).
- [x] The planet made with its stages shown, then the globe to choose where to be born
  (Recommended, Surprise me, or a click; the place under the pointer described); the world
  opened with its birthplace kept (save format 5) and the life born near it
  (`tests/server.rs`); the world's opening told with a bar and a tip (`ToClient::Progress`).
- [x] Pause: the time in words, Back to the game, Options, Watch, Save, Save and quit.
  Known from P1: the world list has no globe thumbnail yet; the globe's hover words are its
  climate and land (who lives there and its dangers come with P2/P6); the mode on Create World
  and the world's Edit come with P2, the Field Guide with P6.

P2 — Game modes and Creative (Amendment P §2–3 as amended by Amendment E §9.1;
`docs/design/modes-creative.md`; D229), done 2026-10-08:
- [x] Realistic, Easy and Creative (`data/hearth/balance/modes.ron`): chosen on Create World with
  their summaries, their rules applied to the world and kept with it; a world's Mode changed on
  the Worlds screen only toward less strict, Creative marking it for good (`worlds::tests`).
  The server refuses, outside Creative, watching, being put elsewhere, moving time, holding the
  weather and Creative's acts; the client hides them; F3 shows only performance there unless
  Developer mode is on (`tests/creative.rs`).
- [x] Creative: unhurt (no injury, illness, fall or drowning; the body kept whole), flight by a
  double Jump with the wheel's speed and no-clip (F7); the inventory (terrain, plants, animals,
  every item form and material, every piece in every material and the stations, knowledge;
  searched) taking, placing, planting young or grown, summoning herds, building, teaching, its
  actions instant; pick and remove (Delete); time and weather on the pause menu; clear view
  (F4) and its parts; spectating (F6) with the world streamed about the eye, resuming on the
  ground there. No people: per Amendment E none are summoned or removed.
- Real? The modes set Earth-calibrated presets; Creative's spectating streams the real world
  about the eye (checked 400 m off). Lean? The old Create World rule settings were already gone
  (P1); the people's summoning written at first was taken out again for Amendment E; nothing
  unused added (the catalogue reuses the content and item registries). Fast? No per-frame cost
  outside Creative; the catalogue is built once at Ready. Whole? The new screens pass the layout
  test at every resolution and scale; clear view is a filter of the same lighting, not a second
  look. Organic? Summoned herds stand scattered, not in a row; nothing tiled added.
  Known from P2: terrain and plant brushes (S1, P7G), favourites, item quality and outlines in
  clear view are not built; pick stays on the middle click until P3.

## Next steps
0. Every milestone ends with `scripts/perf-gate.sh` (≈10 min: builds the baseline commit in
   `perf/baseline` in `bench-out/gate`, three alternating rounds of the quick scenes); a fall
   of more than 5 % in average FPS or 1 % lows is fixed or justified in DECISIONS.md and the
   baseline moved with `--accept` on a committed tree (then commit `perf/baseline`). For
   one-off claims, A/B alternate builds as the gate does (or `--lod-error` / `--render-scale`
   / `--water` within one build); capture golden images with `hearth bench --golden DIR`
   before comparing looks.
1. H9 — Observer mode and the player in society — is done (2026-10-06; D208–D212): life after
   death in full with its settings and presets, the player among people (what it knows of a
   person, the talk wheel, being taught and teaching, joining a band, courting and a family), and
   the Observer with its chronicle, overlays and a fast-forward within its budget. H8 — History
   simulation and Paleolithic eras — is done (2026-10-05; D191–D207); its reviews are
   `docs/review/era-*.md`. The perf gate (`scripts/perf-gate.sh`) and the screenshots' looks
   need the PC: the cloud machine renders only on a software device (llvmpipe), whose frame rates
   say nothing. Amendment R (`dev/AMENDMENT_R.md`) waits until V2-16; only its multiplayer-ready
   rule applies (D166). H10 — the optional conversation backend — is done (2026-10-07;
   D213–D216): off by default; set up, a local or a provider's model phrases what is said to the
   player within a closed vocabulary of what the speaker may say, and reads typed words as a
   wheel's act. Then V2-12 — the Neolithic (PLAN.md).
   Known from H10: no real model was asked (the cloud machine has none, and tests never call a
   service): the adapters are tested against mock servers of each API, and how often a small
   local model's lines pass the closed vocabulary is to be seen on the PC; the Options screen's
   conversation rows and the typed line are drawn but not yet seen on a screen; the vocabulary is
   English only.
   Known from H9: the talk wheel, the regard and the Observer's headline, chronicle and overlays
   are drawn but not yet seen on a screen (the cloud machine's shots draw no interface); trading
   and inviting to a plan wait (V2.1 §16); a culture's rite of adoption is the guest's taking-in;
   the living world's notable events are not saved; the Upper Paleolithic band of the
   acceptance's seed shrank from 33 to 13 over twenty-five years of skipped time (H8's tables and
   crowding to look into).
   Known from H8 (the era reviews have the detail): the people are drawn bare (they wear what
   they know against the cold, D202); the world is drawn at today's climate and sea level, not
   the era's (D195); the people raise no shelters (H11) and make little in a week, their food the
   day's take eaten at camp (D202) until hunting and gathering are lived in full; a band lived in
   full moves camp only within a short walk (D203: long moves wait for paths found over the
   land); *erectus*'s proto-language says little more than names, and in a quiet week nothing; deep time lets Neanderthals
   live in the tropics; the recent past is not yet the same from run to run of a seed; the
   recent past takes a minute or two in a debug build, and the finite water simulation about a
   camp by water slows a debug build several times over; a first life's place picked on the
   globe waits for V2-15.
   Known from H4–H7: levelling among egalitarian foragers waits for the interaction of values;
   the people do not yet give gifts to strangers themselves, nor raid (H11–H12); the household
   tier has no coarse daily outcomes (food got, work done) beyond the life course's crowding; a
   mother carrying her infant is not drawn holding it.
2. Carried forward from the slice review (`docs/review/slice-1.md`, "Left where they belong"): a
   kill is more than one person can use in summer (sharing comes with others, H4); scavengers
   take a carcass within hours of play (the populations' year scale); joints are drawn, not
   built; a ridge piece for odd-span gables; drying racks spoil whole in the rain, unexplained to
   the player; dusk lights the land brighter than the sky; the year bot should sew furs for its
   next winter.

## Known issues
- Night by a fire (H8, D201): a camera in the dark looking at a fire from beyond its light washes
  the firelit ground out white — the eye's adaptation is reckoned from the light where the camera
  stands (the moonlight floor), not from what it looks at.
- Finite water (V2-8), since V2-10 (e)'s rivers: a channel or pit dug through a river's bank where
  it stands a block below the water, or beside a stream stepping down its bed, takes the river's
  water without end and sends it down the land, and the water never rests there (the hydrology's
  rivers do not fall as a breach takes from them; D190). `tests/finite_water.rs` digs where the
  banks hold.
- Hominins (V2-11): an agent's sleeping place is a limb of the crown found from the cells about
  the trunk, so in a crown without limbs near the trunk it sleeps beside the trunk's top; their
  play-time bodies step at the server's tick and are not advanced by sleep's skipped hours; a
  group's tool site is laid only under a nut tree within 70 m of where it is first drawn out.
- In this environment presents never block (FIFO on both Vulkan and DX12 ran at ~1.5–2k FPS
  with terrain), most likely because the window is occluded. Re-check pacing on a visible
  window; the frame limiter covers the vsync-off case.
- Oceans (V2-10 (f)): the sea's realms are its coasts', so the North Pacific's animals are on
  the Atlantic coasts of their realms too (D152); seabirds are drawn only where a cell has land
  (no rafts on the sea), crabs, seals and fish of the sea are not drawn walking, and the deep's
  animals are drawn near the surface (lanternfish rise at night, the anglerfish is never seen
  in its dark); the bioluminescence is the sea pens', not the animals' (no glowing coats); the
  sea's weeds and grasses share the generator's kelp, wrack and seagrass blocks (D157); a thin
  palm trunk leaning is drawn as steps of limb blocks; the mangroves' floor is mud a block over
  the tide, their channels' fish not kept (D159).
- Wetlands (V2-10 (e)): ducks, geese and swans stand on the bank rather than swim, and a bird
  walking is drawn with its wings spread; the reed beds and papyrus stand two to four blocks tall
  and hide much of what is in them; where the waters are patches in dry land a territorial
  hunter of them (the otter) holds on in few numbers (D148); the earlier waves' warm-land
  animals name no coldest month yet (D146).
- Mountains (V2-10 (d)): the planet's tropics have seasons, so its tropical mountains have a
  winter the Earth's have not and their tree lines are subarctic by their class (D136); a
  realm's mountains are one community (D140); old monkey puzzles' umbrella crowns are seldom
  seen, a generated forest's trees being mostly young and conical.
- Animals (V2-10 (c)): a lion's mane is not drawn (the male is a darker lioness), nor a peacock's
  train (both sexes carry a middling tail); the hippo's plan waits for the wetlands (e). Trees'
  thin limbs under a dense crown are drawn nearly black (the lighting gives them no sky), so a
  strangler fig's lattice of stems reads as a dark scaffold.

## Deferred
- Fauna (V2-7): birds and insects seen (flocks crossing the sky, insects over flowers) and
  posing bodies on the GPU with them; territorial defence at dens and animals drawn to people's
  food; whole hides by species and hide-working times by area (hides come as sheets of a
  kilogram); drying spoiled meat still makes good dried meat; prints spaced by the walking
  stride at every gait; coats from resource packs.
- Open lands (V2-10 (b)): the understory's flowers and seed heads are drawn the year round (a
  steppe looks the same in spring and late summer; seasons wait for the plants' calendars), and
  a desert's flowers after rain are not drawn; the saiga's and the onager's long wanderings are
  not simulated (their herds keep a home range).
- Fauna (V2-10 (a)): migration is leaving and coming back, not the journey (the tundra's
  reindeer herds stay in both their lands); a musk ox's skirt of hair; the open tundra's light
  makes dark coats look brown (exposure on bright open ground).
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
  world's first spawn: coming back to one's own camp was not among the slice review's issues
  (V2-9) and waits for settlements (V2-12).
- Containers lying in the world are opened by picking them up; things do not yet get wet in
  the rain or dry out (each keeps a wetness that spoiling and burning read); no rolling logs, travois,
  sledges or rafts yet (log sections are dragged); heavy loads do not yet weigh a swimmer down.
- Building (V2-8): snow is not laid on built pieces, so roofs carry no snow load yet; decay
  is not shown on a piece's texture (the builder's view shows the weakening); statically
  indeterminate frames are cut at their supports (erring safe for spans); stone domes and
  corbelled vaults do not stand (no arch action); log walls wait for timber framing (V2-12).
- Sounds are all the player's own or around them: sounds placed in the world (direction,
  distance, occlusion) come with fauna (V2-7), and with them the work of the hands (knapping,
  scraping, digging) and fire's crackle; no music, thunder, flowing water or animal sounds
  yet (their volume sliders are hidden); the echo has one character, not measured from the
  cave's size; captions show no direction.

## Environment notes
- Rust was installed via rustup in `%USERPROFILE%\.cargo`; shells started before the install
  need `export PATH="$HOME/.cargo/bin:$PATH"`.
- Machine: Ryzen 7 7840HS (8C/16T), RTX 4060 Laptop + Radeon 780M, 16 GB RAM.
