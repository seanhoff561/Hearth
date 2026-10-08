# Smooth terrain: fill, meshing and shading (Amendment S)

*S0's prototypes and the decision they led to. The production mesher (S2) builds on
`hearth_smooth`; this page grows with S1–S4. The look is set by `art-direction.md`; the plan of
the change by `MIGRATION_SMOOTH.md`.*

## The fill

Every voxel of the 1 m grid keeps its material and gains a **fill**: the signed distance from
its centre to the ground's surface, positive inside, clamped to ±1.5 voxels and stored in a
signed byte (`hearth_smooth::field`). Values run −127..=127, so a step is 1.5 m / 127 ≈ 1.2 cm;
−128 is never written. A voxel is ground exactly when its fill is positive; the surface is the
fill's zero crossing.

The fill must be **distance-like**: its gradient about one long near the surface. A height field
gives `height − y`, which overstates the distance on a slope by 1 / cos(slope) and, once
clamped, shifts the crossings of steep ground; the generator divides by the gradient's length
(as S0's scenes do, `tools/bench/src/smooth/scenes.rs`). Two samples straddling the surface are
then never clamped, so a crossing is placed as exactly as the byte allows; samples a cell or two
away may be, which matters only for normals (below).

## The meshers

All three prototypes are **dual**: a *cell* is the cube between eight neighbouring samples;
every cell the surface crosses gets one vertex, and every grid edge whose two samples straddle
the surface gets a quad joining the vertices of the four cells around it, wound counter-clockwise
seen from outside (`hearth_smooth::mesher`). A quad is cut along the diagonal whose midpoint
lies nearer the surface (so a crest or a valley runs along triangle edges instead of being
notched across), failing that the shorter.

They differ in where a cell's vertex goes:

- **Surface Nets**: at the mean of the cell's edge crossings, then one relaxation step toward
  the mean of its linked neighbours' means (Gibson's constrained net), kept in the cell.
- **Surface Nets with sharp features** ("sharp nets"): between the crossings' mean and the
  feature solve, by the vertex's material **sharpness** times the solve's **trust**. No
  relaxation: it shrinks soft forms (a spoil heap's top, a dune's brink) toward the blobby.
- **Dual Contouring**: at the feature solve everywhere (Ju et al. 2002).

**The feature solve** (`hearth_smooth::qef`) finds the point nearest the planes through the
cell's crossings, each with the surface's normal there, plus a small pull (λ = 0.05) toward the
crossings' mean; a 3 × 3 Cholesky solve. Where planes meet (a crease, a corner) the pull barely
moves the point; along the directions they leave free (the run of a crease, a flat or gently
curved face) it holds the point at the mean. The result is kept 2 % inside the cell, so two
cells' vertices never coincide. Its **trust** falls from 1 to 0 as the planes' mean squared miss
grows from 0.004 to 0.025 cells²: beyond that the cell holds more surface than one vertex can
show (a notch or ledge narrower than the grid), and the solve would only scatter vertices into a
jagged edge.

**Normals at the crossings** for the solve come from crease-keeping gradients
(`Field::gradient_sharp`): central differences blur a crease over the samples beside it, mixing
its two faces' slopes, so on each axis where the forward and backward differences disagree by
more than 0.1 the one-sided ones are combined so the gradient's length comes nearest one, as a
distance field's is. A crossing between corners on different faces of a crease takes the nearer
corner's normal rather than a blend of both.

**Vertex normals** for shading are the opposite of the trilinear blend of the cell's corner
gradients (central differences), smooth from cell to cell. Crisp materials should shade with
their faces: the renderer blends the smooth normal toward the triangle's own by the vertex's
sharpness (on the GPU, the face normal from the screen-space derivatives of the position).

**Materials.** Each vertex blends up to four materials: those of the cell's inside corners, each
weighted by its nearness to the vertex and twice for an upper corner (the material on top shows:
grass over soil, snow over rock), heaviest first, normalized to one. Unused slots repeat the
first material with weight 0.

## Seamless and deterministic by construction

A cube is meshed from its 16³ samples and **two samples of apron** on every side: the cells
straddling its faces reach one sample out, their corners' gradients one more. A cube's region
is the grid edges whose lower sample lies in it, so neighbouring cubes share no edge and no
triangle. Every vertex's arithmetic is done in its own cell's coordinates from the samples around
it (positions are the cell's integer corner plus a fraction), and every quad's split in its base
cell's coordinates, so a cell meshed as part of any cube, with its apron, comes out bit for bit
the same: `hearth_smooth/tests/meshing.rs` meshes a rough field whole and in 27 cubes and finds
the same triangles and the same vertices, normals and weights to the bit, for all three methods.
No transcendental function is used (square roots only), so results do not depend on the
platform's maths library.

## What S0 measured

`bench smooth` (`tools/bench/src/smooth/`) builds the eight test scenes of S §3.1 as fill
fields of 96 × 80 × 96 m, meshes each with the three methods (and the blocky grid of today for
reference), measures them and renders comparison sheets on the CPU. The numbers below are from
the run recorded in `BENCHMARKS.md` (Baseline-S), on the cloud machine's 4-core Xeon.

All eight scenes together (216,752 triangles for each method: the three share their topology,
which the sign of the fill alone decides; they differ only in where vertices go). Cube meshing in
16³ cubes with their aprons, on two threads (two of the four were busy with long tests) and on
one; errors against each scene's true surface (vertex distance; the angle between each
triangle's normal and the true normal at its centre); folded triangles face against their
vertices' normals; holes are open edges away from the region's border (none: watertight).

| Method | Surface cubes/s, 2 threads | per thread | Distance error, mean | Face normal error, mean | Folded triangles | Holes | Bytes per surface cube (24 B a vertex) |
|---|---:|---:|---:|---:|---:|---:|---:|
| Surface Nets | 10,747 | 5,764 | 2.50 cm | 3.0° | 28 | 0 | 9,136 |
| Surface Nets, sharp features | 6,200 | 3,553 | 1.45 cm | 2.4° | 61 | 0 | 9,136 |
| Dual Contouring | 6,698 | 3,136 | 1.41 cm | 3.0° | 542 | 0 | 9,136 |

By scene, distance error (mean / 99th percentile, cm) and folded triangles:

| Scene | Surface Nets | Sharp nets | Dual Contouring |
|---|---|---|---|
| Rolling hills (limestone scarps) | 5.6 / 37.7, 14 | 2.8 / 24.6, 22 | 3.0 / 33.4, 232 |
| Sea cliffs (shale notches) | 5.0 / 102.5, 2 | 3.4 / 74.8, 19 | 2.6 / 50.9, 212 |
| Cave | 1.9 / 12.4, 4 | 0.8 / 6.8, 7 | 0.9 / 9.1, 17 |
| Dune field | 1.0 / 15.9, 2 | 0.6 / 11.3, 2 | 0.5 / 4.3, 14 |
| Riverbank | 1.2 / 17.6, 0 | 0.7 / 10.5, 0 | 0.7 / 10.1, 15 |
| Talus slope | 2.3 / 26.8, 2 | 1.6 / 10.2, 6 | 1.6 / 8.4, 24 |
| Dug pit and spoil | 0.4 / 7.6, 0 | 0.3 / 4.1, 0 | 0.2 / 2.1, 5 |
| Mountain ridge | 2.7 / 15.2, 4 | 1.6 / 9.7, 5 | 1.6 / 9.4, 23 |

The largest errors are where the scenes hold features narrower than a cell (the cliff's 0.9 m
notches, the scarps' feet): no single vertex per cell can be near all of such a surface.

Today's mesher, for scale (`hearth bench --terrain-only`, `BENCHMARKS.md`): a surface cube of
the generated world takes 0.28–0.77 ms on one thread (1,300–3,600 a second) and 8.5 KB (coast)
to 43 KB (forest, with its foliage and models) of GPU memory. The prototypes do not yet sample
light or AO or pack vertices, which S2 adds.

**How they look** (`docs/review/s0/*.jpg`; columns: blocky, Surface Nets, sharp nets, Dual
Contouring; rows: the scene, a close look, its triangles):

- *Surface Nets* is smooth everywhere: hills and dunes are right, but a limestone scarp or a sea
  cliff's ledges melt into soft grooves and a cut pit's rim rounds off. Its relaxation pulls
  convex forms in by a few centimetres (2.5 cm mean error against the true surface, twice the
  others').
- *Dual Contouring* keeps every crease, soft or hard: dune brinks and a pit's rim stay sharp, and
  where a feature is narrower than the grid (the cliff's shale notches, the scarps' feet) one
  vertex per cell cannot hold both its edges, so the solve throws vertices out into spikes,
  sawteeth and folds (542 folded triangles across the scenes, nine times sharp nets' 61).
- *Sharp nets* gives each material its own character: sand, soil and snow soft, limestone and
  granite crisp; where a feature is too narrow for the grid its low trust falls back to the
  smooth mean, so sub-metre notches become soft grooves instead of teeth. It has the lowest
  normal error of the three (2.4°), nearly DC's distance error (1.45 cm against 1.41) and few
  folds.

**What none of them can do**: show a feature smaller than about two voxels crisply. A 0.9 m
shale notch or a 30 cm ledge is the shader's work (the strata of S §4.2, normal maps); the
geometry gives the metre-scale form.

**Cut soil** (the dug pit) rounds under sharp nets, as soil's sharpness (0.2–0.25) says. A fresh
spade cut holds an edge for a while before it crumbles; if the review wants that, S1 can give an
edit's faces extra sharpness that weathers away (or the settling simulation can round them
instead), rather than raising soil's sharpness everywhere.

## The decision

**Surface Nets with sharp features**, as the amendment expected (D222). It is the only one that
makes soft ground soft and rock crisp, it degrades gracefully where the grid is too coarse for a
feature, it is crack-free and deterministic by construction, and it is fast enough by a wide
margin: about 3,500 surface cubes a second on one thread of this machine, against S §12.2's
2,000 on eight cores. The production mesher (S2) keeps its rules (apron of two, cell-local
arithmetic, the diagonal rule, crease-keeping gradients, sharpness × trust) and adds compact
vertices, `meshopt`, light and voxel AO sampled at mesh time, and mid-range simplification.

## Shading: biplanar mapping and height blending

The shading prototype (`tools/bench/src/smooth/shading.rs`, `docs/review/s0/shading.jpg`) puts two
procedural materials, turf and limestone (albedo and height, 256² tiles of 2 m), on the rolling
hills' sharp-nets mesh:

- **Height blending** (each material's height plus its weight; the higher wins within a 0.2
  band) turns the linear blend's smear into a natural edge: turf tufts stand into the rock's
  joints and the rock's raised blocks break through thin turf.
- **Biplanar mapping** (the two projections the normal faces most) against **triplanar** (all
  three): they take 2 and 3 texture samples a material (3.05 and 4.57 a pixel over the view, where two
materials overlap at the edges) and their images differ by 0.56 levels of 255 on average, 2 % of
pixels by more than 4 (the difference panel, ×4). Biplanar is the default (S §4.1); triplanar is not worth its third
  sample even at High.
- Limestone's beds are drawn across the rock in world space, as S2's sub-metre strata will be.
