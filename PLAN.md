# Plan (v2, with V2.1)

The game follows `docs/spec/v2-direction-change.md` (v2), which overrides the original build
spec `docs/spec/v1-build-prompt.md` (v1) where they conflict. `MIGRATION.md` maps every v1
milestone and subsystem to its fate. Since 2026-10-03 the amendment
`docs/spec/v2.1-realistic-humans.md` (V2.1, with its Addenda: A, the player is born; B, births
in multiplayer and life after death) replaces
v2 §8.4 and §17 and milestone V2-11 with milestones H0–H13 (simulated people, from genes to
societies); `MIGRATION_HUMANS.md` maps what V2-11 had built onto them (D162). Each milestone ends with: design doc(s) in
`docs/design/` updated, data added, tests, `scripts/check.sh` and `hearth content lint` green,
the performance gate `scripts/perf-gate.sh` passed (or a regression over 5 % justified in
`DECISIONS.md` and the baseline moved with `--accept`, D59), `PROGRESS.md` (incl. the Content
Status table) updated, and a commit.

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
- **Engine (v1 M8, rest):** the quadtree, surface sampling, meshing, streaming and handoff exist
  (built in V2-2, D52); add the on-disk LOD cache, edits reaching the LOD, occlusion culling and
  a VRAM budget for LOD tiles, and a TAA option — LOD tiles show vegetation state and season.
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

## V2-11 — *Australopithecus* & the agent framework (superseded by V2.1)
- Parts (a)–(d) done and kept (`MIGRATION_HUMANS.md`): the population and its hunters, the agent
  crate on the player's body, knowledge, carrying and process engine, hominins drawn out near the
  player with their days, nests, alarms and flight, sites, habituation and learning by watching;
  its acceptance met (decades in suitable habitat; a scripted observer gains knapping insight).
  Part (e) (docs and the gate) folds into H0.

The H milestones each end as every milestone does (docs, data, tests, `scripts/check.sh`, the
content lint, the performance gate, `PROGRESS.md` with its **Humans Status** table, a commit).
*Australopithecus* keeps working throughout: it is the first species profile on the framework.

## H0 — Framework migration
- `MIGRATION_HUMANS.md`; the **Person** composition with versioned components (body, mind,
  knowledge, social, life history, possessions now; genome, phenotype and psyche joining with
  H1–H2); **species profiles** in `data/hearth/humans/species/` (*Australopithecus*
  implemented; *Homo erectus*, *H. neanderthalensis*, *H. sapiens* as data); one random stream
  per person; a step that reads the state before it, so neither order nor thread count changes
  the outcome; the **persons registry**, saved with the world under a version with migrations;
  the developer **inspector** (F3 mode) for any person; *Australopithecus* ported onto it.
- *Accept:* *Australopithecus* behave at least as well as before (the V2-11 tests on the new
  framework); save/load round-trips persons; the determinism test (same seed and inputs, and one
  thread or many, give the same persons after a run) passes.

## H1 — Genetics engine
- Genome (23 chromosome pairs, a few hundred loci), meiosis with crossovers, mutation,
  inbreeding, lineage and kinship coefficients; genotype → phenotype for appearance, health,
  temperament (personality model chosen in `DECISIONS.md`) and aptitude; heritability
  calibration; the ancestry lint (ground rule 1); phenotype-driven figures, so families look
  related.
- **The player's genome is a child's of two parents** (Addendum A): the character creator loses
  its appearance editor (name and sex stay); until families live in the world (H3) the parents
  are drawn from the spawn region's gene pool and shown.
- *Accept:* V2.1 §18's genetics tests; family resemblance visible in a generated
  three-generation family screenshot set.

## H2 — Psyche and mind core
- Personality and its behaviour mappings, appraisal emotions, mood and stress, values;
  perception; episodic and semantic memory, beliefs; routines, utility selection and HTN
  planning on the process engine; budgets.
- *Accept:* agents complete multi-step plans (a hafted spear from scratch, knowing how); modest
  trait–behaviour correlations; emotions visible and contagious.

## H3 — Life course and demography
- Pair bonds (abstracted), pregnancy, birth, the life stages from infant to elder with child body
  models and animations, development, ageing, death, mourning and inheritance; life tables.
- **The player is born** (Addendum A, `docs/design/humans/player-birth.md`): into a family of the
  world — in Wild Earth one of its few wandering families (D164) — growing up at the childhood
  pace through moments and the years between, and coming of age.
- **Death** (Addendum B §2, `docs/design/humans/life-after-death.md`, D167): the death as an event
  of the world, the death screen and the life story; **Spectate** (the Observer's entry: a free
  camera following any person or animal, its controls with H9); **Restart** (replay the world or
  start a new one, the old save archived); **Inhabit an adult** (body, knowledge, relationships
  taken over; the "Who you are" briefing).
- *Accept:* a 200-year forager run meets V2.1 §14.3's targets; children visibly learn by
  imitation and play; families persist across generations; a scripted player is born, grows up
  through its moments and comes of age in its family; a scripted player dies, reads its life
  story and goes on as a grown kinsman.

## H4 — Social systems
- Kinship systems, households, relationships and obligations, cooperation and sharing norms,
  reputation and gossip, norms and sanctions, status and group decisions, conflict escalation
  and de-escalation, strangers.
- *Accept:* V2.1 §18's social tests; a group debates and decides where to move camp; a norm
  violation produces gossip and sanctions.

## H5 — Culture and language
- The culture generator and model, transmission and evolution; generated languages, families and
  drift, names; speech acts, gestures, subtitles with partial translation; the player learning a
  language.
- *Accept:* two cultures from one ancestor diverge after a split with related languages and
  customs; the player learns a language over play.

## H6 — Knowledge and social learning
- Observation (generalized from the hominins'), teaching, apprenticeship, storytelling, agents'
  own discoveries, the collective brain's retention and loss, diffusion between groups, the
  anachronism guard.
- *Accept:* knowledge is lost in an isolated small population and kept in a large connected one;
  the player is taught a technique faster than discovering it.

## H7 — Tiers and persistence
- Household and demographic tiers, promotion and demotion conserving state, individuals
  instantiated from populations with synthesized genealogies, persistent persons, pruning to
  genealogy stubs, budgets.
- *Accept:* leaving a band and coming back finds it consistent; a population instantiated on
  approach has coherent families; budgets met.

## H8 — History simulation and Paleolithic eras
- The deep-time layer (dispersal from the cradle over the planet's real geography, gene pools,
  cultures, languages, knowledge geography, the chronicle) and the recent-history layer about the
  spawn; species profiles of *H. erectus*, Neanderthals and *H. sapiens* implemented; Lower,
  Middle and Upper Paleolithic era profiles (routines, camps, seasonal rounds, aggregation); the
  era selector enables them; **birth options**: two to four households of the chosen area to be
  born into (Addendum A); after death **be born again** and **inhabit a child** (its childhood
  from its age), with the eligibility rules and scope filters; Wild Earth's families as many as
  the players a world expects (Addendum B).
- *Accept:* `docs/review/era-*.md` reviews for the three eras; dispersal plausible on the
  planet's geography; births offered at places across the eras.

## H9 — Observer mode and the player in society
- The Chronicle mode (time controls, overlays, following people, stepping in as a birth into a
  chosen household); obligations, family and children, joining another group; the interaction UI
  (speech-act wheel, gestures, give and trade, asking to be taught, teaching).
- **Life after death in full** (Addendum B §2): the inhabiting flow (filters, consent prompts, the
  "not right now" rules), the **Knowledge after death** setting, the **Permadeath** preset, v2's
  death rules retired with a save migration (D167).
- *Accept:* a scripted player, born into a band, is taught, forms a family and goes on as their
  grown child after death; the Observer's fast-forward meets its budget.

## H10 — Optional conversation backend
- The `ConversationBackend` trait (none / local / remote, off by default), prompts built from a
  person's own state, player free text parsed into speech acts, anachronism and knowledge
  filters.
- *Accept:* disabled, nothing changes; enabled, a conversation suite never leaks unknown facts or
  anachronisms and never changes state outside speech acts.

## V2-12 — Neolithic
- Plant and animal domestication across generations, farming (soils, seasons, weeds, pests,
  irrigation, fallow, manure), pottery and kilns, spinning and weaving, permanent houses,
  querns and bread, dairy, storage, boats and sledges, the wheel.
- *Accept:* a bot domesticates a grain and sees yields rise; a sheep lineage becomes docile and
  woolly; under-fired pottery fails.

## H11 — Neolithic society
- Villages, farming and herding households, lineages, storage, feasting, early inequality, crowd
  diseases; the Neolithic era profile.

## V2-13 — Metallurgy & mining
- Prospecting, mining with supports, ore processing, charcoal, furnaces, bellows, crucibles,
  casting, alloying, bloomery, smithing, heat treatment; metal tools and armour.
- *Accept:* realistic yields; bronze needs copper and tin sources; iron needs a bloomery and
  forging; measurable tool quality differences.

## H12 — Bronze Age society
- Specialists, exchange and markets, chiefdoms and early states, temples, towns, organized
  conflict (abstracted, ground rule 4); the Bronze Age era profile.

## V2-14 — Late scope: Iron Age & Classical
- Lime mortar and concrete, arches/vaults/domes, cranes and pulleys, lathe, glassblowing, water
  wheel with a minimal mechanical power network, advanced boats and sails, carts with draft
  animals; `docs/design/future-systems.md` ready for Era 6.

## H13 — Iron Age / Classical society
- Kingdoms and city-states, coinage, law, cities; the era profile.

## V2-15 — World creation & menus
- Full §16 flow: planet settings, life & time settings, era selector with the deep-time
  progress, globe spawn picker with region info, the birth options (Addendum A; no appearance
  editor), the Observer's entry; map with exploration memory.
- **Engine (v1 M11/M12 remainder):** remaining screens, resource packs with hot reload, WASM
  mod API + examples, `MODDING.md`.
- *Accept:* UI tests; spawning at chosen points across climates works.

## V2-16 — Long-run balance, performance & cohesion QA
- 100-year headless planet runs and 500-year planet history runs with the era reviews (V2.1);
  §21 budgets and v1 frame-rate targets verified
  (`BENCHMARKS.md`); interaction matrix fully checked; screenshot suite across ecosystems,
  seasons and times of day; "survive two years in three climates" bot run;
  `docs/review/v2-final.md`.
- **Engine (v1 M13/M14):** profiling, zero steady-state allocations, software-adapter run,
  README/BUILDING/MODDING/ASSETS_LICENSES, fresh-clone build, soak test.

## Phase R (after the game is complete)
Amendment R (`dev/AMENDMENT_R.md`, 2026-10-03): open-source release, multiplayer, AI and voice,
the guide and the trailer. None of it starts until every H and V2 milestone above, through V2-16,
is complete and accepted (R §0.1). Until then only its **multiplayer-ready rule** applies (R §0.3,
D166): gameplay state on the server's side, new messages serializable and versioned through the
channel, no simulation assuming a single player. When V2-16 is done: re-read the amendment, write
`RELEASE_PLAN.md`, then R1. Each R milestone: implement, document, test, all checks,
`PROGRESS.md` (with its Release Status table), commit.

### Phase R-A — online features
- **R1 — Networking core.** `hearth_net`: QUIC, protocol and versioning, the in-memory transport
  for single-player, server-authoritative intents, handshake and identity keys. *Accept:*
  single-player runs through the network layer with no regressions in the v1/v2 benchmarks.
- **R2 — Replication and world sync.** Seed-plus-deltas cube sync with hashes, LOD updates,
  interest management, snapshots and deltas, prediction and reconciliation, lag compensation,
  replicated processes, plants, animals and people. *Accept:* 4 bot clients play an hour under
  150 ms latency and 2 % loss without desyncs; controls feel immediate.
- **R3 — Hosting, joining and administration.** Host & Play, dedicated server and Docker image,
  `server.toml`, LAN discovery, UPnP, invite codes, relay, community list protocol, roles and the
  admin panel, block/mute/report, PvP and sleep settings, mod sync; **births and deaths in
  multiplayer** (Addendum B, `docs/design/humans/multiplayer-births.md`, D168): the shared start
  lobby and shared childhood, Remembered Childhood, family links between players with consent, a
  player pair's child invitations, the death choices and spectators, the birth settings, logging
  off (resting, or living on quietly).
  *Accept:* a 32-bot soak on a dedicated server meets budgets; join flows work across platforms.
- **R4 — AI Bridge.** Providers, capability levels, the Agent Bridge (WebSocket + MCP) with
  examples, guardrails, decision journal, budgets and fallbacks, AI settings and wizard step.
  *Accept:* R §12's AI tests; a local-model setup in under five minutes by the guide; the game
  identical with AI off.
- **R5 — Proximity voice.** Capture, DSP, Opus, transport, server range gating, spatial audio with
  occlusion and reverb, whisper and shout as in-world noise, controls, indicators, settings.
  *Accept:* R §12's voice tests; two players across a cave wall hear each other muffled; a shout
  scares a nearby herd.
- **R6 — Voice for agents.** Local and hosted speech-to-text, speech to speech acts gated by
  language knowledge, `VoiceBackend` (vocalizations, local phoneme TTS speaking generated
  languages, hosted TTS), stable per-person voices with emotional prosody, client-side synthesis.
  *Accept:* talking to an agent by voice end to end with local models only; relatives sound
  alike.

### Phase R-B — release (only after R-A passes)
- **R0 — Repository audit and open-source foundation.** R §1.1–1.2, §1.4: licenses, `cargo
  deny`/`cargo about`, secrets and clean-room scans, `xtask`, devcontainer, `scripts/rename`,
  `RELEASE_PLAN.md`; the working files (`PROGRESS.md`, `PLAN.md`, `MIGRATION*.md`,
  `DECISIONS.md`) into `dev/`. *Accept:* a fresh clone builds and runs on all three platforms in
  CI; no secrets or foreign trademarks; licenses complete.
- **R7 — Packaging, CI and releases.** R §1.3, §1.5 (first-run wizard — with a Profile step,
  name and identity key, where a character creator was: Addendum B — diagnostics), the update
  check, measured system requirements. *Accept:* a test tag produces every artifact; each
  installs and runs on a clean VM per OS; Releases page to playing in under five minutes.
- **R8 — README, guide, docs site and in-game Field Guide.** R §9 in full, with generated tables,
  feature anchors and readability checks; birth, childhood, the death choices and births in
  multiplayer told plainly (Addendum B). *Accept:* every docs check passes; every implemented
  system has a guide section; nothing unimplemented described as available.
- **R9 — Branding, press kit and trailers.** R §8 and §10: the cinematic system, every trailer
  cut (a birth or childhood moment in the launch trailer's people beat if it reads well:
  Addendum B), thumbnail, descriptions, the review loop. *Accept:* trailers rendered and reviewed;
  `trailer/REVIEW.md` complete; licenses recorded.
- **R10 — Launch readiness review.** Fresh-machine installs, the guide followed literally,
  security and privacy reviews, a performance re-check, `dev/LAUNCH_REPORT.md` with the owner's
  decisions still to confirm (R §0.5); the v0.1.0 release drafted, not published.
