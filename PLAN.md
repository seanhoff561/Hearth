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
- **M0–M3** (v1 engine); **V2-0 – V2-10** (content platform to ecosystem waves); **V2-12** (the
  Neolithic); **S0** (D222); **P0–P2** (D224–D226, D229). V2-11 and V2.1's H0–H10 superseded.
- **E0** — the human systems removed and archived (D230): Amendment E keeps the game to a lone
  player on a true Earth until Phase F plans people anew.
- **Audit 0** — the baseline (`docs/review/audits/AUDIT-0.md`, D231–D234).
- **Q1** — the interface's typefaces, panels and journal (`docs/design/interface.md`, D236).

## Audits (Amendment Q §8.2)
Each bounded to about a tenth of the work it covers: metrics and trend (Q §9),
`scripts/lean-check.sh` and a fresh-eyes review, the gate and a profile of the worst scene,
realism and cohesion against the reference ranges and real references, the repetition metrics,
a prioritized fix list (high first), `docs/review/audits/AUDIT-<n>.md` and five lines in
`PROGRESS.md`.

## From Audit 0 (medium priority; each when its file is next touched)
- Split the eight files over 2,000 lines along the cut lines in AUDIT-0 (`client.rs`,
  `workshop.rs`, `server.rs`'s `run` into a `ServerWorld`, `ecology.rs`, `live.rs`,
  `screenshot.rs`, `bench.rs`, the bot test).
- One `smoothstep`; debug tools (F3+T, the counting allocator) behind Developer mode; material
  statuses derived from use; planned knowledge cut to id, name and a line; Q §3's repetition
  checks in the screenshot suite (with S2).

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
  views; skipping a long action with the fade, interruptions,
  cancelling with partial progress kept, queued repeats (§6). Sleep by the two-process model with
  the lie-down menu; the time in words (§7).
- *Accept:* every process animates; lying down at night leads to sleep within a realistic time;
  every refusal explains itself; skipping gives what waiting gives.
- *Amended by E §9.1:* no skipping active work (Rest / Wait for waiting processes); sleep on
  real 24-hour days. Poses are mind-agnostic actions (the Actor rule, E §2.3).

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
  footsteps and footprints on smooth ground, the nav grid rebuilt for animals,
  built-piece skirts and Level ground (S §6, §8).
- *Accept:* S §13 collision and movement tests; animals path well over hills, scree and
  riverbanks; 300 animals within budget.

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
  landforms, veins and strata off the grid's axes (P §11.1–11.2); a
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
- *Accept:* herds stay within budgets; every item form renders for every material.
- *Amended by E §9.2:* animals and items; the human body is E7's. With the item meshes, the
  inventory's icons rendered from them under neutral light and cached (Q §6, D236).

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
- Play as the owner does, in all three modes: create a world and a character, choose where to
  begin, survive a season, sleep, make fire, hunt, build a shelter, die and begin a new life;
  spectate and build in Creative. A screenshot sequence (or video) a step; `docs/review/playability.md`
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

## Phase R (after V2-16; `dev/AMENDMENT_R.md` as amended by E §9.3)
Nothing of it starts before V2-16 is accepted; until then only the **multiplayer-ready rule**
(R §0.3, D166): gameplay state on the server's side, messages serializable and versioned, no
simulation assuming one player. Then: re-read the amendment, write `RELEASE_PLAN.md`, R1. Each R
milestone's acceptance is R §12's and is detailed in `RELEASE_PLAN.md`.

### Phase R-A — online features (R1 → R2 → R3 → R5; R4 and R6 move to Phase F)
- **R1 — Networking core:** `hearth_net`, QUIC, versioned protocol, the in-memory transport for
  single-player, server-authoritative intents, identity keys. *Accept:* no regression in the
  benchmarks through the network layer.
- **R2 — Replication and world sync:** seed plus deltas with hashes, interest management,
  prediction and reconciliation, lag compensation. *Accept:* 4 bots play an hour at 150 ms and
  2 % loss without desync.
- **R3 — Hosting, joining, administration:** Host & Play, dedicated server, LAN, invites, relay,
  roles, moderation, mod sync; starts and new lives by E §6.3 and §6.6; body interactions between
  players on the Actor rule. *Accept:* a 32-bot soak within budgets.
- **R5 — Proximity voice:** Opus, range gating, spatial audio with occlusion, whisper and shout
  as in-world noise. *Accept:* two players across a cave wall hear each other muffled.

### Phase R-B — release (after Audit 6)
- **R0 — Repository audit and open-source foundation** (licenses, `cargo deny`, clean-room and
  secret scans, the working files into `dev/`).
- **R7 — Packaging, CI and releases** (first-run wizard with a Profile step, update check).
- **R8 — README, guide, docs site and the Field Guide** (people, eras and AI "Coming later").
- **R9 — Branding, press kit and trailers** (players together where the people beat was).
- **R10 — Launch readiness review** (after Audit 7): `dev/LAUNCH_REPORT.md`, v0.1.0 drafted.

## Phase F — Simulated humanity (after R10; designed in E1, `docs/design/future/humanity/`)
F0 research and prototypes; F1 people foundation (the archive's genetics, life course and
demography adapted); F2 the History Engine; F3 the World Bible and the Historian; F4 procedural
minds; F5 the System 1 action model; F6 reflective minds and the AI Bridge (R4); F7 conversation
and voice (R6); F8 societies; F9 settlements; F10 births and childhood; F11 eras, the Upper
Paleolithic first; F12 scale, cost and safety. Acceptance detailed when Phase F begins; an audit
after every three.
