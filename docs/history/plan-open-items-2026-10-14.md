# PLAN: open items from S1–S4, E7 and Audit 2 (as of 2026-10-14)

*Moved from `PLAN.md` when G1b's ground was done; each to be taken up when its file is next touched.*

### From S1 (open)
- Building pieces, tilling and the structure check read the fill (S3's Level ground, a built
  piece's buried skirt); collision against the field is S3's.
- A fresh cut's faces holding sharper than soil's sharpness, weathering soft (S0's note).
- The bot suite (it digs) with the full suite.

### From S2 (open)
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

### From E7 (open; E7 done 2026-10-08, D266–D270, `docs/design/people.md`)
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

### From S3 (open; S3 done 2026-10-08, D271–D273)
- The grip by wetness (the ground families' wet friction) and footwear; loose footing slipping
  now and then on scree and sand, and moving disturbing it (S §8.2, §8.4); cutting steps.
- The player's footprints (as the animals' signs); dust kicked up.
- Animals' capsules against the field near players; trees' skeleton capsules (S5).
- Level ground's footprint chosen (a 3 m square now), and its preview with the dig preview.
- The budgets (300 animals, movement substeps) measured on the PC.

### From S4 (open; S4 done 2026-10-08, D274–D278)
- The v1 §8.4 frame targets and the LOD's GPU time with the height fields (2,304 triangles a
  tile, ground and canopy frustum-culled only) measured on the PC; fewer triangles for flat
  tiles if it needs them.
- Overhangs as smooth shelves (S §5); the crown boxes' snow by the seasons; impostors between
  boxes and canopy (S5).

### From Audit 2 (`docs/review/audits/AUDIT-2.md`)
- Step heights scaled by body size; `Ground::friction` (a slipperiness) replaced by a real
  friction coefficient with a source for the slope fit.
- Duplicates: LOD normal packing, the seasonal range, two field gradients, the beard's lip
  zone; a `cover()` accessor for the canopy; `level`'s reach documented.
