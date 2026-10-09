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
E4.1. V2-11 superseded.

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
| T1 — quick wins (ravens, skin, highlight, globe) | under way: T1.1, T1.2, T1.4 done (D292–D294) |
| T2 — the realism gap analysis | planned |
| S5/P7 (c)–(f) with G1 → W1 → L1 → P7G → S6 with H1 → R1 → S7 → S8 → P8 | planned (T2 may revise) |
| V2-13 → V2-14, Audit 4; V2-15 → V2-16, Audit 5 | planned |
| Phase R-A, Audit 6; R-B, Audit 7; R10; Phase F | planned |

| Row | State |
|---|---|
| Smooth world (S) | S4 done: the distant ground smooth height fields in fixed point, shaded with the near ground's materials and seasons, canopies over far stands, out to the real horizon. S3 done: movement on the field. S2 done: natural ground meshed smooth on the server and drawn with blended procedural materials, wet and snow overlays (0.28–0.87 ms a surface cube, as the blocks); frame targets need the PC. S1 done: fill in every surface cube, generated and saved; ground families; dig, pile and settle conserving volume (sand to 34.3°). S0 done (D222: Surface Nets with sharp features, biplanar shading); Baseline-S's CPU half recorded, its GPU half needs the PC (`scripts/baseline-s.sh`); prototype mesher 3,553 surface cubes/s on one thread (target 2,000 on eight) |
| Playability (P) | P0–P5 done (P6 removed); open issues in `dev/PLAYTEST.md` |
| Earth-True (E) | E0–E7 done 2026-10-08; E4.1 (the Earth-sized world fast to see, make a person on and load into) done 2026-10-09 |
| Quality (Q) | Audit 0 done 2026-10-08 (`docs/review/audits/AUDIT-0.md`); open high-priority findings: none; Audit 1 done 2026-10-08 (`AUDIT-1.md`); Audit 2 done 2026-10-08 (`AUDIT-2.md`); next: an audit after three completed milestones (Q §8.2) |
| Realism (T) | last suite render: none yet (T2 makes the suite); top open gaps from the owner: the ground below ~38 m (PLAYTEST 40–41), water as blocks (39), the seam at the edge of full detail (42) |

## Owner checks
Pictures and runs that need the owner (T §7), newest first:
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
- Next: T1.3 the thing aimed at; then T1's acceptance and `main`.

## E4.1 — Earth-scale performance (2026-10-09, D279–D289, `docs/spec/e4.1-earth-scale-performance.md`)
The owner's playtest of the Earth-sized world (PLAYTEST 29–31): the globe all blue and laggy, the
creator slow, loading maxing the CPU and never arriving. Measured first (`hearth bench
globe|creator|load`, `hearth_core::prof`, the crash log), then fixed; the numbers before, at
step 5 and after are in `BENCHMARKS.md`, the budgets in `docs/design/budgets.md`.
- **Queries at their scale** (D279): the globe, the far field, the distant tiles, the animals'
  habitats and the places read the level their footprint needs; `tests/fine_tiles.rs` (in
  `scripts/check.sh`) fails if a coarse caller builds a tile of the finest level.
- **The globe** (D280): its map from the planet grid in 1.1 s, kept beside the planet (was
  never finished: some 9 hours); hover 0.004 ms (154); a click's details on a thread, 0.48 s.
- **The creator** (D281): colours the next frame; a shape at once roughly, then coarse, then full
  detail, the last setting shown at 0.14 s (2.3–2.7 s).
- **Loading** (D282–D284, D288): one job system with four priorities and a core kept free; the
  menus' generator kept for the world; stages and Cancel; the animals made in parallel and set
  aside far from the player; the refinement tiles kept on disk; the near ground streamed nearest
  and in view first, each cube fading in. A new world in control at 3.35 s (never, behind the
  menus' work), its save at 2.34 s (32.8 s); the whole render distance at 37.1 and 34.6 s
  (llvmpipe's drawing most of it).
- **Memory** (D283, D287): budgets by kind (40 % of the machine's for the caches), memory by
  kind in F3; a 30-minute flight found three leaks (a column's record outliving its cubes, the
  animals' regions never let go, meshes a slow client had not taken piling up in the channel:
  now one per cube in the server's outbox, at most 512 unread). The flight holds 1.6–1.9 GB from its third minute to its thirtieth (the near terrain's arena
  doubling once on the way), 2.1 GB at the most (before: 1.3 → 2.5 GB, rising throughout).
- **Precision and the seam** (D285): every noise periodic in the circumference, features hashed
  by their place round the planet, geometry about its own origin or in f64, the shaders'
  patterns periodic in 4,096 blocks; a body crossing a pole comes out on the far side. The seam
  and pole tests run in `scripts/check.sh`.
- **The machine** (D286): the first run picks the preset from the cores, the memory and the
  adapter (its memory from Windows' registry or Linux's sysfs); the perf gate runs the three
  benches on both builds (`bench earth-judge`: a median 5 % worse beyond a floor fails, as does a
  coarse caller building the finest tiles).
- **Realism** (`docs/review/earth-scale-performance.md`): the planet the same to the byte; all
  of 600 sampled columns at the same height; the land's kinds in the same measure (1.8 % of the
  blocks trading kinds); individual trees, plants and outcrops in other places (the noises made
  periodic, D285); before and after pictures alike at a distance. The coastal salt pans' test
  wanted a finer lattice after the shelter's noise was redrawn (28 columns, all where they
  belong).
- **Five tests:**
  - *Real?* The planet and the land's shape unchanged to the byte and the block; the land's
    make-up and Earth's hypsometry (29.0 % land, 791 m mean height, 3,640 m mean depth) as
    before, checked against the stopping point's own build, census and pictures.
  - *Lean?* The globe's block-level map, the creator's whole-body rebuild for a colour, the
    menus' second generator and the duplicate meshes in the channel are gone; one job system
    replaces rayon used ad hoc; the disk caches capped; nothing unused (`lean-check`).
  - *Fast?* Every budget met: here but two set for the reference machine (the whole render
    distance 34.6–37.1 s against ~30 s, the creator's full detail 0.9–1.2 s against 1 s; llvmpipe
    and four cores), and those on the owner's laptop (8 cores, RTX 4060 Laptop): the whole render
    distance in 4.8 s, the creator's full detail in 0.24 s (`BENCHMARKS.md`, run with `cargo run
    --release -p hearth -- bench load|creator|globe`); the gate holds the Earth-scale numbers
    from the next baseline; memory flat in the 30-minute flight.
- **After E4.1** (D289, PLAYTEST 32–33): the owner's numbers showed this machine's cached seed 7
  was an older build's planet, used for every new world of the seed. The cache's name now
  carries the generator's version (pinned by `tests/planet_pinned.rs`), and a world keeps its
  planet in its folder. On this build's planet the whole render distance comes at 20 s here
  too. Open: the first world after an update stops the loading screen 1.4 s making the scene's
  pipelines (PLAYTEST 33); whether Windows' maths make the same planet (`planet_pinned` on the
  owner's machine).
  - *Whole?* What could look out of place: a world saved before E4.1 opens with its trees and
    plants moved (reviewed, D285); cubes fading in while they load; the land past a pole's edge
    not drawn until crossed (PLAN). Checked with the review's pictures and the seam, pole,
    fine-tile, outbox and in-view tests.
  - *Organic?* No new repetition: the generator's patterns repeat only round the planet
    (40,000 km), the shaders' grain and beds every 4 km; the census's tops and kinds as varied
    as before.

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
