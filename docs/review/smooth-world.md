# The smooth world: visual review (Amendment S §12.4)

The smooth world against the blocky one it replaces, place by place, kept through S0–S8 and
completed at S8. Each S milestone adds what it changed; S8 sets the whole screenshot suite (v1
§14's cohesion check, the v2 milestones' lists, V2.1's era reviews) beside Baseline-S and checks
the list below.

## What is checked (S §12.4)

No cracks at cube seams or between LOD levels; no shimmering or tiling patterns; soft materials
soft and rock crisp; nothing blobby; beaches, riverbanks and dunes natural; cliffs showing their
strata; caves like caves; clean water edges; forests right near, mid and far; bare winter trees
right; people and animals belonging to the same world as the ground; no colour jump at the LOD
transition. And `art-direction.md`'s checks: no cube in natural things, no tiling repeat at a
glance.

## Baseline-S: the ground before (S0, 2026-10-07)

Rendered on the cloud machine's software device from `tools/shots/s0_baseline.shots` (the same
places are rendered again as the smooth world lands):

| Place | Before |
|---|---|
| Dry grassland by a coast (seed 4, huge planet) | `s0/baseline-grassland.jpg` |
| A stony shore (seed 7) | `s0/baseline-stony_shore.jpg` |
| A river in broadleaf wood (seed 7) | `s0/baseline-river.jpg` |
| Alpine rock and snow (seed 7, huge planet) | `s0/baseline-alpine_rock.jpg` |
| Montane forest slopes (seed 7, huge planet) | `s0/baseline-montane.jpg` |
| A cold desert (seed 7, huge planet) | `s0/baseline-cold_desert.jpg` |

What the baseline shows: every natural surface a staircase of cubes (the alpine slope's
terraces, the shore's steps into the water); plants drawn as crossed sprites standing on the
cubes; distant land the LOD's flat-topped columns. The alpine view overflowed the distant
terrain's quad arena (10.6 M quads against 8.4 M; some far tiles undrawn), a limit S4's LOD work
replaces.

## S0: the meshers compared

`s0/<scene>.jpg` for the eight test scenes (columns: blocky, Surface Nets, Surface Nets with
sharp features, Dual Contouring; rows: the scene, a close look, its triangles) and
`s0/shading.jpg` (turf and limestone: linear against height blending, biplanar against
triplanar). The chosen mesher and why: D222 and `docs/design/smooth-terrain.md`.
