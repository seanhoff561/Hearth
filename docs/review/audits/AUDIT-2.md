# Audit 2 — after the smooth ground, the people and the distant terrain

*2026-10-08, after S4 (Amendment Q §8.2). Covers S1, S2, E7, S3 and S4. An independent
reviewer, given only the charter, re-read the distant terrain's fields and renderer, the mover
on smooth ground, the hair and the ground's editing. Its verified findings are folded in below.*

## Metrics (Q §9), against Audit 1

| What | Now | Audit 1 | Note |
|---|---|---|---|
| Rust in `src/`, 24 crates | 131,300 lines (119,900 without inline test modules) | 123,200 | character 6.2k (+2.8k: the body, hair, eyes, garments), render 13.8k (people, smooth ground, LOD fields), world 7.4k (fill, ground editing), hearth 33.3k |
| Dead-code allowances | 12 | 12 | at the threshold |
| Files over 2,000 lines | 8 | 8 | `hearth_lod/src/lib.rs` reached 2,345 in S4 and was split (`ground.rs`); `client.rs` 4,459 (+69) |
| Restart files | PROGRESS 25 KB → 14 KB after this audit's trim, DECISIONS 22, PLAN 19 | 24 → 12, 20, 19 | P3–S2 entries moved to `docs/history/` |
| Repository | 18 MB tracked | 15 MB | review images of S1–S4 and E7 |
| Unused dependencies, unlicensed assets | none | none | `scripts/lean-check.sh` |
| Frame and tick times | not measurable here (software device) | — | the gate waits for the PC |
| Repetition scores | none yet | none | still on the fix list |

## Performance pass (CPU; the GPU gate needs the PC)
Top three hotspots measured on the cloud machine (dev-opt, one tile at a time):
1. **LOD tiles that grow every real tree** (level 3, 256-block tiles): 35–118 ms each; level 2:
   28–46 ms. The canopy map runs the generator's tree code over the tile.
2. **Far-field tiles** (levels 8–10) reading the coarsest refinement levels: 29–66 ms each,
   built rarely (a few hundred reach the horizon).
3. **Meshing a person** at 6 mm: about 0.7 s on a worker thread, once per appearance and detail.
The LOD selection's 1,600–2,200 tiles build in 6–12 s on four threads for a screenshot; in the
game they stream nearest first. The ground now draws 2,304 triangles a tile (S4): its GPU time
is the first thing to measure on the PC.

## Lean pass — fixed
- **Bug: earth piled over a cave went into it.** `pile` took the lowest open voxel with ground
  under it within some 6 m below, a cave's floor included. It now takes each column's surface
  (searched from the top); regression test `earth_piled_over_a_cave_lies_on_the_surface`.
- **Bug: hair moved differently at different frame rates.** The guides' damping and their pull
  to the style were per frame; they are now per 60th of a second, scaled to the step.
- **Dead code: the LOD quads' water flag** (water is the ground's field since S4), with its
  shader branches; the crown sink's wrapper closure.
- Duplicates: the mover's step-height match (written twice) and its slide test (now
  `too_steep`); the hair's cranium centre; an unread accumulator in the ground's ray cast; the
  renderer's hard-coded tile side.

## Lean pass — for when each file is next touched (PLAN)
- The crown boxes' snow still follows a temperature rule while the ground under them follows
  the cover model's seasons (until S5 replaces the boxes).
- Step heights do not scale with a body's size (a child steps up an adult's rise).
- `Ground::friction` is a slipperiness (0.6 normal, 0.98 ice), a convention from the original
  game; the slope fit 2.25 (1 − s) needs a real friction coefficient with a source.
- Duplicates: the LOD normal packing (ground and canopy), the seasonal range formula, the
  field's gradient in the mover and in `ground.rs` (different steps), the beard's lip zone in
  two places; the canopy's cover read through `GroundVertex::water()`; `level`'s ±3 m reach
  undocumented; a double read in `settle`; a redundant variable in `ground_at`.

## Realism and cohesion pass
- **Two styles side by side:** the people and the ground are smooth and shaded physically; the
  plants, trees, loose stones and built pieces are still voxel boxes (`docs/review/e7/`,
  `docs/review/s2/montane.jpg`). S5, P7, P7G and S6 replace them, in that order.
- **Snow near the player** lies under the smooth surface and shows as patches while the
  distant snow is whole (S7 makes snow fill).
- **Distant terrain** (`docs/review/s4/`): no seam or colour jump at the handoff; mountains and
  coasts read well to the horizon. Mid-distance woodland is a little denser than the trees
  grown near it.
- **People:** no foot IK on slopes yet; first person shows the body (checked).

## Fix list
- **High:** all done above.
- **Medium (PLAN):** the crown boxes' snow; step heights by size; a real friction
  coefficient; the duplicates listed; the GPU measurements on the PC (LOD fields, people, the
  smooth ground); splitting `client.rs` and `workshop.rs`; repetition scores.
- **Low:** the naming and doc items listed.
