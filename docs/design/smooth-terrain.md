# Smooth terrain: fill, meshing and shading (Amendment S)

*S0's prototypes and the decision they led to; S1's fill in the world, its ground families and
its editing; S2's meshes and shading in the game. This page grows with S3–S4.
The look is set by `art-direction.md`; the plan of the change by `MIGRATION_SMOOTH.md`.*

## The fill

Every voxel of the 1 m grid keeps its material and gains a **fill**: the signed distance from
its centre to the ground's surface, positive inside, clamped to ±1.5 voxels and stored in a
signed byte (`hearth_smooth::field`, and in the world `hearth_world::fill`). Values run
−127..=127, so a step is 1.5 m / 127 ≈ 1.2 cm; −128 is never a depth (in the world it marks a
voxel whose state changed and whose fill was not set since). The surface is the fill's zero
crossing.

The fill must be **distance-like**: its gradient about one long near the surface. A height field
gives `height − y`, which overstates the distance on a slope by 1 / cos(slope) and, once
clamped, shifts the crossings of steep ground; the generator divides by the gradient's length
(as S0's scenes do, `tools/bench/src/smooth/scenes.rs`). Two samples straddling the surface are
then never clamped, so a crossing is placed as exactly as the byte allows; samples a cell or two
away may be, which matters only for normals (below).

## The fill in the world (S1)

- **Kinds** (S §2.1): each block says what it is to the smooth world (`kind` in
  `blocks/_templates.json`: rock and soil natural, leaves and foliage, anything else from what
  it is: a fluid fluid, a block without collision empty, the rest a structure). The registry
  keeps it as the state flag `NATURAL`.
- **Storage:** a cube keeps a fill array only where the surface passes through it; elsewhere a
  voxel's fill is what its state says (natural ground full, anything else empty). Cubes write
  it after their light, flagged in their first byte, so it travels with them to the client and
  into region files.
- **Generation** (S §2.2): the generator keeps each voxel's depth from its continuous tests:
  the terrain's height, across the slope (`(height − y − ½) / √(1 + slope²)`, the distance to
  an inclined plane); the cliffs' shaping; the caves' and caverns' walls, cut out to a voxel
  and a half beyond them. Deposits, water and trees change states afterwards; the fill is made
  to agree with what each voxel ended as. Unchanged cubes regenerate the same fill from the
  seed; the surface it gives lies within 6 cm of the terrain's height
  (`surface_cubes_carry_the_grounds_fill`).
- **What it means:** anything but natural ground is outside the ground. Natural ground is
  usually more than half full, but may be less, its surface below the voxel's centre (dug, or a
  thin layer). A voxel's **occupancy**, the share of it that is ground, is ½ + its depth,
  clamped: full half a voxel in, empty half a voxel out.
- **Saves** (S §2.4, save format 8): the player's changes keep their fill (`blocks.json`'s
  `fills`). A change saved before takes the regenerated ground's fill, at least half in (or
  out), so its faces meet the ground about it.

## Ground families (S §2.3)

`materials/reference.ron`'s `ground` gives each family of natural ground its behaviour; the
first family whose filter matches a material is its own, and the lint holds every natural
block's material to one.

| Family | Sharpness | Repose, dry / rain-wet | Friction, dry / wet / iced | Footsteps |
|---|---:|---|---|---|
| snow | 0.0 | 38° / 30° | 0.3 / 0.2 / 0.1 | snow |
| ice | 0.6 | — | 0.1 / 0.05 / 0.05 | glass |
| sand | 0.05 | 34° / 40° | 0.55 / 0.6 / 0.15 | sand |
| gravel | 0.3 | 40° / 41° | 0.6 / 0.55 / 0.15 | gravel |
| volcanic ash and loess | 0.2 | 35° / 25° | 0.55 / 0.35 / 0.1 | sand |
| clay | 0.35 | 40° / 20° | 0.6 / 0.25 / 0.1 | mud |
| peat | 0.15 | 40° / 30° | 0.5 / 0.35 / 0.1 | moss |
| soil and loose earth | 0.2 | 37° / 42° | 0.6 / 0.4 / 0.1 | soil |
| rock | 0.9 | — (stands) | 0.7 / 0.5 / 0.1 | stone |

Moist grains hold steeper than dry by capillary cohesion; wet clay and loess lose their
strength. The natural blocks' footsteps come from their family. Each family's sources are in
the file; the clay, peat, ash and loess and soil values are marked uncertain.

## Looking, digging and slumping (S §8.3–8.4)

`hearth_world::ground` treats the ground as the fill's field: trilinear between the voxels'
centres, unloaded ground outside.

- **Looking:** a look marches the field a tenth of a metre at a time and bisects to the
  crossing, to a millimetre; the normal is the field's gradient (`raycast`).
- **Digging** takes a volume from a bowl sunk into the surface at the point looked at, the
  voxels nearest its middle giving most (`dig`). It takes whole steps of the fill (some 12
  litres of a 1 m voxel); a stroke smaller carries the rest to the next. A dig process moves a
  cubic metre in all, stroke by stroke as the work goes (`workshop/ground.rs`); work taken up
  again has its share dug already.
- **Piling** puts a volume into the lowest open voxels about a point, each filled from below,
  reaching further where something stands in the way; it buries grass and low plants (`pile`).
  What is laid on top is what a voxel shows: a voxel is of one material.
- **Slumping** (`settle`): over the columns about a change, ground passes from a column's top
  to a lower neighbour's top while the step between them is steeper than the top's angle of
  repose, half the excess at a time, until nothing moves. Only loose ground flows (spoil, sand,
  gravel, ash, snow); intact earth stands in a pit's wall, rock never moves. In rain the wet
  angles hold.
- After an edit the fill about it is set again from the occupancies (`refill`): a part-full
  voxel holds its surface where its share puts it, a full one is as deep as its emptiest
  neighbour leaves it, an empty one as far out as its fullest neighbour.
- **Volume is conserved:** dug is lost, piled is gained, slumped is moved. Measured
  (`hearth_world/tests/ground.rs`): forty strokes of 6 litres dig 0.24 m³ and put back fill
  the hole to within 2 litres; four cubic metres of sand tipped in one place stand at 76° and
  settle to 34.3° in 18 moves; a 14 m³ pit dug straight down in sand slumps to 34.7° walls; a
  look meets the ground within a centimetre.

## The smooth ground in the game (S2)

- **Meshing** (`hearth_render::smooth`): the server meshes each cube's natural ground from a
  window of its fill and two voxels of apron (`window`) with sharp nets, alongside the blocks'
  faces and models, which no longer draw natural ground. The window is in its own coordinates
  and each vertex's place is its cell's integer corner plus its place in the cell, rounded
  apart, so a vertex meshed in two cubes comes out the same far from the world's origin.
- **Vertices** (24 bytes): position to half a millimetre in the cube, an octahedral normal, four
  material slots and their weights, light, ambient occlusion, sharpness, snow cover and the
  column's climate (for grass's colour). A triangle whose corners name different materials gets
  corners of its own carrying their union, so the shader can take the materials from one corner
  and blend by the three's weights; slivers under 8 % are dropped. About 25 bytes a triangle.
- **Light** is the open voxels about a vertex by nearness, taken half a voxel out along its
  normal; **ambient occlusion** the ground found along five short rays into the hemisphere about
  it.
- **Materials** (`scene::ground_materials`): one slot per natural block (105), coloured by its
  material's two colours, roughness and pattern (the grain's size and how its relief stands up in
  blending); a block that names no material by its map colour. Grass is its place's and season's
  colour at a sward's albedo, broken by thinner places where the soil shows.
- **Shading** (`terrain.wgsl`, `fs_smooth`): each material is 3D value noise at its grain in
  world space (three octaves, the finer fading out with distance before they would shimmer), so
  there is no projection to choose and nothing tiles. Materials meet by height blending: each
  weight raised by its material's relief there, the highest winning within a narrow band.
  Bedded rock (layered and banded patterns) shows beds of 12–25 cm across its faces, wavering
  and fading with distance. Crisp materials shade toward their faces' normals, soft ones with the
  smooth normal. **Overlays:** wet ground after rain at 0.6 of its dry albedo (the environment's
  `wetness`, a film of rain that dries by warmth, dryness and wind); snow cover, where snow
  layers lie on the ground, its edge broken by the ground's relief (snow layers on natural
  ground are no longer drawn as blocks; S7 makes snow fill).
- **Drawing:** smooth meshes live in their own arenas (vertices, and indices as `u16` in pairs)
  and are drawn by indexed indirect draws from the CPU's list of visible cubes (frustum and cave
  culled), first in the GPU-culled path's first phase, so their depth feeds the occlusion
  pyramid; they are not yet occlusion-culled themselves.
- **Looking at it:** the client's look meets the smooth surface by the field (12 µs a pick).
- **Measured** (`hearth bench --terrain-only`, `BENCHMARKS.md`): a surface cube meshes in
  0.28–0.87 ms on one thread, as the blocks did; cube memory grows by the fill array where the
  surface passes (+4 KiB); mesh bytes fall in the mountains (33.8 → 25.5 KiB a surface cube) and
  grow on coasts (8.9 → 21.6 KiB). The look on the software device: `docs/review/s2/`.

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
