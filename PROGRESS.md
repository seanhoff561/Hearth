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
   and the perf gate. The cloud machine renders only on a software device: frame rates, the
   perf gate and how screenshots look need the owner's PC.

## Milestones
Done: M0–M3 (v1 engine), V2-0 – V2-10, V2-12 (the Neolithic), H0–H10 (removed in E0), S0,
P0, P1, P2, E0, Audit 0, Q1, E1, E2, E3, E4, E5, P3, P4, E6, P5, S1, S2, E7, S3, S4, Audit 2,
E4.1, T0, T1, T2. V2-11 superseded.

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
| R1a → G1a → S5/P7 (c)–(f) with G1b → W1 → L1 → P7G → S6 with H1 → R1b → S7 → S8 → P8 | next: R1a (D297) |
| V2-13 → V2-14, Audit 4; V2-15 → V2-16, Audit 5 | planned |
| Phase R-A, Audit 6; R-B, Audit 7; R10; Phase F | planned |

| Row | State |
|---|---|
| Smooth world (S) | S4 done: the distant ground smooth height fields in fixed point, shaded with the near ground's materials and seasons, canopies over far stands, out to the real horizon. S3 done: movement on the field. S2 done: natural ground meshed smooth on the server and drawn with blended procedural materials, wet and snow overlays (0.28–0.87 ms a surface cube, as the blocks); frame targets need the PC. S1 done: fill in every surface cube, generated and saved; ground families; dig, pile and settle conserving volume (sand to 34.3°). S0 done (D222: Surface Nets with sharp features, biplanar shading); Baseline-S's CPU half recorded, its GPU half needs the PC (`scripts/baseline-s.sh`); prototype mesher 3,553 surface cubes/s on one thread (target 2,000 on eight) |
| Playability (P) | P0–P5 done (P6 removed); open issues in `dev/PLAYTEST.md` |
| Earth-True (E) | E0–E7 done 2026-10-08; E4.1 (the Earth-sized world fast to see, make a person on and load into) done 2026-10-09 |
| Quality (Q) | Audit 0 done 2026-10-08 (`docs/review/audits/AUDIT-0.md`); open high-priority findings: none; Audit 1 done 2026-10-08 (`AUDIT-1.md`); Audit 2 done 2026-10-08 (`AUDIT-2.md`); next: an audit after three completed milestones (Q §8.2) |
| Realism (T) | last suite render: T2, 2026-10-09 (`docs/review/realism/`); top open gaps (`scorecard.md`): no cast shadows; the relief from 30 m to 30 km (half the Earth's, a sixth in hill country); trees, grass and water as voxels, sprites and blocks |

## Owner checks
Pictures and runs that need the owner (T §7), newest first:
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

## T2 — the realism gap analysis (2026-10-09, Amendment T §3, D296–D297)
- **Measured** (`bench realism terrain|global|levels|weather|images`; `docs/review/realism/`):
  the planet right (land 29.0 %, mean land 791 m); the land between 30 m and 30 km half the
  Earth's relief (200 random windows each: median slope 1.5° against 2.8°, relief 78 m against
  163 m), a sixth in hill country (the Oregon Coast Range's 762 m over 30 km, the generator's
  122 m) — each level's relief scaled by uplift alone; below 38 m noise two to six times smoother
  than lidar; drainage on the grids' eight directions; closed hollows where real ground drains;
  no cast shadows; trees, grass and water as voxels, sprites and blocks; the seam 3–8 times the
  steps beside it in forests. `RGA-1.md` with the ranked table; `scorecard.md`.
- **The suite** (`tools/shots/realism.shots`): 16 biomes and a stream × underfoot, eye height, a
  40 m rise, a 400 m hill × late morning and low sun, 132 shots, each shot's seam measured
  (`seams.tsv`); sheets in `docs/review/realism/suite/`. References (`references.md`): USGS 3DEP
  lidar and SRTM for five matched places, AWS terrain tiles for the planet-wide windows; the
  photographs by `scripts/fetch-realism-refs.sh --photos` (Commons is closed to this machine).
- **Fixed in passing** (D296): the weather took wet days for wet hours — London rained in 29 %
  of its hours, now 6.0 % (records 6–8 %), the years within 5 % of their normals.
- **The plan revised** (D297): R1a (sun shadows) → G1a (relief from 30 m to 30 km) → S5/P7
  (c)–(f) with G1b (walking scale) → W1 → L1 → P7G → S6 with H1 → R1b → S7 → S8 → P8.
- **Accepted** (T §8): `RGA-1.md` with evidence at every scale of T §3.1, the ranked gap table,
  the realism suite and its metrics (`bench realism`), the scorecard, `PLAN.md` revised (D297);
  `scripts/check.sh` green.
- **Real?** The analysis is the Earth's numbers against ours; the weather now rains as places
  do. **Lean?** One bench module; `tiff` and `zune-jpeg` only in the bench tool; the
  E4.1 entry moved to `docs/history/`. **Fast?** No cost in the game (the weather's thresholds
  two lookups). **Whole?** The suite reviewed shot by shot (RGA-1 §2.13). **Organic?** The
  suite's repetition measured (`images.md`).
- Next: R1a, sun shadows.

## T1 — quick wins (2026-10-09, Amendment T §2)
- **T1.1 birds over remains, notices shown** (D292, PLAYTEST 34): `hearth_fauna::flock`; the
  scavengers circle fresh remains by day (live within 150 m, drawn by the client to 2.5 km from
  `ToClient::Flocks`), come down in turns, lift when someone is near, leave; calls near and far.
  Kill, strike, attack and herd lines gone; a blow is heard (`ToClient::Struck`,
  `Sound::Strike`). Tests: `hearth_fauna/tests/flocks.rs` (ring speed and heading, turns, lifting,
  leaving, server and client alike), `hearth/tests/fauna.rs` (crows over a hind 300 m off; the
  spear test reads the world). Far birds drawn as specks; pictures `docs/review/t1/`
  (`tools/shots/t1_birds.shots`). Owner check: a carcass by day seen from a rise.
- **T1.2 skin as measured** (D293, PLAYTEST 35): roughness by region, pores and lines (normals
  or roughness by the pixel), creases, the sky reflected, scattering pre-integrated by
  curvature (`hearth_render::skin_lut`), a water film, measured skin's albedo. Sheet:
  `skin_under_four_lights`; pictures `docs/review/t1/skin_*`. Owner check: the creator and a
  hand in sunlight on the PC.
- **T1.4 the globe in relief** (D294, PLAYTEST 38): heights at 4096 × 2048 (the grid's surface,
  bicubic) lighting the land and the sea floor, exaggerated more at small scales; hypsometric
  tints, the sea's depths, rivers by discharge, lakes, ice; by biome, climate or relief (buttons,
  keys 1–5); the 2.4 km level's relief over the view from zoom 10. Made in 1.2–1.3 s, read back
  in 0.03 s (kept in four parts unpacked in parallel), hover 0.006 ms (`hearth bench globe`).
  Tests: `tests/globe_map.rs` (land share, heights, rivers, ice, kept and read back the same),
  `bicubic_rows` sum for sum. Pictures `docs/review/t1/globe_*` (`tools/shots/t1_globe.shots`).
- **T1.3 the thing itself** (D295, PLAYTEST 37): `aim.rs` meets what is drawn — blocks by their
  models' quads and textures' alpha (the season's leaf fall too), a plant's fruit apart from its
  leaves (sprites mark them, `PART_ALPHA`), things by their drawn boxes, the ground's dig patch,
  water's surface (0.010–0.012 ms a look); `hearth_render::outline` glows along that shape (a
  mask of its own geometry, unjittered and depth-tested, a glow laid after the tonemap). A dig
  takes its earth where the look rested (`ToServer::Act::at`, `ground::dig` shares a stroke
  over the voxels about the point). Tests: `tests/picking.rs` (fruit, leaves, gaps, water,
  leaf fall, cost), `scene_shaders` (the glow only about its shape), `workshop` and
  `hearth_world/tests/ground.rs` (the hole where struck). Pictures `docs/review/t1/aim_*`
  (`tools/shots/t1_aim.shots`, keys `look=`, `thing=`). Box edges gone.
- **Accepted** (T §2): no notice repeats (the world's lines reviewed, D292); birds over remains
  near and as specks 2.5 km off; skin close-ups under four lights; each aim target glows along
  its own shape (berries, a branch, a stone, a stick, a dig patch, water); the globe's relief,
  depths and rivers; `scripts/check.sh` green; the CPU budgets met. The GPU's share (the
  skin's, the glow's) and the perf gate need the PC (owner checks), as the gate's baseline is
  still H1's.
- **Real?** Birds as scavengers gather (by carrion share and mass, by day, lifting at people);
  skin's roughness and albedo from measured faces (Weyrich et al. 2006); the look meets the
  shape drawn, the leaves a season dropped included; the globe's heights from the grid itself.
- **Lean?** Gone: five repeating notices, the box-edge highlight (`box_edges`), the cell-based
  aim and its boxes, the second sharp skin lobe, map format 2.
- **Fast?** A look 0.010–0.012 ms (< 0.2); the globe made in 1.28 s, read back in 0.03 s, hover
  0.006 ms; flocks reckoned from the clock (no state per bird). Gate: the PC's.
- **Whole?** Pictures reviewed (`docs/review/t1/`): the glow follows only the thing looked at
  and hides behind what stands before it; birds only where remains are; the globe's three looks.
- **Organic?** Each bird on its own ring and phase; pores and lines by region, not tiled; a dig's
  leftover steps by a draw, not always the nearest voxel.
- Next: T2, the realism gap analysis.

## E4.1 — Earth-scale performance (done 2026-10-09)
Its entry: `docs/history/progress-2026-10-09-e4.1.md`; the numbers in `BENCHMARKS.md`.

## S5 with P7 — stopped at (b) for Amendment T (T0, 2026-10-09)
Where it stands (`PLAN.md` S5, P7; tasks (a)–(f)); (c)–(f) resume after T2, with G1:
- **(a) done:** `hearth_flora::mesh` builds a tree's mesh from the skeleton its blocks come
  from (wood tubes, leaf cards by leaf kind, three details: a mature oak 47k / 8.7k / 1.7k
  triangles); each variant turned to its own angle and up to 0.4 m off its block's middle, voxels
  and mesh alike (`template::variant_skeleton`, `Turn::apply_f`).
- **(b) done:** `hearth_render::trees` (instanced meshes, wood and alpha-tested leaves, wind
  sway, seasons, translucency; `shaders/tree.wgsl`) in the scene's opaque pass;
  `hearth::tree_draw` gives each species its look from its blocks' colours (D290), its mesh
  keys and meshes, and an instance's packing. The species sheets (`hearth/tests/tree_sheets.rs`,
  91 species × three ages × four seasons, `docs/review/smooth-world.md`) found twenty
  evergreen species shedding their leaves (retinted, D290) and a conifer leader's hoop of
  sprays (sprays now fill their shoots); a test in the suite holds every species to its kind.
- **Next, (c):** trees drawn as meshes in the world. Worked out before T0 stopped it:
  `LocalWorld::mesh` (every mesh goes through it) blanks the voxels of trees drawn as meshes
  before meshing, where they still hold their natural state; which trees those are is decided
  on both sides from the generator and the shared `Vegetation` alone, so server and client
  agree: every natural tree but those the player has cut into (a set of feet kept in the
  vegetation, saved and sent with it; marked where player edits are recorded, the cubes the tree
  spans remeshed), so what is taken from a tree shows as blocks; `PlacedTree` carries its
  variant for the mesh key; a tree's instance is its foot plus `Turn::apply_f`'s affine map (an
  origin and a 2×2 turn); the client lists trees by loaded column (`trees_in`), detail by
  distance, meshes built off the main thread; the screenshot tool draws them; felling animates
  the mesh (`TreeFalls` carrying the tree); trees beyond the near field as instances, then
  impostors, never boxes (with L1); collision against the skeleton's capsules (From S3).
- **Then:** (d) woody shrubs as individuals, Poisson-like spacing, clustering, clonal patches,
  herbs off the grid, P §11.4's statistics; (e) boulders and fallen logs as smooth shapes,
  dipping strata and veins, the §11.3 evaluation, the screenshot set (with G1's clasts and
  landforms); (f) docs.
- **The stopping point's check** (`scripts/check.sh`, the whole suite) found two tests to mend:
  the felling tests looked for a slim tree by its blocks' shape, which trees off the grid no
  longer have, and now take its foot from where the generator placed it; the finite-water test
  found no river bank that holds a channel among the four it looked at, and now looks at some
  twelve times as many.

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
