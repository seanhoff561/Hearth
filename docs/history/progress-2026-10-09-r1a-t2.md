# PROGRESS: R1a and T2 (2026-10-09)

*Moved from `PROGRESS.md` when G1b's ground was done; the milestones' entries as they stood.*

### R1a — the sun's shadows (2026-10-09, D298, RGA-1's first gap)
- **Cascaded shadow maps** (`hearth_render::shadow`, `terrain/casters.rs`, `common.wgsl`): four
  spheres about the camera (12–320 blocks; texels 1.2–31 cm at 2048²) and one over the distant
  terrain (1–12 km), their texels still as the camera walks and turns (snapped centres, the
  light moved on in 0.23° steps), the outer ones redrawn by turns. Cast: the far side of solid
  cubes, the smooth ground and the distant height fields, plants and leaves cut to their texels
  (fallen ones left out), figures, people's bodies (in first person with the head), the mesh
  trees; the near world into the near cascades (the cubes looked up in each one's region), the
  distant terrain into the far one, where a near point counts only what lies beyond its near
  cascade's reach. Received by the terrain, water and ice, the distant terrain, figures, skin,
  hair and eyes, trees: each comparison against the surface's own plane, the edges as soft as
  the sun's disc makes them in the two nearest cascades (to 3.7 cm); never deep in enclosed
  places. Option *Shadows* (off, low, medium, high; presets), screenshot `shadows=`, bench
  `--shadows`, F3 line.
- **Tests**: `sun_shadows` (a slab over the ground: its shadow 0.161 against 0.567 without the
  maps; the open ground 0.592 both ways); `shadow.rs` (the light's basis, the snapping, the
  light's steps and the cascades' turns, the casters' selection, every caster in its cascade's
  box); `options` (an older file's shadows by its preset).
- **Pictures**: the realism suite re-rendered with shadows (`docs/review/realism/suite/`); low
  sun before and after, a person, a ridge at dawn, the soft edges (`docs/review/r1a/`).
- **Accepted** (PLAN, R1a): the suite's low-sun shots shadowed — trees, tussocks, stones and
  people cast at 7 h, the distant terrain at 5 h; the GPU budget (≤ 1.5 ms at 1080p on the
  RTX 4060) is the owner's check: on the software device the shadow passes take half the main
  terrain pass's time (1.54 M triangles a frame against 0.81 M without, 5,316 draws against
  2,492), the frame thread +1.5 ms (prepare: terrain 1.02 → 2.52 ms; `BENCHMARKS.md`).
- **Real?** Shadows from the sun's own direction and disc; image statistics (medians over the
  biomes): spectral α underfoot 1.89 → 2.26 (the Earth's 1.8–2.4), at eye height 1.23 → 1.32,
  colourfulness at eye height 60 → 54 (15–60); a low sun's darkest tones underfoot 0.34 → 0.14.
  **Lean?** One module and a submodule; the sky-light gate kept only where no map reaches;
  `terrain.rs` split (the arena, the casters) under 2,000 lines. **Fast?** Within the CPU
  budget here; the GPU on the PC. **Whole?** Everything lit by the sun receives and nearly
  everything casts (not hair, eyes or the distant crowns' boxes, D298). **Organic?** The
  penumbra's spiral turned by a hash fixed in the world.
- Next: G1a, the relief from 30 m to 30 km (done).

### T2 — the realism gap analysis (2026-10-09, Amendment T §3, D296–D297)
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
