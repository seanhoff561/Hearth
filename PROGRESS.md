# Progress

Current status only (Amendment Q §5.4). The full record of M0 to E0 (2026-09-30 to 2026-10-08)
is `docs/history/progress-2026-09-30-to-10-08.md` (then `progress-2026-10-08-e0-to-e5.md` and
`progress-2026-10-08-p3-to-s2.md`); decisions are indexed in `DECISIONS.md`
with their texts in `docs/decisions/`.

Direction: v2 (`docs/spec/v2-direction-change.md`) as amended by S (smooth world), P
(playability) and, above them, **E and Q** (`docs/spec/amendments-e-q.md`, D227): Earth-True
and the Quality Charter. V2.1's simulated humans were removed in E0 and archived
(`docs/archive/humans-v2.1/`, D230); Phase F plans them anew. Amendment R waits until V2-16.

## Resume
1. Read this file, `PLAN.md` and the `DECISIONS.md` index; `git log --oneline -20`. Open
   `docs/decisions/`, `docs/history/` or a design doc only when a task needs it.
2. Run `scripts/check.sh` (it runs `scripts/lean-check.sh`); the long runs are
   `scripts/soak.sh`, at audits (D235).
3. Work in `PLAN.md`'s order; each milestone ends with its five-test checklist (Q §8.1) below.
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
| E4.1 — Earth-scale performance (the owner's fix pass) | done 2026-10-09 (D279–D288) |
| S5 with P7 | now: resumes at (b), where E4.1 paused it |
| P7G → S6 → S7 → S8 → P8, Audit 3 | planned |
| V2-13 → V2-14, Audit 4; V2-15 → V2-16, Audit 5 | planned |
| Phase R-A, Audit 6; R-B, Audit 7; R10; Phase F | planned |

| Row | State |
|---|---|
| Smooth world (S) | S4 done: the distant ground smooth height fields in fixed point, shaded with the near ground's materials and seasons, canopies over far stands, out to the real horizon. S3 done: movement on the field. S2 done: natural ground meshed smooth on the server and drawn with blended procedural materials, wet and snow overlays (0.28–0.87 ms a surface cube, as the blocks); frame targets need the PC. S1 done: fill in every surface cube, generated and saved; ground families; dig, pile and settle conserving volume (sand to 34.3°). S0 done (D222: Surface Nets with sharp features, biplanar shading); Baseline-S's CPU half recorded, its GPU half needs the PC (`scripts/baseline-s.sh`); prototype mesher 3,553 surface cubes/s on one thread (target 2,000 on eight) |
| Playability (P) | P0–P5 done (P6 removed); open issues in `dev/PLAYTEST.md` |
| Earth-True (E) | E0–E7 done 2026-10-08; E4.1 (the Earth-sized world fast to see, make a person on and load into) done 2026-10-09 |
| Quality (Q) | Audit 0 done 2026-10-08 (`docs/review/audits/AUDIT-0.md`); open high-priority findings: none; Audit 1 done 2026-10-08 (`AUDIT-1.md`); Audit 2 done 2026-10-08 (`AUDIT-2.md`); next: Audit 3 after P8 |

## E4.1 — Earth-scale performance (2026-10-09, D279–D288, `docs/spec/e4.1-earth-scale-performance.md`)
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
  - *Fast?* Every budget met on this machine but two set for the reference machine (the whole
    render distance 34.6–37.1 s against ~30 s, the creator's full detail 0.9–1.2 s against 1 s;
    llvmpipe and four cores here); the gate holds the Earth-scale numbers from the next baseline;
    memory flat in the 30-minute flight. The owner is asked for `hearth bench load`, `creator`
    and `globe` (`cargo run --release -p hearth -- bench …` in PowerShell, the output teed to
    `bench.txt`).
  - *Whole?* What could look out of place: a world saved before E4.1 opens with its trees and
    plants moved (reviewed, D285); cubes fading in while they load; the land past a pole's edge
    not drawn until crossed (PLAN). Checked with the review's pictures and the seam, pole,
    fine-tile, outbox and in-view tests.
  - *Organic?* No new repetition: the generator's patterns repeat only round the planet
    (40,000 km), the shaders' grain and beds every 4 km; the census's tops and kinds as varied
    as before.

## S5 with P7 — resuming (paused for E4.1 on 2026-10-08)
Where it stands (`PLAN.md` S5, P7; tasks (a)–(f)):
- **(a) done:** `hearth_flora::mesh` builds a tree's mesh from the skeleton its blocks come
  from (wood tubes, leaf cards by leaf kind, three details: a mature oak 47k / 8.7k / 1.7k
  triangles); each variant turned to its own angle and up to 0.4 m off its block's middle, voxels
  and mesh alike (`template::variant_skeleton`, `Turn::apply_f`).
- **(b) in progress:** `hearth_render::trees` (instanced meshes, wood and alpha-tested leaves,
  wind sway, seasons, translucency; `shaders/tree.wgsl`) is built and wired into the scene's
  opaque pass but nothing gives it trees yet; `hearth/tests/scene_shaders.rs` checks its
  pipelines. Next: species silhouette sheets (three ages, four seasons) in the screenshot tool.
- **Next:** (c) the client's tree instances for loaded columns (from `trees_in` with the
  vegetation), the cube mesher hiding natural tree voxels (a per-cube mask from the templates),
  the LOD band (levels 0–2) as instances or impostors instead of crown boxes, the felling
  animation with the mesh; (d) woody shrubs as individuals, Poisson-like spacing, clustering,
  clonal patches, herbs off the grid, P §11.4's statistics; (e) boulders and fallen logs as
  smooth shapes, dipping strata and veins, the §11.3 evaluation, the screenshot set; (f) docs.
- **The stopping point's check** (`scripts/check.sh`, the whole suite) found two tests to mend:
  the felling tests looked for a slim tree by its blocks' shape, which trees off the grid no
  longer have, and now take its foot from where the generator placed it; the finite-water test
  found no river bank that holds a channel among the four it looked at, and now looks at some
  twelve times as many.

## Audit 2 (2026-10-08, `docs/review/audits/AUDIT-2.md`)
- Metrics: 131k lines in `src/` (+8k: people, smooth ground, LOD fields); files over 2,000
  lines 8 (`hearth_lod` split); PROGRESS trimmed to 14 KB.
- Fixed: earth piled over a cave fell into it; hair moved by frame rate; the LOD quads' dead
  water path; duplicates in the mover, hair, ground ray cast and renderer.
- Hotspots: LOD tiles growing real trees (35–118 ms), far-field tiles (29–66 ms), meshing a
  person (0.7 s); the GPU gate needs the PC.
- Open (PLAN): crown-box snow, step heights by size, a real friction coefficient, duplicates.

## S4 — the distant terrain smooth (2026-10-08, D274–D278)
- **Heights:** LOD columns keep the fill's surface in sixteenths of a block.
- **Ground:** each tile is a smooth height field of 33 × 33 corners (each the mean of the four
  columns about it), with the field's normals and skirts sized to the crack against a coarser
  neighbour. One instanced draw for every tile. A tile's error is how far the field strays
  from its columns, so smooth slopes no longer refine as staircases.
- **Matched shading:** natural ground takes the near ground's material table (one slot
  numbering for both) at the mean of its noise, with the same tints and wetness. Snow and ice
  follow the seasons of the same year model that lays the near cover.
- **Trees:** crown boxes stay where trees are grown one by one (they match the cubes' trees
  until S5). Far stands are a canopy surface, rounded to the edge, conifers pointed. Sparse
  woodland crowns as many columns as its cover, so it stays woodland into the distance.
- **To the horizon (E §9.2):** the quadtree's roots are now the coarsest tiles that go round
  the planet whole (32,768 blocks on Earth); their columns read the coarsest refinement
  levels. From 3 km up on Earth some 2,200 tiles reach 230 km, built in 12 s on 4 threads.
- **Review:** `docs/review/s4/` (`tools/shots/s4_distant.shots`): from above, the full-detail
  square sits in its LOD with no seam or colour jump. LOD tile cache format `HLT4`.
- **Tests:** the height field, skirts and error; canopy profiles; crown boxes; snow and ice
  seasons against the year model; roots round the planet; the cache. `lod_horizon` passes.
- **Five tests:**
  - *Real?* Heights from the fill; snow and ice from the same climate model as near.
  - *Lean?* One mesh and shader for near LOD and far field; no separate planet mesh. The
    renderer reads the near ground's own material table.
  - *Fast?* Tiles build as fast as before once the season memo is warm (5.8 s for 1,600
    tiles). The ground draws 2,304 triangles a tile: GPU time needs the PC (PLAN, From S4).
  - *Whole?* Ground, water, snow, ice, canopies, the far field and the tile cache all changed.
    Crown boxes keep the old snow rule, and near snow still sits under the smooth surface
    (S7).
  - *Organic?* Woodland thins into scattered crowns, winter comes to the far hills on the
    same day as nearby.

## S3 — moving on the smooth ground (2026-10-08, D271–D273)
- **Collision:** natural ground is collided as the fill's field, not as block boxes; built
  things stay boxes. Server and client run the same code on the same fill.
  - The feet stand on the surface under the body's middle and four points half its radius out.
  - Earth higher than a step pushes the body back along the surface.
  - On the ground, the body is kept on a slope going down; ledges of earth are climbed.
- **Slopes:**
  - No steps on natural ground.
  - The steepest slope walked up follows the footing's grip: about 42° dry, 34° on wet clay,
    3° on ice. Steeper ground is slid down.
  - Pace follows Tobler's function; effort follows the ACSM equations (a 10 % rise about
    doubles a walk's cost).
- **Animals** stand on the smooth surface (one fill lookup a column). Their paths cost slopes
  and go round ground steeper than their kind takes, by the gentle way.
- **Building:** pieces on natural ground have a buried half-metre skirt. **Level the ground**
  (digging stick, two hours) flattens a 3 m square to its middle height, volume conserved.
- Bodies are set down on the terrain's surface itself. Footsteps already read the voxel the
  feet stand in.
- **Tests:** seven on smooth ground in `hearth_physics`; levelling; going round steep ground;
  the skirt; effort uphill. The creative resume test now checks the field. A habitat test's
  coastal spawn (half land) was fixed.
- **Five tests:**
  - *Real?* Grip from friction, Tobler's pace, the ACSM's oxygen cost; no steps on slopes.
  - *Lean?* No collision meshes: the fill the mesher reads is what the feet stand on. Animals
    pay one lookup.
  - *Fast?* A handful of field samples a substep. The budgets still need the PC.
  - *Whole?* Players, animals, paths, building and effort all moved to the smooth ground.
    Footprints, wet grip and footwear wait (PLAN, From S3).
  - *Organic?* Herds take the gentle way round; a muddy slope sends a body sliding.

## E7 — realistic people (2026-10-08, D266–D270, `docs/design/people.md`)
- **The body** is a signed distance field of some 150 anatomical forms on the rig's joints
  (trunk, girdle and muscles, hands with fingers and nails, feet with toes, a face with sockets
  and lips), sized by the creator's proportions, build and face sliders.
  - It is sculpted in an A-pose and meshed by Surface Nets at 6, 12 or 24 mm by distance.
  - Vertices are projected onto the field, with its gradient for normals.
  - Four joint weights a vertex, from the nearness of each joint's forms.
  - At 6 mm: about 68,000 vertices and 135,000 triangles, meshed in about 0.7 s on a worker
    thread (dev-opt).
- **Skin** scatters light under it (red wrapped widest) with two GGX lobes. Its state comes
  from the body simulation (`hearth_body::skin`, saved, in `BodyView`):
  - sunburn and tan from the UV index against a minimal erythema dose by tone;
  - dirt from soil underfoot and in the hands, blood where wounds bleed, both washed off;
  - scars where injuries healed;
  - drawn with wetness, pallor, flush and goosebumps.
- **Garments:** the loincloth (a cord and draped flaps that swing with the thighs) and the
  chest band are fitted meshes; other garments stay boxes over the body.
- **Hair:** 400–1,000 procedural cards a head for all eleven styles, brows and facial hair.
  - Strands are painted in the shader, with two shifted highlights and backlight.
  - Up to 32 guide strands swing the cards with the head and the wind.
  - The scalp, buzz cuts and stubble are short hair on the skin.
- **Eyes:** balls with iris, pupil and wet cornea under blinking lid shells with lashes;
  saccades and blinks on their own clock.
- **In the game:** the player's body (the head folded away in first person), the creator's
  preview and the screenshots (`who=`, `skin=`) draw the sculpted people, lit as the world is.
  Review: `docs/review/e7/` (`tools/shots/e7_people.shots`, `hearth_render/tests/body_preview.rs`).
- **Deferred** (PLAN, From E7): face morphs for expressions and lip sync, teeth; detail normals;
  hair coverage, self-shadowing, growth and cutting; cloth simulation; foot IK; impostors; the
  GPU budget on the PC.
- **Five tests:**
  - *Real?* Proportions from adult surveys; UV dose, MED and the burn's and tan's timing from
    photobiology; skin's scattering and specular values from measurements.
  - *Lean?* One mesher (the terrain's) for bodies and garments; one shader body for preview and
    world; hair and skin painted in the shader with no textures; the box figure kept only as
    fallback and for unfitted garments.
  - *Fast?* Meshing off the main thread, one build at a time; three levels of detail; the GPU
    cost still to measure on the PC.
  - *Whole?* The body sim's skin reaches the screen; the creator, the world and screenshots
    share the code. Expressions, hair growth and cloth wait (PLAN).
  - *Organic?* The person's looks come from what they lived: sun, mud, wounds, water.

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
