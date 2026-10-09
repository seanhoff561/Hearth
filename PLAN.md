# Plan

The game follows `docs/spec/v2-direction-change.md` (v2) over `docs/spec/v1-build-prompt.md` (v1),
as amended by Amendment S (`docs/spec/amendment-s-smooth-world.md`, smooth world), Amendment P
(`docs/spec/amendment-p-playability.md`, playability) and, since 2026-10-08, Amendments E and Q
(`docs/spec/amendments-e-q.md`): **Amendment E** (Earth-True: real Earth time and size, work
stroke by stroke, Wild Earth as an adult you design, the simulated humans removed and planned as
Phase F) and **Amendment Q** (the Quality Charter, above the per-amendment details), and since
2026-10-09 **Amendment T** (`docs/spec/amendment-t-true-to-earth.md`, True to Earth: the world
real at every scale, from continents to the pebbles underfoot, judged by evidence; rewriting
cube-era code allowed where it serves that, T §5). V2.1 and its Addenda are superseded
(archived in E0). The full plan as it stood before is `docs/history/plan-2026-10-08.md`.

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
6. **P3** → **P4 with E6** → **P5**. (P6, Learn to Play, is removed: D258.)
7. **Audit 1** (done).
8. **S1 → S2 → E7 → S3 → S4** (S4 as amended by E §9.2) (done).
9. **Audit 2** (done).
10. **E4.1 — Earth-scale performance** (the owner's fix pass, done 2026-10-09).
11. **Amendment T** (D291): **T0**, S5/P7 stopped at (b) (done) → **T1**, quick wins → **T2**,
    the realism gap analysis, which revises what follows.
12. By default after T2: **S5/P7 (c)–(f) with G1** (the ground at walking scale) → **W1**
    (water; S7's water) → **L1** (seamless distance; trees in the LOD band) → **P7G** → **S6
    with H1** → **R1** (rendering realism; earlier if T2 says) → **S7 → S8 → P8.** An audit
    after every three milestones (Q §8.2), each updating the realism scorecard (T §3.6), the
    first with a check that no coarse-scale caller has crept into the fine generator.
13. **V2-13 → V2-14** (technology only).
14. **Audit.**
15. **V2-15 → V2-16** (as amended by E §9.4).
16. **Audit.**
17. **Phase R-A: R1 → R2 → R3 → R5** (as amended by E §9.3).
18. **Audit.**
19. **Phase R-B: R0 → R7 → R8 → R9.**
20. **Audit.**
21. **R10.**
22. **Phase F**, an audit after every three milestones.

## From E4.1 (open; E4.1 done 2026-10-09, D279–D289, `docs/spec/e4.1-earth-scale-performance.md`)
- The owner's laptop meets §4.4's targets (`BENCHMARKS.md`: the whole render distance in 4.8 s,
  the creator's full detail in 0.24 s); the perf gate's Earth entries from the next baseline
  there.
- The scene's pipelines made while the world loads, off the main thread: the first world after
  an update holds the loading screen 1.4 s (PLAYTEST 33).
- Whether Windows' maths make the same planet as Linux's: `cargo test -p hearth_worldgen --test
  planet_pinned` on the owner's machine (the globe's map differs by some 150 texels, D289); if
  not, portable maths (the `libm` crate) in the planet's generation.
- §4.8's SIMD inner loops and GPU compute for the refinement tiles: not needed for E4.1's
  targets once the tiles left the interactive paths (D279) and are kept on disk (D284); S8's
  pass profiles tile building with Audit 2's hotspots.
- Past a pole edge the land beyond is not drawn as the pole's far side until the player crosses
  (D285).
- The distant tiles refine in place without a fade (D288).


## Completed
M0–M3 (v1 engine); V2-0 – V2-10; V2-12 (the Neolithic); S0 (D222); P0–P2; E0 (the human
systems archived, D230); Audit 0; Q1; E1–E6; P3–P5 (P6 removed, D258); Audit 1; S1; S2; E7; S3;
S4; Audit 2; E4.1.
What each did: `PROGRESS.md`, `docs/history/` and the design docs. V2-11 and V2.1's H0–H10
superseded.

## Audits (Amendment Q §8.2)
Each bounded to about a tenth of the work it covers: metrics and trend (Q §9),
`scripts/lean-check.sh` and a fresh-eyes review, the gate and a profile of the worst scene,
realism and cohesion against the reference ranges and real references, the repetition metrics,
a prioritized fix list (high first), `docs/review/audits/AUDIT-<n>.md` and five lines in
`PROGRESS.md`.

## Older open items
From Audit 0, E3–E5, P3–P5 and Audit 1, each when its file is next touched:
`docs/history/plan-open-items-2026-10-08.md`.

## Amendment S — Smooth voxel world (S0–S8)
Amendment S (`docs/spec/amendment-s-smooth-world.md`, 2026-10-07) makes natural terrain smooth
on the same 1 m voxel grid (a fill value per voxel), leaves real foliage, trees real trunks, and
bodies and items smooth forms; every simulation system stays on the grid. It is engine work
inserted here, after V2-12 and before the remaining H milestones (S §0.2); Amendment R stays last.
`MIGRATION_SMOOTH.md` maps each subsystem to Keep / Modify / Replace; Baseline-S and the latest
numbers live in `BENCHMARKS.md`; `docs/review/smooth-world.md` is the visual review. Each S
milestone: implement, design docs and data, tests, `scripts/check.sh`, `hearth content lint`,
the benchmarks, `PROGRESS.md` (with its Smooth World Status row), commit.

## From S1 (open)
- Building pieces, tilling and the structure check read the fill (S3's Level ground, a built
  piece's buried skirt); collision against the field is S3's.
- A fresh cut's faces holding sharper than soil's sharpness, weathering soft (S0's note).
- The bot suite (it digs) with the full suite.

## From S2 (open)
- The client's soft preview of what a dig will take.
- The smooth ground occlusion-culled on the GPU (it is frustum and cave culled, and drawn first
  so its depth culls what lies behind it); `meshopt` vertex-cache ordering; mid-range cubes
  meshed from a field at half resolution.
- Materials: a normal and roughness from the noise (the surface shades with the mesh's normal
  only), specular for wet and icy ground, and moss, leaf litter, scorch and frost overlays.
  Material packs are data packs overriding `materials/` (no hot reload yet).
- The debug views (wireframe, normals, weights, fill slices, the developer's blocky view).
- Frame times for terrain-only scenes on the PC (`scripts/baseline-s.sh`).
- The stony shore's shot place (`tools/shots/s0_baseline.shots`): E4 moved the coast; the
  camera stands in the ground there.
- Snow as fill (S7: the near snow lies under the smooth surface); plants, trees and loose
  stones (S5–S6).

## From E7 (open; E7 done 2026-10-08, D266–D270, `docs/design/people.md`)
- Faces: morph targets (same topology frame to frame) for expressions driven by the body (pain,
  cold, exertion, exhaustion, fear) and for lip sync (R §5.1); teeth and tongue.
- Skin: detail normals (pores, creases at joints, lines with age); the sculpture's finer forms
  (collarbones, ribs, tendons, knuckles) checked close up; breathing and weight shifts.
- Hair: alpha-to-coverage or dithered coverage under TAA, self-shadowing, translucency by
  thickness; wet hair clumping; growth in real time (about 1 cm a month, stubble in days) and
  cutting or tying back as an action; fine body hair; the coily styles' volume.
- Garments: light cloth simulation for the flaps; the other garments as fitted meshes with S6's
  items; mud and blood on garments.
- Feet: IK on uneven ground with S3's collision; first-person arms doing the work closer (P §6,
  §7) and motion-matched locomotion.
- Far away: impostors beyond some 60 m (the 24 mm mesh is drawn to any distance now); other
  players' bodies with multiplayer (R).
- The GPU budget, a close-up person within about 1 ms at High (some 135,000 skinned triangles,
  10,000 hair-card vertices and the eyes), measured on the owner's PC; meshing a body in about
  0.7 s (dev-opt build) on a worker thread.

## From S3 (open; S3 done 2026-10-08, D271–D273)
- The grip by wetness (the ground families' wet friction) and footwear; loose footing slipping
  now and then on scree and sand, and moving disturbing it (S §8.2, §8.4); cutting steps.
- The player's footprints (as the animals' signs); dust kicked up.
- Animals' capsules against the field near players; trees' skeleton capsules (S5).
- Level ground's footprint chosen (a 3 m square now), and its preview with the dig preview.
- The budgets (300 animals, movement substeps) measured on the PC.

## From S4 (open; S4 done 2026-10-08, D274–D278)
- The v1 §8.4 frame targets and the LOD's GPU time with the height fields (2,304 triangles a
  tile, ground and canopy frustum-culled only) measured on the PC; fewer triangles for flat
  tiles if it needs them.
- Overhangs as smooth shelves (S §5); the crown boxes' snow by the seasons; impostors between
  boxes and canopy (S5).

## From Audit 2 (`docs/review/audits/AUDIT-2.md`)
- Step heights scaled by body size; `Ground::friction` (a slipperiness) replaced by a real
  friction coefficient with a source for the slope fit.
- Duplicates: LOD normal packing, the seasonal range, two field gradients, the beard's lip
  zone; a `cover()` accessor for the canopy; `level`'s reach documented.

## T1 — Quick wins (T §2)
- Scavenger birds over fresh remains, seen and heard (and as dots on the horizon), in place of
  the repeating text; every world-describing notice reviewed (show it, don't say it, T §0.2).
- Skin calibrated against measured reflectance: rougher, regional, micro-detailed, occluded,
  subsurface-scattered, lit by the sky, a film when wet.
- Aim at real things and their parts (a berry cluster, a branch, a stone, a dig patch, a water
  point, a body part), picked against their shapes, highlighted from their own geometry; the box
  edges removed.
- The globe in relief: 4096 × 2048, hillshading, hypsometric tints, bathymetry, rivers, ice.
- *Accept:* T §2's.

## T2 — The realism gap analysis (T §3)
- Evidence at every scale against photographs, real elevation data and field statistics; the
  fine-detail regression (Standard against Earth); process against appearance per scale.
- *Output:* `docs/review/realism/RGA-1.md`, the ranked gap table, the realism suite (shot
  specs, references and licenses, metrics script), `scorecard.md`, this plan revised (D-entry).

## G1, W1, L1, R1, H1 (T §4; refined by T2)
- **G1:** physically based ground below ~38 m (diffusion, rills, colluvium, talus, tree-throw,
  frost, aeolian, karst, glacial forms) or example-based or learned detail, chosen by terrain
  statistics; clasts by lithology and sorting; soil surfaces; with S5/P7 (c)–(f).
- **W1:** water as continuous surfaces from the hydrology (hydraulic geometry, beds and banks
  carved to hold them), a shallow-water simulation near players, flow maps; block water removed.
- **L1:** no seam where full detail ends: measured, then the first rings shaded as the near
  ground, geomorphing, trees as instances then impostors, distant water as near.
- **R1:** indirect light, occlusion, translucency, calibrated materials, by realism per ms.
- **H1:** the body to the owner's reference bar (T §4.5) from anthropometric data and openly
  licensed bases, with S6's animals and items.

## S5 — Trees and foliage (with P7)
- Smooth trunks and branches, felling rigid bodies and log meshes, leaf clusters with
  translucency and wind, foliage occupancy and dappled light, seasons, the foliage LOD chain
  with impostors, ground cover seated and bendable (S §7).
- *Accept:* each species' silhouette screenshot set at three ages and four seasons; forest
  benchmark targets met.
- From (b), the sheets: spring's fresh green (it is summer's), the scale-leaved sprays
  (tamarisk, saxaul) finer, the crowns self-shadowed.

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

## Phase F — Simulated humanity (after R10; `docs/design/future/humanity/roadmap.md`)
F0 research and prototypes; F1 people foundation (the archive's genetics, life course and
demography adapted); F2 the History Engine; F3 the World Bible and the Historian; F4 procedural
minds; F5 the System 1 action model; F6 reflective minds and the AI Bridge (R4); F7 conversation
and voice (R6); F8 societies; F9 settlements; F10 births and childhood; F11 eras, the Upper
Paleolithic first; F12 scale, cost and safety. Acceptance detailed when Phase F begins; an audit
after every three.
