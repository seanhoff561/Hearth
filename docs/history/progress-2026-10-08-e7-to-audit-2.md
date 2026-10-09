# Progress, 2026-10-08: S3 to Audit 2

Moved from `PROGRESS.md` (2026-10-09, T1) to keep it to the current status (Amendment Q §5.4):
the milestone records of E7, S3, S4 and Audit 2 as they stood when each was done.

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
