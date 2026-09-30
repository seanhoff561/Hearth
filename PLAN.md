# Plan (v2)

The game follows `docs/spec/v2-direction-change.md` (v2), which overrides the original build
spec `docs/spec/v1-build-prompt.md` (v1) where they conflict. `MIGRATION.md` maps every v1
milestone and subsystem to its fate. Each milestone ends with: design doc(s) in
`docs/design/` updated, data added, tests, `scripts/check.sh` and `hearth content lint` green,
`PROGRESS.md` (incl. the Content Status table) updated, and a commit.

Engine work that v1 scheduled but that is not done yet is folded into the first v2 milestone
that needs it (marked **Engine**), keeping v1's acceptance criteria for it.

## Completed v1 engine milestones
- **M0 Foundation** — workspace, options, input actions, window, wgpu bring-up.
- **M1 Voxel core** — wrap-aware planet coordinates, registries, block states, palette cubes,
  heightmaps, raycasts.
- **M2 World generation** — planet model (tectonics, erosion, drainage, climate), regional
  sampler, biomes, cube generation, caves, features.
- **M3 Near-field rendering** — texgen, texture array, greedy meshing, GPU-driven culling,
  translucency, headless screenshots, fly-camera preview.
- The light engine part of v1 M5 (sky/block light, cross-cube BFS, incremental updates).

## V2-0 — Migration & content platform
- `MIGRATION.md`, this plan (done with the direction change).
- `hearth_content` crate: loaders for every domain of §3.1 (RON preferred, JSON accepted),
  serde structs as schemas (`deny_unknown_fields`), `notes` / `realism_source` / `uncertain`
  on every entry, `status: implemented|planned` where relevant.
- Units (SI internally, `units.ron`), `time.ron` scales, balance layer with Authentic / Hardy /
  Custom presets (`balance/`).
- Form × material generation of items and block families.
- `hearth content lint` (schemas, cross-references, reachability, habitats, food-web producers,
  unit sanity, file/line diagnostics) and `hearth content graph` (DOT/SVG + HTML under
  `docs/generated/`); lint runs in `scripts/check.sh`.
- Hot reload (F3+T) plumbing: reload registries, report systems that can't reload.
- **Engine (v1 M4 saves):** world directory with versioned metadata, content registry mapping,
  region files for cubes; migration framework; format 1 refused with a clear message.
- World settings struct for v2 options (Knowledge Mode incl. `Open`, realism preset, etc.).
- Remove dropped v1 content (ore blocks and MC ore bands, MC workstations, beds, wool colours,
  MC-only building blocks) and their textures/models.
- `docs/design/` skeleton (one doc per system) + `interactions.md` matrix.
- *Accept:* lint passes on the seed dataset; format-1 worlds refused with a clear message; a
  V2-0 save loads after a deliberate format change via a migration test.

## V2-1 — Time, calendar & seasons
- Calendar (day length 20–120 min, days per season 3–91, starting season), the two time scales
  in one module (`time.ron`), axial tilt → solar declination, sun position and day length by
  latitude, polar day/night, synodic moon; hemispheres.
- Seasonal climate: monthly temperature/precipitation from the planet climate (seasonal
  amplitude, ITCZ shift, monsoons), weather cells that follow it (rain/snow/storms, wet/dry
  seasons), snow cover accumulation/melt (snow layers), lake/river ice growth and thaw,
  permafrost flags.
- Phenology state plumbing (per-species calendars come with flora in V2-6); seasonal grass/
  foliage colormaps as GPU parameters (no remeshing).
- **Engine (v1 M6):** atmosphere (transmittance/multi-scatter/sky-view LUTs), sun/moon/stars by
  latitude and local time, day-night lighting, HDR + tonemap + auto-exposure, realistic
  darkness, fog/aerial perspective.
- **Engine (v1 M10 part):** weather rendering (rain/snow particles, clouds).
- *Accept:* screenshot suite at 4 seasons × 3 latitudes; unit tests for solar position and day
  length against known values; a headless year shows correct snow, ice and wet/dry timing by
  latitude.

## V2-2 — Geology, soils, hydrology & resources
- Geological provinces from the tectonic history; stratigraphy (flat and folded layers, dips,
  outcrops), crystalline basement, geothermal gradient; volcanic landforms and rocks.
- Soils from climate × parent rock × vegetation × drainage × slope, with profiles; clays, sands,
  gravels, peat, loess, alluvium, permafrost.
- Groundwater table, springs, seasonal river levels, floodplains; finite conserved
  player-moved water with levelling and flow (**Engine, v1 M5 fluids**); water quality.
- Coasts: coral reefs (fringing, barrier, atolls), kelp, mudflats/salt marsh, mangroves, rocky
  intertidal, sea ice.
- Deposit models for every Appendix D resource with surface indicators (gossans, stains,
  float, placers, indicator plants); panning.
- Surface detail: layer blocks, loose stones/pebbles/boulders, talus with angle of repose.
- **Engine (v1 M7):** water rendering (waves, reflections, refraction, absorption, foam,
  underwater fog, caustics) plus ice.
- Minimal globe spawn picker for testing (finished in V2-15).
- *Accept:* `worldmap` geology/soil/deposit layers; statistical tests that deposits occur only in
  their provinces at plausible frequencies; each Era 0–5 resource reachable from every continent
  at Standard size, or lint flags the gap.

## V2-3 — Player: character, body & physiology
- **Engine (v1 M4):** `hearth_protocol`, integrated server thread (20 TPS), client mirror,
  physics/collision shared with entities, saves of players.
- **Engine (v1 M11 framework):** UI toolkit and text rendering (clean-room font), screens
  framework, options screens, controller support, localization plumbing, audio engine.
- Character creator with rotatable preview and profiles; realistic blocky rig; first-person
  arms/body; walk/jog/sprint/crouch/crawl/swim/climb, falls by impact speed.
- Physiology: energy and macronutrients, fresh-food reserve, hydration, food safety,
  thermoregulation (clo, wetness, wind, radiation), sleep with smooth time acceleration and
  interruptions, fatigue, localized injuries and illnesses, stamina; death modes (Legacy,
  Permadeath, Hardy).
- Diegetic HUD, Body panel (B), optional Guided HUD; body audio.
- *Accept:* headless physiology tests (hypothermia in 5 °C rain without clothing vs fur + fire;
  dehydration fatal after ~3 game days; sprain vs fracture recovery on the right time scale).

## V2-4 — Inventory, carrying & clothing
- Hands, body attachment points, containers with grids and limits, mass/volume/footprint,
  placing items in the world, dragging/rolling/travois, encumbrance, quick slots 1–6 + radial.
- Clothing layers with insulation, wind/water resistance, capacity; visible on the model.
- *Accept:* capacity and encumbrance tests; UI round-trip tests; a 100 kg log section can only be
  dragged, slowly.

## V2-5 — Interaction, process crafting & knowledge
- Gathering by hand; excavation with spoil piles and angle of repose; contextual actions.
- Process engine (inputs, tools by property, conditions, durations on the right scale, quality,
  by-products, failures); thermal model for fires and items; fire-making; cooking and
  preservation basics; tool condition and maintenance.
- Minigames (knapping first), skills, journal with discovery routes (experiment, observation,
  inference, evidence), Knowledge Modes; Era 0–2 knowledge and processes.
- *Accept:* a scripted bot goes from nothing to fire, a hafted stone spear, sewn hide clothing
  and dried meat using only discovery routes; no unreachable Era 0–2 nodes; effort rollup rises
  by era.

## V2-6 — Flora framework (temperate first)
- Plant model; procedural tree growth for the temperate species set (real heights/girths,
  branches as sub-block models, root flares); wood properties; passable foliage (slows, hides,
  partial shade); climbing; felling as a physical event, limbing, bucking; understory; edible/
  medicinal/toxic plants; succession; vegetation cell state; wildfire.
- **Engine (v1 M8):** LOD quadtree, fast surface generator, cache, meshing, seamless handoff,
  TAA option — LOD tiles show vegetation state and season.
- *Accept:* species silhouettes at 3 ages (screenshots); a cleared area goes through succession
  over simulated years; wildfire spreads and burns out plausibly in a dry-season test; v1 M8's
  horizon screenshot from a peak and fly-through benchmark.

## V2-7 — Fauna framework (temperate forest first)
- **Engine (v1 M9 infrastructure):** ECS integration, body plans with shared skeletons,
  procedural animation, pathfinding (walk/swim/fly/climb), instanced rendering, animal audio.
- Species coat textures; senses with wind-carried scent; utility AI behaviours; herds and
  packs; ecological cells with population dynamics; materialization/folding; biogeographic
  realms; predators and dangerous herbivores with real attack causes; tracking; hunting;
  butchering; Tier 1 temperate fauna; ambient birds and insects.
- *Accept:* 50-year headless run stays within plausible bounds for every species; heavy hunting
  depletes and later recovers a local deer population; every predator attack logs a realistic
  cause.

## V2-8 — Structural building & shelter
- Construction pieces and stages; incremental stability solver (support propagation + load
  check) with collapses and debris; excavation supports; roofs/rain/rot rules; shelter quality;
  builder's view; Era 0–3 techniques.
- *Accept:* a too-long stone span collapses; a timber-supported tunnel stands; thatch keeps rain
  out while a flat bark roof leaks; solver within budget on large structures.

## V2-9 — Vertical slice review
- Temperate forest year from a loincloth in spring; scripted bot run through a year with
  screenshots and logs; `docs/review/slice-1.md`; fix the top issues before expanding.

## V2-10 — Ecosystem expansion waves
- Boreal/tundra/polar → grassland/steppe/desert → savanna/tropical forest → mountains/alpine →
  wetlands/rivers/lakes → oceans (coasts, reefs, kelp, open ocean, deep sea bioluminescence).
- *Accept:* per-wave 50-year stability runs and screenshot suites.

## V2-11 — *Australopithecus* & the agent framework
- Agent framework (Body, Mind, KnowledgeState, Inventory, SocialGroup, Culture) using the same
  process system; hominin groups, tool use, tree nests, traces, habituation, observation
  learning; group-level abstract simulation.
- *Accept:* groups persist for decades in suitable habitat; a scripted observer gains knapping
  insight by watching.

## V2-12 — Neolithic
- Plant and animal domestication across generations, farming (soils, seasons, weeds, pests,
  irrigation, fallow, manure), pottery and kilns, spinning and weaving, permanent houses,
  querns and bread, dairy, storage, boats and sledges, the wheel.
- *Accept:* a bot domesticates a grain and sees yields rise; a sheep lineage becomes docile and
  woolly; under-fired pottery fails.

## V2-13 — Metallurgy & mining
- Prospecting, mining with supports, ore processing, charcoal, furnaces, bellows, crucibles,
  casting, alloying, bloomery, smithing, heat treatment; metal tools and armour.
- *Accept:* realistic yields; bronze needs copper and tin sources; iron needs a bloomery and
  forging; measurable tool quality differences.

## V2-14 — Late scope: Iron Age & Classical
- Lime mortar and concrete, arches/vaults/domes, cranes and pulleys, lathe, glassblowing, water
  wheel with a minimal mechanical power network, advanced boats and sails, carts with draft
  animals; `docs/design/future-systems.md` ready for Era 6.

## V2-15 — World creation & menus
- Full §16 flow: planet settings, life & time settings, era selector, globe spawn picker with
  region info, character selection; map with exploration memory.
- **Engine (v1 M11/M12 remainder):** remaining screens, resource packs with hot reload, WASM
  mod API + examples, `MODDING.md`.
- *Accept:* UI tests; spawning at chosen points across climates works.

## V2-16 — Long-run balance, performance & cohesion QA
- 100-year headless planet runs; §21 budgets and v1 frame-rate targets verified
  (`BENCHMARKS.md`); interaction matrix fully checked; screenshot suite across ecosystems,
  seasons and times of day; "survive two years in three climates" bot run;
  `docs/review/v2-final.md`.
- **Engine (v1 M13/M14):** profiling, zero steady-state allocations, software-adapter run,
  README/BUILDING/MODDING/ASSETS_LICENSES, fresh-clone build, soak test.
