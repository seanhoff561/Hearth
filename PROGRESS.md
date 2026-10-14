# Progress

Current status only (Amendment Q §5.4). The full record of M0 to E0 (2026-09-30 to 2026-10-08)
is `docs/history/progress-2026-09-30-to-10-08.md` (then `progress-2026-10-08-e0-to-e5.md`,
`progress-2026-10-08-p3-to-s2.md` and `progress-2026-10-08-e7-to-audit-2.md`); decisions are
indexed in `DECISIONS.md`
with their texts in `docs/decisions/`.

Direction: v2 (`docs/spec/v2-direction-change.md`) as amended by S (smooth world), P
(playability) and, above them, **E and Q** (`docs/spec/amendments-e-q.md`, D227): Earth-True
and the Quality Charter. V2.1's simulated humans were removed in E0 and archived
(`docs/archive/humans-v2.1/`, D230); Phase F plans them anew. Amendment R waits until V2-16.
Since 2026-10-09 **Amendment T** (`docs/spec/amendment-t-true-to-earth.md`, D291): true to Earth
at every scale, judged by evidence; T0 → T1 → T2, then the plan T2 revises.

## Resume
1. Read this file, `PLAN.md` and the `DECISIONS.md` index; `git log --oneline -20`. Open
   `docs/decisions/`, `docs/history/` or a design doc only when a task needs it.
2. Run `scripts/check.sh` (it runs `scripts/lean-check.sh`); the long runs are
   `scripts/soak.sh`, at audits (D235).
3. Work in `PLAN.md`'s order; each milestone ends with its five-test checklist (Q §8.1) below.
   Check the realism scorecard's open gaps (`docs/review/realism/scorecard.md`, once T2 has made
   it) before choosing the next task (T §9).
4. Builds: `cargo test --profile dev-opt` (no LTO, D233); `--release` is fat LTO, for the game
   and the perf gate. A bare `cargo run` is the game's (the workspace's default member); the
   tools are `-p bench`. The cloud machine renders only on a software device: frame rates, the
   perf gate and how screenshots look need the owner's PC.

## Milestones
Done: M0–M3 (v1 engine), V2-0 – V2-10, V2-12 (the Neolithic), H0–H10 (removed in E0), S0,
P0, P1, P2, E0, Audit 0, Q1, E1, E2, E3, E4, E5, P3, P4, E6, P5, S1, S2, E7, S3, S4, Audit 2,
E4.1, T0, T1, T2, R1a, G1a. V2-11 superseded.

| Next, in order | State |
|---|---|
| Audit 0, then its high-priority fixes | done 2026-10-08 (D231–D234) |
| Q1 — interface design | done 2026-10-08 (D236) |
| E1 — the humanity plan | done 2026-10-08 (documents only) |
| E2 — controls | done 2026-10-08 (D237–D239) |
| E3 — Earth's clock | done 2026-10-08 (D240–D243) |
| E4 — Earth's size | done 2026-10-08 (D244–D248) |
| E5 — Wild Earth start | done 2026-10-08 (D249–D251) |
| P3 — looking and the hands | done 2026-10-08 (D252–D254) |
| P4 with E6 — work by the body, sleep | done 2026-10-08 (D255–D257) |
| P5 | done 2026-10-08 (P6 removed, D258) |
| Audit 1 | done 2026-10-08 |
| S1 — fill data and editing | done 2026-10-08 (D260–D262) |
| S2 — smooth terrain drawn | done 2026-10-08 (D263–D265) |
| E7 — realistic people | done 2026-10-08 (D266–D270) |
| S3 — moving on the smooth ground | done 2026-10-08 (D271–D273) |
| S4 — the distant terrain smooth | done 2026-10-08 (D274–D278) |
| Audit 2 | done 2026-10-08 |
| E4.1 — Earth-scale performance (the owner's fix pass) | done 2026-10-09 (D279–D289) |
| T0 — S5/P7 stopped at (b) | done 2026-10-09 (D290, D291) |
| T1 — quick wins (ravens, skin, highlight, globe) | done 2026-10-09 (D292–D295) |
| T2 — the realism gap analysis | done 2026-10-09 (D296, D297) |
| R1a — the sun's shadows | done 2026-10-09 (D298) |
| G1a — relief from 30 m to 30 km | done 2026-10-09 (D299) |
| S5/P7 (c)–(f) with G1b → W1 → L1 → P7G → S6 with H1 → R1b → S7 → S8 → P8 | S5/P7 (c) done 2026-10-14 (D300); G1b's ground done 2026-10-14 (D301); next: S5/P7 (d)–(f); Audit 3 after them (D297) |
| V2-13 → V2-14, Audit 4; V2-15 → V2-16, Audit 5 | planned |
| Phase R-A, Audit 6; R-B, Audit 7; R10; Phase F | planned |

| Row | State |
|---|---|
| Smooth world (S) | S4 done: the distant ground smooth height fields in fixed point, shaded with the near ground's materials and seasons, canopies over far stands, out to the real horizon. S3 done: movement on the field. S2 done: natural ground meshed smooth on the server and drawn with blended procedural materials, wet and snow overlays (0.28–0.87 ms a surface cube, as the blocks); frame targets need the PC. S1 done: fill in every surface cube, generated and saved; ground families; dig, pile and settle conserving volume (sand to 34.3°). S0 done (D222: Surface Nets with sharp features, biplanar shading); Baseline-S's CPU half recorded, its GPU half needs the PC (`scripts/baseline-s.sh`); prototype mesher 3,553 surface cubes/s on one thread (target 2,000 on eight) |
| Playability (P) | P0–P5 done (P6 removed); open issues in `dev/PLAYTEST.md` |
| Earth-True (E) | E0–E7 done 2026-10-08; E4.1 (the Earth-sized world fast to see, make a person on and load into) done 2026-10-09 |
| Quality (Q) | Audit 0 done 2026-10-08 (`docs/review/audits/AUDIT-0.md`); open high-priority findings: none; Audit 1 done 2026-10-08 (`AUDIT-1.md`); Audit 2 done 2026-10-08 (`AUDIT-2.md`); next: an audit after three completed milestones (Q §8.2) |
| Realism (T) | last suite render: R1a, 2026-10-09 (`docs/review/realism/`); top open gaps (`scorecard.md`): grass and water as sprites and blocks, rivers' banks below their water, trees past the render distance as boxes; the grid's lakes three times the Earth's; the seam at the edge of full detail; the desert's form. G1a brought the relief from 30 m to 30 km within the Earth's bounds, G1b the walking scale (D301); S5 (c) draws the trees as their meshes |

## Owner checks
Pictures and runs that need the owner (T §7), newest first:
- G1b: on the PC, a new world in hill country and one in a desert: walk a hillside and down a
  gully to its brook, look across a valley from a ridge and out over the land from a hill (the
  far ground should read as the near); in the desert, a wash and a stony slope. Which ground looks
  least real, and what gives it away?
- S5 (c): on the PC, `cargo run --release -- bench --scenes lowland_forest,rainforest_walk`
  and the same with `--trees blocks`: the frame times and the GPU's passes with the trees as
  meshes and as blocks; then `--preset high --scenes rainforest_walk` (S §7.4: a dense
  rainforest at High meets the frame targets). In the world: walk a wood at dawn and at noon;
  look at a trunk and at the leaves within reach (each outlined by its own shape); fell a tree
  and watch it fall; lop a limb from a standing tree and see it turn to its blocks.
- R1a: on the PC, `cargo run --release -- bench --scenes quick --shadows off` and the same
  without `--shadows off`: the shadows' GPU time at 1080p (the "shadows" pass and what the
  terrain passes gain; budget 1.5 ms) and the frame thread's (prepare: terrain). Then in the
  world at dawn: a wood, a person beside you, a far ridge's shadow on the valley; the option
  Video → Quality → Shadows.
- T2: on the PC, `cargo run --release -- --screenshot-list tools/shots/realism.shots` (the
  realism suite, 132 shots), `scripts/fetch-realism-refs.sh --photos` (Wikimedia Commons, free
  licences only), then open `bench-out/realism/sheet.html`: each shot beside real places of its
  kind. Which look least real, and what gives them away?
- T1.3: in the world on the PC, look at a berry bush's berries and its leaves, a branch, a stone
  and a stick lying, the ground and water: each glows along its own shape (only the berries when
  they are looked at); dig a little where the ground glows and see the hole there.
- T1.4: the globe on the PC (New world → Birthplace): turning it under the lamp, the three
  looks and the relief buttons, a range zoomed in past 10 (the finer relief comes in a few
  seconds the first time).
- T1.2: the creator's face and a hand in sunlight, overcast and by firelight on the PC, against
  photographs of skin under the same light (`docs/review/t1/skin_lights_after.jpg` is the
  software device's view).
- T1.1: a carcass in the open by day (Creative: kill a deer, step back to a rise): birds circling
  over it, coming down in turns, lifting as you walk up; a flock seen a kilometre or two off.
- `cargo test -p hearth_worldgen --test planet_pinned` on the laptop: whether Windows' maths make
  the same planet as Linux's (D289). One command, a pass or a failure to paste back.

## G1b — walking scale (the ground: 2026-10-14, D301, RGA-1's gaps 3, 5 and 9)
- **A walking level** of 4.8 m cells beneath the 38 m one (`relief/walking.rs`), made by the same
  tiles and read only by the blocks about the players; its erosion by dry land's storms, its
  relief the dry basins' own surface, a second drainage pass over half-offset tiles, drained and
  its brooks stepping along its rows and columns, its tiles made about the spawn first.
- **Ridges and valleys running downhill** at every level (`relief/gullies.rs`); soil-mantled
  ground smoother below 76 m. **The ground's own roughness** below the cells
  (`Terrain::micro_relief`): soil's mounds and pits, dry land's stony knobs, bare rock; none on a
  channel's bed. Reads between cells never leave the cells' range (`monotone`).
- **Every other read a stand-in** (`walking_proxy`): the distant view (within half a degree of
  the blocks at 30 m), and placement (`sample_regional`: deposits, springs, caves, places' cards).
- **Measured** (`docs/review/g1b/`, G1a at the same windows): |Δh| at 1–64 m and the 4 m
  curvature within × 1.5 of lidar in hills, desert, boreal land and the plains' first window
  (0.68–1.34); the plains' second 0.48–0.56, between the kind's two lidar windows; mountains to
  their 64 m relief (0.66). Closed hollows over 0.25 m 0.0–0.7 % off rivers outside the
  mountains (G1a 0.8–12 %). The planet's measures within bounds; slope at 30 m by kind within
  ± 25 % but the desert's (5.5° against 4.2°). Drenched hills' gradient set again (D301).
- **Tests**: the walking level drains (cells, and as the blocks read it); its channels step along
  rows and columns; reads between cells stay between them; a far read stands in for it; coarse
  callers make none of its tiles (`fine_tiles`); placement reads (`places`, `hydro`).
- **Fast?** Cold, both caches cleared (`BENCHMARKS.md`): places 7.2–7.3 s (G1a 7.7–8.4), a new
  world in control 4.8–4.9 s (5.0–5.3), its save 2.8–3.1 s (2.8); 16 walking tiles, 52 ms each.
  The whole render distance 33–36 s here (28–29): finer distant tiles where the land is
  rougher, llvmpipe's drawing (4.8 s on the laptop at E4.1, budget 30 s).
- **Real?** Ridges, gullies and washes where lidar has them; hollows as rare as the Earth's.
  **Lean?** One module for the walking level; the old blocks' noise gone; the bench's probes
  folded into `realism terrain` (the far read against the blocks). **Whole?** The far view
  stands in for the near ground at 30 m. **Organic?** Gullies on a jittered lattice, brooks
  along curves through their nodes (D299), the micro relief's noise unrepeating.
- **Open** (PLAN): the desert's form, the mountains' kilometre relief and grain, the plains'
  steep valley sides; rivers' banks below their water and stepped lake shores (W1); frost, wind
  and solution forms, clasts, soil surfaces (S5/P7 (e), P7G).
- Next: S5/P7 (d)–(f); Audit 3 after them.

## G1a — relief from 30 m to 30 km (2026-10-09, D299, RGA-1's gaps 2 and 4)
- **Relief by the land's kind and height above base level** (`relief/roughness.rs`): an
  amplitude at the valleys' spacing from the height above the graded profile the rivers could cut
  to, rain, dry land, ice, provinces and mountains' uplift; the spacing as far as hillslopes take
  to rise through it. Dry basins' floors buried at the finer levels. **Water spread over the
  slopes** (`relief/drainage.rs`: many-direction routing and implicit erosion), **hillslope
  creep** at the ~300 m and ~40 m levels, **rivers** whose beds every tile lays the same
  (`relief/courses.rs`), whose courses curve through their nodes at every level and through the
  finest reaches at the blocks, wandering by slope and discharge.
- **Measured** (`bench realism global|terrain`, `docs/review/realism/measures/`), each within its
  bounds: median slope 2.3° (the Earth's 2.8°), relief 183 m (163), land over 5° 30 % (33);
  relief by height 80 · 73 · 188 · 482 · 1,167 m (73 · 80 · 231 · 458 · 1,170); slope at 30 m
  hills 20.0° (21.8), plains 2.9° (3.3), desert 4.9° (4.2), boreal 2.4° (2.1), mountains 28.5°
  (24.6). Before (T2): 1.5°, 78 m, 16 %; 33 · 50 · 89 · 299 · 1,515 m; 1.8 · 2.3 · 2.4 · 1.1 · 29.5°.
- **Tests**: water spreading over a cone instead of down eight spokes; creep; relief by height,
  rain and dry land; rivers falling through every level, curving at the blocks and through their
  nodes at the levels; a cell the same whichever tile is made first.
- **Pictures** (`docs/review/g1a/`): the planet's windows, the five kinds at 30 m beside lidar,
  the river map, the land from 400 m in game.
- **Fast?** Tiles 3–18 % slower (level 3 ~85 ms). R1a's pipelines had frozen a world's arrival
  4–5 s and kept a save 6 s from control; the scene is now made beside the server's opening:
  a new world in control in 3.7–4.1 s, a save in 2.5–2.8 s (`BENCHMARKS.md`).
- **Open** (PLAN): the grid's lakes (6.3 % of the land; their stepped shores) for W1; hills
  right in amount but not in form, the desert's ranges low.
- Next: S5/P7 (c) with G1b, walking scale; Audit 3 after it.

## R1a — the sun's shadows; T2 — the realism gap analysis (done 2026-10-09)
Their entries: `docs/history/progress-2026-10-09-r1a-t2.md`.

## T1 — quick wins (done 2026-10-09)
Its entry: `docs/history/progress-2026-10-09-t1.md`.

## E4.1 — Earth-scale performance (done 2026-10-09)
Its entry: `docs/history/progress-2026-10-09-e4.1.md`; the numbers in `BENCHMARKS.md`.

## S5 with P7 — (c) done 2026-10-14 (D300); (d)–(f) with G1b
Where it stands (`PLAN.md` S5, P7; tasks (a)–(f)):
- **(a) done:** `hearth_flora::mesh` builds a tree's mesh from the skeleton its blocks come
  from (wood tubes, leaf cards by leaf kind, three details: a mature oak 47k / 8.7k / 1.7k
  triangles); each variant turned to its own angle and up to 0.4 m off its block's middle, voxels
  and mesh alike (`template::variant_skeleton`, `Turn::apply_f`).
- **(b) done:** `hearth_render::trees` (instanced meshes, wood and alpha-tested leaves, wind
  sway, seasons, translucency; `shaders/tree.wgsl`) in the scene's opaque pass;
  `hearth::tree_draw` gives each species its look from its blocks' colours (D290). The species
  sheets (`hearth/tests/tree_sheets.rs`, 91 species × three ages × four seasons) hold every
  species to its kind.
- **(c) done (D300): the natural trees drawn as their meshes in the world.**
  - The server leaves their blocks out of the cubes' meshes where they still hold their natural
    state (`LocalWorld::mesh`). The client, the screenshot tool and the bench find the same
    trees from the generator and the vegetation, a column at a time on worker threads
    (`hearth::tree_view`).
  - Each tree is drawn at the detail its distance asks for (meshes made off the main thread, a
    coarser one meanwhile), turned and placed as its blocks are, lit beside its foot and over its
    crown, with its column's climate and snow.
  - A tree cut into by the player or a fire is marked in the vegetation (as what it then is) and
    drawn as its blocks until another grows there.
  - A felled tree falls as its mesh, its stump left (`TreeFalls` carries its foot); the tree looked
    at is outlined by its own mesh, wood or leaves.
  - Options: `--trees blocks|meshes` for `hearth bench`, `trees=` and `fell=` for screenshots; a
    bench scene `rainforest_walk` (S §7.4).
- **Tests:** the mesh stands where its blocks do (`tree_meshes`); a tree cut into is meshed as its
  blocks, kept as cut while it is what it was, saved (`tree_meshes`, `vegetation`); the server
  tells a felling by the tree's foot after its vegetation, and a limb lopped marks its tree cut
  (`felling`); a felled tree's mesh has no stump and its cut capped (`hearth_flora`); the tilt's
  packing (`tree_draw`).
- **Pictures** (`docs/review/s5/world/`): before and after at head height in four biomes and
  from 40 m in two, a winter wood from above, a tree's fall, the outline of a trunk and of leaves.
- **Fast?** (`BENCHMARKS.md`, the same build with `--trees blocks`) the forest flight's draws
  5,316 → 2,461, triangles 1.54 → 0.88 M, video memory 360 → 289 MiB; the rainforest walk's
  6,634 → 3,194, 2.55 → 1.53 M, 418 → 352 MiB (the shadow cascades' draws counted, each cascade
  drawing only the trees that may cast into it). The trees' frame-thread work 0.73–0.79 ms here
  with the camera at 80 m/s (budget 1 ms). The frame times need the PC (owner check).
- **Open:** the look still meets a tree by its blocks (close to its drawn shape), and so does
  collision (the skeleton's capsules, S §7.1); the fallen trunk lies as blocks (logs, (e)); the
  trees past the render distance stay boxes (L1).
- **Next, with G1b:** (d) woody shrubs as individuals, Poisson-like spacing, clustering, clonal
  patches, herbs off the grid, P §11.4's statistics; (e) boulders and fallen logs as smooth
  shapes, dipping strata and veins, the §11.3 evaluation, the screenshot set (with G1's clasts and
  landforms); (f) docs.

## Content
`hearth content status` (implemented = used by a system; planned = data only):

| Domain | Implemented | Planned |
|---|---|---|
| Materials | 103 | 256 |
| Rocks, minerals, provinces, deposit models, soils | 35, 27, 14, 52, 16 | 0, 3, 0, 0, 0 |
| Plant species | 223 | 3 |
| Animal species | 360 | 0 |
| Ecosystems | 20 | 0 |
| Item forms | 66 | 0 |
| Processes | 1,551 | 0 |
| Knowledge nodes | 86 | 94 |
| Workstations, construction pieces, garments | 11, 21, 11 | 0 |
| Injuries, illnesses | 10, 7 | 0 |
| Eras | 1 | 9 |

## Known issues
- Night by a fire (D201): a camera in the dark looking at a fire from beyond its light washes
  the firelit ground out white; adaptation is reckoned where the camera stands.
- Finite water: a channel dug through a river's bank a block below the water takes the river's
  water without end (the hydrology's rivers do not fall as a breach takes from them; D190).
- Presents never block in the cloud environment (likely an occluded window); re-check pacing
  on a visible window.
- Oceans: the sea's realms are its coasts' (D152); seabirds only over land cells; crabs, seals
  and sea fish not drawn walking; deep animals drawn near the surface; mangrove channels' fish
  not kept (D159).
- Wetlands: waterfowl stand on the bank rather than swim; a walking bird is drawn with its
  wings spread; reed beds hide much (D148).
- Mountains: the planet's tropics have seasons, so tropical mountains have a winter (D136; E4
  recalibrates); a realm's mountains are one community (D140).
- Time going faster (E3): the finite water's flow keeps its own pace; beyond 100 times the
  animals about the player lag the clock until the populations' tier takes over.
- Animals: no lion's mane or peacock's train; thin limbs under dense crowns draw nearly black.
- Distant terrain: the player's changes reach it as each column's top block; the tile
  selection runs on the frame thread (5–9 ms after a 16 m move); FXAA is not built.
- Not drawn yet: lightning, fog banks, splashes; no shadow maps (V2-16). Snow per column (no
  drifts); near the player its layers lie under the smooth surface, showing only as patches
  (S7 makes snow fill). Sounds placed in the world, music, thunder, water and
  animal sounds wait (S6–S8).
- Deferred to their milestones (detail in the history file): flocks and insects seen; seasonal
  flowers; migrations as journeys; trees regrowing beyond their sites, rot, spreading fire,
  smoke; panning and deposit zoning (V2-13); warm fumarole ground; containers opened only by
  picking up, wetness of things; snow loads, decay on textures, arches and domes (S6, V2-13).

## Environment
- The owner's PC: Ryzen 7 7840HS (8C/16T), RTX 4060 Laptop + Radeon 780M, 16 GB; Rust by
  rustup (`export PATH="$HOME/.cargo/bin:$PATH"` in older shells).
- The cloud machine: 4 cores, 15 GB, no GPU (llvmpipe); the git proxy refuses tag pushes.
