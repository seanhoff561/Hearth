# Plan

The game follows `docs/spec/v2-direction-change.md` (v2) over `docs/spec/v1-build-prompt.md` (v1),
as amended by Amendment S (`docs/spec/amendment-s-smooth-world.md`, smooth world), Amendment P
(`docs/spec/amendment-p-playability.md`, playability) and, since 2026-10-08, Amendments E and Q
(`docs/spec/amendments-e-q.md`): **Amendment E** (Earth-True: real Earth time and size, work
stroke by stroke, Wild Earth as an adult you design, the simulated humans removed and planned as
Phase F) and **Amendment Q** (the Quality Charter, above the per-amendment details). V2.1 and its
Addenda are superseded (archived in E0). The full plan as it stood before is
`docs/history/plan-2026-10-08.md`.

## Definition of done (every milestone)
- Implement; design docs in `docs/design/` current; data added; tests.
- `scripts/check.sh`, `hearth content lint` and the performance gate `scripts/perf-gate.sh` green
  (a regression over 5 % justified in `DECISIONS.md` and the baseline moved, D59).
- **The five tests (Q §1):** every change is *Real* (more like the real Earth, perceptibly, or
  needed for something that is), *Lean* (needed now, the simplest thing; nothing unused left),
  *Fast* (within its budget, measured), *Whole* (nothing out of place in scale, light, colour,
  detail, motion, sound or style) and *Organic* (no visible repetition; procedural first).
  Where realism and performance conflict: perceptual realism, the simplification and its error
  bound recorded in `DECISIONS.md`.
- **The milestone checklist (Q §8.1)** in its `PROGRESS.md` entry, one line each: Real? Lean?
  Fast? Whole? Organic? Any "no" fixed before the milestone closes, or a `DECISIONS.md` entry
  saying why not and when.
- `PROGRESS.md` (its status rows), `dev/PLAYTEST.md` updated; a commit.

## Order of work (Amendments E and Q, D227)
1. **P2** finished, building nothing E §2 removes (no summoning people, no inhabiting or being
   born again, no people overlays).
2. **E0** — remove the human systems.
3. **Audit 0**, then its high-priority fixes.
4. **Q1** — the new interface design.
5. **E1 → E2 → E3 → E4 → E5.**
6. **P3** → **P4 with E6** → **P5** → **P6** (as amended by E §9.1).
7. **Audit 1.**
8. **S1 → S2 → E7 → S3 → S4** (S4 as amended by E §9.2).
9. **Audit 2.**
10. **S5 with P7 → P7G → S6** (animals and items) **→ S7 → S8 → P8.**
11. **Audit 3.**
12. **V2-13 → V2-14** (technology only).
13. **Audit 4.**
14. **V2-15 → V2-16** (as amended by E §9.4).
15. **Audit 5.**
16. **Phase R-A: R1 → R2 → R3 → R5** (as amended by E §9.3).
17. **Audit 6.**
18. **Phase R-B: R0 → R7 → R8 → R9.**
19. **Audit 7.**
20. **R10.**
21. **Phase F**, an audit after every three milestones.

## Completed
- **M0–M3** — v1 engine: foundation, voxel core, world generation, near-field rendering.
- **V2-0 – V2-10** — content platform, time and seasons, geology and hydrology, the player's body,
  carrying and clothing, process crafting and knowledge, flora, fauna, structural building, the
  vertical slice review, ecosystem waves.
- **V2-11** — superseded by V2.1 (H0–H10), itself superseded by Amendment E.
- **H0 – H10** — the simulated humans (genetics to societies, history, the Observer, the
  conversation backend); removed in E0, archived as `archive/humans-v2.1`.
- **V2-12** — the Neolithic: pottery, fields and grain domestication, herds, textiles, timber,
  moving loads.
- **S0** — Baseline-S and the smooth-terrain prototypes (D222).
- **P0** — triage and quick fixes (D224, D225). **P1** — menus and world management (D226).
- **P2** — game modes and Creative (D229). **E0** — the human systems removed (D230): two crates,
  their modules, screens, protocol, data, schemas and tests, about 42,800 lines, because
  Amendment E keeps the game to a lone player on a true Earth until Phase F plans people anew;
  archived in `docs/archive/humans-v2.1/` and the branch `archive/humans-v2.1`.

## Audits (Amendment Q §8.2)
Each bounded to about a tenth of the work it covers: metrics and trend (Q §9),
`scripts/lean-check.sh` and a fresh-eyes review, the gate and a profile of the worst scene,
realism and cohesion against the reference ranges and real references, the repetition metrics,
a prioritized fix list (high first), `docs/review/audits/AUDIT-<n>.md` and five lines in
`PROGRESS.md`. **Audit 0** (after E0) also sets the baseline: the restart files cut down (Q §5.4),
a fluff inventory, the original-game-era leftovers with a plan each (Q §5.8),
`data/hearth/materials/reference.ron` started, `docs/design/budgets.md`, `scripts/lean-check.sh`.

## Q1 — Interface design (Q §6)
- An OFL typeface rendered as SDF/MSDF text; quiet panels in natural low-saturation colours;
  item icons rendered from the items' meshes; a lightly diegetic journal; accessibility and the
  layout test kept. Only `hearth_ui` and the screens.

## E1 — The future humanity plan (documents only, E §10)
- `docs/design/future/humanity/` (README, roadmap and one document per E §10 section), research
  notes; `docs/design/future-humanity.md` pointing there; Phase F below. No code.

## E2 — Controls (E §3)
- Any key bindable alone (Ctrl, Alt, left and right modifiers; releases fed to the rebind
  capture); hold and tap rules for a lone modifier with combinations; clicking with no target
  uses the item in hand or attacks (punch, swing, thrust, kick), physically.
- *Accept:* E §11's control tests; the owner's Ctrl/Alt report resolved in `dev/PLAYTEST.md`.

## E3 — Real Earth time (E §4)
- One clock: 86,400 s days, the 365.2422-day year, the 29.530589-day month, tilt 23.44°; the sky
  by real astronomy with the equation of time; every motion and process at its real rate;
  Sleep and Rest / Wait the only way time passes faster; start "Spring morning" or "Now".
- *Accept:* E §11's time tests; no compressed clock left; Rest / Wait works.

## E4 — Real Earth size (E §5)
- `PlanetSize::Earth` only (test planets in Developer mode); the generator recalibrated with
  nested refinement levels; real heights, lapse rate, tree and snow lines, altitude physiology;
  lazy simulation planet-wide; a far-field planet layer to the real horizon.
- *Accept:* E §11's Earth-scale tests; an Earth world made in ≤ ~45 s with a progress screen;
  performance targets met or honestly recorded.

## E5 — Wild Earth start (E §6)
- The character creator back; 3–5 suggested places with verified "what to look for" and "watch
  out for"; other eras "Coming soon"; waking at dawn; death: a new life or restart.
- *Accept:* E §11's start tests; title screen to waking at dawn; places add ≤ ~15 s.

## P3 — Looking and the hands
- Highlighting and precise picking, name tags and hand hints (§5.1); the intent resolver
  (`data/hearth/interaction/intents.ron`) with safe defaults, learned preferences, stowing to free
  a hand and hold-to-repeat (§5.2); the middle-click action menu (§5.3); feedback without
  clutter, the crosshair's offer list removed (§5.4); controller mapping.
- *Accept:* a scripted playtest covers every example of §5.2; nothing happens without a
  highlighted target or an action on oneself; picking costs under 0.2 ms a frame.
- *Amended by E §9.1:* clicking with no target follows E §3.2; hold-to-repeat becomes stroke by
  stroke work (E §7.2); no progress ring.

## P4 — Poses, animation, sleep and time (with E6)
- The work-pose library with IK; every process with its pose (lint); first- and third-person
  views; people using the same poses; skipping a long action with the fade, interruptions,
  cancelling with partial progress kept, queued repeats (§6). Sleep by the two-process model with
  the lie-down menu; the time in words (§7).
- *Accept:* every process animates; lying down at night leads to sleep within a realistic time;
  every refusal explains itself; skipping gives what waiting gives.
- *Amended by E §9.1:* no skipping active work (Rest / Wait for waiting processes); no people;
  sleep on real 24-hour days.

## E6 — Work the way the body does it (with P4, E §7)
- No timers or progress bars: active work stroke by stroke while the button is held, rates from
  real sources, work in progress persistent; gathering one thing at a time from a loose-objects
  layer and from living plants; waiting processes on real-rate state models, inspectable.
- *Accept:* E §11's work tests; every active process animates stroke by stroke; every waiting
  process can be inspected; no progress bars remain.

## P5 — Motion timing audit
- `docs/design/motion-timing.md`: everything that moves, its clock (real or game) and its speed,
  and the automated check, fast-forward included (§8).
- *Accept:* every animated visual listed and passing the check.

## P6 — Learn to play
- The tutorial's chapters and the bot that completes them, first-time hints, the basic Field
  Guide (§9).
- *Accept:* the bot completes every chapter; a new player can learn sleeping, drinking, fire and
  the hands unaided.
- *Amended by E §9.1:* no People chapter; teach stroke-by-stroke work, gathering one thing at a
  time, inspecting the meat over the fire, Rest / Wait; set in a Wild Earth valley at real time.

## Audit 1
After P6, covering E1–E6 and P3–P6.

## Amendment S — Smooth voxel world (S0–S8)
Amendment S (`docs/spec/amendment-s-smooth-world.md`, 2026-10-07) makes natural terrain smooth
on the same 1 m voxel grid (a fill value per voxel), leaves real foliage, trees real trunks, and
bodies and items smooth forms; every simulation system stays on the grid. It is engine work
inserted here, after V2-12 and before the remaining H milestones (S §0.2); Amendment R stays last.
`MIGRATION_SMOOTH.md` maps each subsystem to Keep / Modify / Replace; Baseline-S and the latest
numbers live in `BENCHMARKS.md`; `docs/review/smooth-world.md` is the visual review. Each S
milestone: implement, design docs and data, tests, `scripts/check.sh`, `hearth content lint`,
the benchmarks, `PROGRESS.md` (with its Smooth World Status row), commit.

## S1 — Fill data and editing core
- Fill values in cubes and generation (S §2), material properties (sharpness, angle of repose,
  friction, slump, walk sound, surface recipe), saves and migration, network delta format,
  raycast with sub-voxel hits, mass-conserving dig and place brushes, settling (S §8.3–8.4).
- *Accept:* S §13 data and editing tests pass; a dug pit and spoil pile settle realistically in
  a headless test.

## S2 — Smooth terrain rendering
- The production mesher (S §3.2), compact vertices, mid-range simplification, material blending,
  procedural PBR materials and compression (S §4.2), biplanar shading with height blending and
  anti-tiling, shader overlays (wetness, snow, moss, litter, scorch), sub-metre stratigraphy,
  voxel AO and trilinear light, material packs.
- *Accept:* S §13 meshing tests; the screenshot suite shows smooth terrain everywhere; frame
  targets met for terrain-only scenes.

## E7 — Realistic human body, face and hair (after S2, E §8)
- Anatomically realistic skinned bodies with morphs, skin with subsurface scattering and state
  from the body simulation, faces and eyes, card hair with anisotropic shading and physics and
  real growth, the loincloth fitted, first-person arms; the creator updated.
- *Accept:* the E7 review screenshots; a close-up character within about 1 ms of GPU at High.

## S3 — Movement, collision and navigation
- Field-based capsule collision on server and client, slope walking, sliding and footing,
  footsteps and footprints on smooth ground, the nav grid rebuilt for animals and humans,
  built-piece skirts and Level ground (S §6, §8).
- *Accept:* S §13 collision and movement tests; animals and humans path well over hills, scree
  and riverbanks; budgets met.

## S4 — Distant terrain
- Fixed-point LOD heights, smooth LOD meshing, matched shading and overlays, canopy shapes in
  LOD (S §5).
- *Accept:* no visible seam or color jump at the near/far transition in the screenshot suite;
  v1 §8.4 targets still pass.
- *Amended by E §9.2:* blended into the far-field planet layer out to the real horizon; measured
  on the Earth-sized planet.

## Audit 2
After S4, covering S1, S2, E7, S3, S4.

## S5 — Trees and foliage (with P7)
- Smooth trunks and branches, felling rigid bodies and log meshes, leaf clusters with
  translucency and wind, foliage occupancy and dappled light, seasons, the foliage LOD chain
  with impostors, ground cover seated and bendable (S §7).
- *Accept:* each species' silhouette screenshot set at three ages and four seasons; forest
  benchmark targets met.

## P7 — Natural generation without the grid (with S5)
- Shrubs and bushes grown as individuals of their species' forms, berries, flowers and thorns on
  their branches; every natural thing at real-valued positions and turns, spaced by
  competition, clustered by dispersal, in clonal patches, crowns merging into hedges,
  thickets and reed beds, foliage occupancy from the plants' real shapes; rocks, logs, trees,
  landforms, veins and strata and people's places off the grid's axes (P §11.1–11.2); a
  building grid of each structure's own origin and turn evaluated (§11.3).
- *Accept:* §11.4's statistical tests (spacing, clustering, cover, no position quantized to
  the grid, occupancy matching what is drawn) and screenshots (meadow, forest edge, thicket,
  reed bed, talus, river cobbles); the §11.3 evaluation recorded or built.

## P7G — Grasses and ground cover
- The sward (P §11.5) in place of `grass_block` and the grass plant blocks: a living layer on
  the soil, derived from climate, soil, light and season with only events' deviations stored;
  real grass species and growth forms for every zone; grazing, trampling, fire and the
  seasons; the GPU blade renderer and its distance chain; movement, hiding, forage and fuel from
  it; Creative painting; the migration of worlds and of everything that used the old blocks.
  It supersedes S §7.3 for grasses.
- *Accept:* §11.5.9's tests, screenshots and budgets.

## S6 — Bodies and objects
- Smooth procedural skinned bodies for every body plan, genetics-driven shape, expressive faces,
  coats, hair and fur, fitted clothing, body LODs and impostors; smooth item meshes from form ×
  material (S §10).
- *Accept:* a three-generation family screenshot set shows resemblance; herds and crowds stay
  within budgets; every item form renders for every material.
- *Amended by E §9.2:* animals and items; the human body is E7's.

## S7 — Water, snow, ice, caves and built-piece polish
- S §9 in full and S §6 visual polish.
- *Accept:* shoreline, waterfall, snowdrift, frozen lake, glacier and cavern screenshots
  reviewed; no gaps where buildings meet the ground.

## S8 — Performance and cohesion pass
- Optimize to the S §12.2 targets at every preset; complete `docs/review/smooth-world.md`
  (S §12.4); update docs, journal art and the Amendment R guide plan (S §11).
- *Accept:* all S §12 targets met or honestly documented for owner decision; the review shows
  one coherent world.

## P8 — Playability review
- Play as the owner does, in all three modes: create a world, choose a birthplace, be born, grow
  up, survive a season, sleep, make fire, hunt, build a shelter, die and go on; spectate and
  build in Creative. A screenshot sequence (or video) a step; `docs/review/playability.md`
  with what still confuses, drags or fiddles; the top items fixed.
- *Accept:* every issue in `dev/PLAYTEST.md` resolved or explained; the review's top items
  fixed.

## Audit 3
After P8, covering S5–S8.

## V2-13 — Metallurgy & mining
- Prospecting, mining with supports, ore processing, charcoal, furnaces, bellows, crucibles,
  casting, alloying, bloomery, smithing, heat treatment; metal tools and armour.
- *Accept:* realistic yields; bronze needs copper and tin sources; iron needs a bloomery and
  forging; measurable tool quality differences.
- Technology only (E §9.4): usable alone, in Creative and in multiplayer.

## V2-14 — Late scope: Iron Age & Classical
- Lime mortar and concrete, arches/vaults/domes, cranes and pulleys, lathe, glassblowing, water
  wheel with a minimal mechanical power network, advanced boats and sails, carts with draft
  animals; `docs/design/future-systems.md` ready for Era 6.
- Technology only (E §9.4).

## Audit 4
After V2-14.

## V2-15 — World creation & menus
- *Amended by E §6, §9.4:* Create World is name, seed, mode, era (Wild Earth only; the others
  "Coming soon") and starting date, then the suggested places, then the character creator.
- The map with exploration memory; **Engine (v1 M11/M12 remainder):** remaining screens, resource
  packs with hot reload, WASM mod API + examples, `MODDING.md`.
- *Accept:* UI tests; starting at chosen places across climates works.

## V2-16 — Long-run balance, performance & cohesion QA
- long-run headless planet runs at Earth scale and real time (E §9.4);
  §21 budgets and v1 frame-rate targets verified
  (`BENCHMARKS.md`); interaction matrix fully checked; screenshot suite across ecosystems,
  seasons and times of day; "survive two years in three climates" bot run;
  `docs/review/v2-final.md`.
- **Engine (v1 M13/M14):** profiling, zero steady-state allocations, software-adapter run,
  README/BUILDING/MODDING/ASSETS_LICENSES, fresh-clone build, soak test.

## Audit 5
After V2-16, before Phase R.

## Phase R (after the game is complete)
Amendment R (`dev/AMENDMENT_R.md`, 2026-10-03): open-source release, multiplayer, AI and voice,
the guide and the trailer (since Amendment S, describing and showing the smooth world). None of it starts until every milestone above, through V2-16,
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

*Amended by E §9.3:* Phase R-A is **R1 → R2 → R3 → R5**; R4 (AI Bridge) and R6 (agent voices)
move into Phase F. R3's births in multiplayer are removed: starts and new lives use E §6.3 and
§6.6 (suggested places, anywhere, near a friend), and R3 adds player-to-player body interactions
on the Actor rule, contact by consent, hostile ones by PvP. The guide's people, eras and AI
sections become "Coming later"; the trailer's people beat becomes players together. Audit 6
after R-A; Audit 7 before R10.

## Phase F — Simulated humanity (after R10; E §10, designed in E1)
Each milestone's acceptance tests are detailed when Phase F begins; an audit after every three.
- **F0 — Research and prototypes:** model latency, throughput and quality on reference hardware
  for System 1 and System 2; a five-person believability prototype; a voice-latency prototype.
- **F1 — People foundation:** the Person record and levels C0–C2 on the Actor framework; genetics,
  life course and demography restored from the archive, adapted to real time and
  childhood-as-the-past.
- **F2 — The History Engine:** region graph, settlements, polities, economy, conflict, culture,
  era snapshots, with plausibility tests.
- **F3 — The World Bible and the Historian:** narrative with fact validation and lazy zoom.
- **F4 — Procedural minds (C2):** needs, emotions, planner, routines, social rules, pathfinding at
  every scale.
- **F5 — The System 1 action model:** training pipeline, batched runtime, fallback.
- **F6 — Reflective minds (C3):** memory, reflection, structured outputs, validators, budgets; the
  AI Bridge providers.
- **F7 — Conversation and voice (C4):** turn-taking, streaming speech in and out, voices,
  languages and translation options, multiplayer.
- **F8 — Societies:** households, cooperation, institutions as collective agents, deliberation,
  markets, conflict and war.
- **F9 — Settlements:** people building with the real construction system; growth and roads.
- **F10 — Births and childhood:** any household, the vignette engine with the real family's
  minds, coming of age.
- **F11 — Eras:** the Upper Paleolithic vertical slice first, then the others, each with an
  authenticity review.
- **F12 — Scale, cost and safety hardening:** million-person regions, cost and latency budgets,
  red-team suites, multiplayer determinism through the decision journal.
