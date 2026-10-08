# AMENDMENT S — Smooth Voxel World
## Natural terrain, real foliage and lifelike forms on the same voxel grid

*Received from the owner on 2026-10-07, during V2-12; recorded verbatim. Precedence and ordering
are in §0; the milestones S0–S8 are in `PLAN.md` after V2-12.*

You are the autonomous engine programmer building `hearth`. You've been following the v1 build prompt, v2 ("Earth, Transposed"), V2.1 ("Realistic Humans") with Addenda A and B, and you have Amendment R filed for the end of the project. This document is **Amendment S**. It changes how the world *looks and feels underfoot*: natural terrain becomes smooth and lifelike instead of cubic, leaves become real foliage, trees get real trunks and branches, and characters, animals and objects get smooth forms that match. **The world stays a voxel world**: the 1-metre grid remains the source of truth for every simulation system.

Read all of it before changing code. All earlier ground rules still apply: clean-room, data-driven, deterministic, multiplayer-ready, always green, no stubs, performance as a feature, the V2.1 content rules.

---

## 0. Precedence, ordering and first actions

### 0.1 What this amends
- **Replaces** for natural terrain: v1 §7.2 (cube meshing), the per-vertex block-corner AO in v1 §7.2, the pixel-art terrain textures in v1 §10, and v1's AABB-on-cubes collision (v1 §3) for terrain contact.
- **Modifies:** v1 §7.1 (voxel data gains a fill value), v1 §8 (LOD meshes become smooth; segment heights become sub-block), v1 §9.3 (water surfaces), v2 §5.6 and §11.2 (terrain surface detail, digging and piles now sculpt smooth volumes), v2 §6.2 and §6.7 (trees and foliage rendering), v2 §7.2 and §9.2 and V2.1 §4.3 (blocky cuboid rigs become smooth procedural bodies), v2 §14 (building pieces meet smooth ground).
- **Unchanged:** cubic chunks and unlimited height, the planet and climate model, geology and stratigraphy, ecosystems, physiology, crafting, knowledge, structural rules, humans, births, multiplayer plans. These systems run on materials, volumes and positions, not on how cubes are drawn.

### 0.2 Ordering
1. Finish your current milestone to a green, committed state.
2. Insert **S0–S8** (§14) into `PLAN.md` **immediately after** it, ahead of the remaining H milestones. This is engine work, and every later milestone that adds textures, trees, building pieces, characters or screenshots would otherwise be built on the old look.
3. Amendment R stays last (after V2-16), unchanged, except that its guide and trailer now describe and show the smooth world.

### 0.3 First actions
1. Read `PROGRESS.md`, `PLAN.md`, `DECISIONS.md`, `git log --oneline -40`; run `scripts/check.sh` and `hearth content lint`.
2. Write `MIGRATION_SMOOTH.md`: every affected subsystem marked **Keep / Modify / Replace**, with a note.
3. Start S0, which captures the **performance baseline** of the current renderer before anything changes (§12.1). All of S's performance targets are relative to that baseline.

---

## 1. Goal and art direction

### 1.1 The look: "stylized realism"
The target is a world that reads as **real places**, not a toy: soft hills, dunes and snowdrifts; crisp rock cliffs and ledges; sand that slumps; muddy riverbanks; real-looking trees with leafy canopies; people and animals with believable shapes. It should still have a coherent, slightly stylized identity (clean forms, readable silhouettes, careful color), not photo-scanned realism. Write `docs/design/art-direction.md` defining this: references by description (no other games' assets), palette principles, material families, how sharp vs soft each material is, scale rules (1 block = 1 m), and the "nothing looks like a cube unless it's a built or cut thing" rule.

### 1.2 What becomes what
| Thing | Before | After |
|---|---|---|
| Natural ground (soils, sand, gravel, clay, mud, snow, rock, ores in rock) | Cubes | **Smooth surfaces** through the voxel grid, with per-material sharpness (rock crisp, sand soft) |
| Built and worked pieces (logs in walls, planks, posts, beams, bricks, dressed stone, mudbrick, thatch) | Cubes and sub-block models | **Crisp, grid-snapped models** (real buildings are made of discrete pieces) with natural detail (bevels, bark, grain, mortar lines) |
| Leaves | Cutout leaf blocks | **Leaf clusters** on real branches; impostors in the distance |
| Trunks and branches | Log blocks and sub-block branch pieces | **Smooth procedural meshes** from the tree growth model |
| Grass, flowers, small plants | Cross-sprites | Instanced meshes and cards with wind (as planned), seated on the smooth ground |
| Water | Block levels | Same fluid levels, **smooth surfaces** |
| Characters, animals | Cuboid rigs | **Smooth procedural bodies** on the same skeletons (S6) |
| Tools, items, loose stones, logs, carcasses | Sub-block cuboid models | Smooth meshes from form × material |
| Distant terrain (LOD) | Blocky columns | Smooth surfaces matching the near field |

### 1.3 Non-negotiables
- **Still voxel.** Every simulation system (lighting, fluids, structure, digging volumes, ecology, nav grids, saves, networking) keeps working on the 1 m grid. Smoothness is a property of how fill values are stored, meshed, collided with and textured, never a separate world.
- **Voxel size stays 1 m.** Don't change the grid resolution. If S0 shows 1 m can't reach the art direction, write a proposal with prototype screenshots and benchmark numbers in `DECISIONS.md` under "Needs owner confirmation", and continue at 1 m.
- **Similar or better quality and performance** than the baseline (§12). Measured, not assumed.
- Deterministic and **multiplayer-ready**: collision and editing must give identical results on server and clients.

---

## 2. Voxel data: material plus fill

### 2.1 Representation
- Each voxel keeps its **material** (palette-compressed, as now) and gains a **fill value**: a signed, quantized density (8 bits) describing how far the voxel center is inside (+) or outside (−) the surface, clamped to about ±1.5 voxels. The surface is the zero crossing. This is what lets a 1 m grid describe a surface at sub-metre precision.
- **Uniform cubes stay free:** cubes entirely solid or entirely empty store no fill array (as now with single-value palettes). Fill arrays exist only for surface cubes, compressed in memory and on disk.
- Each voxel also has a **kind** derived from its material: *natural* (meshed smooth), *structure* (a built piece occupies it; §6), *fluid*, *foliage occupancy* (§7), *empty*. Kinds come from data.

### 2.2 Worldgen gives fill for free
The generator already computes a continuous density field (v1 §6.1). Store its quantized value as the fill of generated cubes. For unmodified cubes, fill can be **regenerated from the seed on demand**, so saves and the network only carry fill for cubes that were edited (§2.4).

### 2.3 Material properties added (data)
In `data/hearth/materials/`, each natural material gains: `sharpness` (0 soft … 1 crisp), `angle_of_repose` (dry and wet), `friction` (dry, wet, icy), `slump` (whether it flows when unstable: sand, gravel, soil, snow, mud yes; rock no), `walk_sound`, and a `surface_recipe` for its textures (§4.2). Tune with realistic values (dry sand ≈ 34°, gravel ≈ 40°, etc.), mark uncertain values per v2 §3.1.

### 2.4 Saves and network
- Edited cubes store material + fill deltas; unmodified ones are regenerated. Bump the save format with a migration: existing edited cubes get fill = full solid or empty from their old material state, then a one-time smoothing pass blends edited regions with the regenerated field so old dev saves look right.
- Network cube deltas (Amendment R §3.2) include fill; keep them compressed and hash-checked as planned.

---

## 3. Smooth meshing

### 3.1 Choose the algorithm by prototype (S0)
Prototype at least these on a fixed set of test scenes (rolling hills, sea cliffs, a cave, a sand dune field, a riverbank, a talus slope, a dug pit with a spoil pile, a mountain ridge):
1. **Surface Nets** (one vertex per surface cell, averaged edge crossings), with a short relaxation step.
2. **Surface Nets with sharp features**: vertex placed by blending between the averaged position and a feature-preserving solve (QEF from density-gradient normals), weighted by material `sharpness`.
3. **Dual Contouring** with QEF.
Compare image quality (soft materials soft, rock crisp, no blobby look, no pinches or holes), triangle counts, meshing speed and robustness. Pick one, record the choice and the evidence (screenshots and numbers) in `DECISIONS.md`. Option 2 is the expected winner; it's simple, fast, crack-free and handles both sand and cliffs.

### 3.2 Requirements for the chosen mesher
- **Seamless across cubes:** mesh each cube with a 1–2 voxel apron from its neighbors (6 faces, edges and corners). If a neighbor isn't loaded, sample the worldgen field for it (deterministic), and mark the mesh for a quick re-mesh when the neighbor loads only if it was edited.
- **Normals** from the density gradient (smooth materials) and from the feature solve at creases (sharp materials).
- **Material blending:** each vertex carries up to 4 material ids with weights (from the solid voxels around it), so transitions blend (grass into dirt into rock) instead of hard-switching. Weights favor the material on top for the right look (snow over rock, moss in hollows).
- **Compact vertices:** position quantized within the cube (e.g., 10-10-10 bits), octahedral-encoded normal, packed material ids and weights, packed light. Indexed meshes with vertex-cache and overdraw optimization (the `meshopt` crate). Target ≤ ~24 bytes per vertex and report the real average.
- **Threaded and prioritized** exactly like the current meshing (distance and view priority, cancellation, per-frame upload budget). Player edits near the camera re-mesh in the same frame when possible, as now (no visible lag after digging).
- **Simplification for mid-range:** cubes beyond a distance threshold are meshed at reduced detail (simplified with error bounds, or meshed from a 2× downsampled field), before the LOD system takes over (§5).
- **Optional GPU path:** a compute-shader mesher is allowed if it measurably beats the CPU path; keep the CPU path as the reference and for the server.
- The existing **GPU-driven culling** (frustum, Hi-Z occlusion, cube visibility graph, indirect draws, shared buffers) applies unchanged to smooth meshes.

### 3.3 Debug views
Wireframe, normals, material weights, fill-value slices, cube boundaries, simplification level, and a **"blocky" debug render** of the raw material grid. The blocky view stays developer-only: it can't be a player option, because collision and editing follow the smooth surface and a blocky view would show feet floating or sinking by up to half a metre.

---

## 4. Materials, textures and shading

### 4.1 Shading model
- PBR materials (albedo, normal, roughness, height, AO) blended per pixel by the vertex material weights, using **height-based blending** so transitions look natural (pebbles poke through sand, snow fills hollows first).
- **Biplanar mapping** (two projections chosen by the normal) as the default, cheaper than full triplanar with near-identical results; triplanar on High if it measurably looks better.
- Limit to **3 blended materials per pixel**, with the rest dropped by weight.
- Optional **parallax occlusion** on High/Ultra for rock and gravel.
- **Anti-tiling:** world-space macro variation (low-frequency color and roughness noise per material), detail textures at close range, and texture bombing or stochastic sampling for large flat areas.
- **Distance fade:** beyond a set distance, shading fades to each material's average color and roughness, which is exactly what the LOD terrain uses (§5), so the near/far transition can't change color.

### 4.2 Procedural PBR materials (`hearth_texgen` rewrite for terrain)
- Each natural material's `surface_recipe` generates its PBR set at build time: layered noise (fBm, cellular for cracks and pebbles, flow noise for streaks), grain size, strata banding, crack networks, and color from the material's real appearance. Typical resolution 1024² for close-up materials, with full mip chains.
- Compress for the GPU (BC7 for color, BC5 for normals; BC1/BC4 where enough) at build time; keep VRAM within budget (§12).
- Weather and ecology overlays happen **in the shader**, not in textures: wetness (darker, glossier after rain, v1 §9.4), snow cover by slope and accumulation, moss and lichen by moisture and shade, leaf litter in autumn, burn scorch after wildfire, frost.
- **Stratigraphy finer than a metre:** for layered rocks (sandstone, shale, limestone, chalk), the shader samples the geology's layer function (v2 §5.1) in world space to draw thin bands across cliff faces, so cliffs read like real rock even though voxels are 1 m.
- Ore and mineral visibility: ore voxels blend their own material (green copper stains, rusty iron) into the rock surface so the prospecting cues from v2 §13.3 still read.

### 4.3 Lighting and AO
- Light stays on the voxel grid (sky and block light, v1 §7.1). Vertices sample it **trilinearly** from the surrounding voxels for smooth gradients.
- Replace corner-based block AO with **voxel AO from the fill field**, sampled at mesh time (a handful of cone samples into the density grid per vertex), plus optional screen-space GTAO on High.
- Shadows, atmosphere, fog, water and the rest of the v1 §9 pipeline are unchanged.

### 4.4 Resource packs
- Introduce a **material pack** format (data + textures) for terrain PBR materials, loaded through the existing pack system with hot reload.
- The vanilla-layout resource pack loader stays for UI, icons and built-piece textures only. Document that terrain uses material packs.

---

## 5. Distant terrain (LOD) to match

- Keep the v1 §8 LOD data model, generation and caching. Change **segment heights to fixed-point** (e.g., 1/16 block), taken from the fill field, so the LOD knows where the surface really is between grid lines.
- Mesh LOD tiles as **smooth heightfield surfaces** (interpolated heights, normals from the height field, seam-free stitching between levels with the existing skirts), with overhang segments as smooth shelves.
- Shade LOD with the same material averages and the same weather and ecology overlays as near terrain (§4.1 distance fade), so there's no color or brightness jump. Keep the dithered crossfade.
- Forest canopies in LOD use per-species canopy shapes and seasonal colors (§7.4), not flat colored columns.
- Re-run the v1 §8.4 LOD targets; they must still pass.

---

## 6. Built pieces on smooth ground

- Construction pieces (v2 §14) stay **grid-snapped models** occupying voxels of kind *structure*. The terrain mesher treats structure voxels as empty, so the ground surface stays closed around them.
- **Foundations and contact:** every piece that touches the ground (posts, sills, foundation stones, wall bottoms) has a short **buried skirt** below its base so there are no gaps where the smooth ground meets it. A **Level ground** action (with a digging tool) flattens the footprint before building by really moving soil (volume goes to a spoil pile, v2 §11.2).
- **Visual polish:** bevelled edges, wood grain and bark, mortar and stone variation, weathering by age and climate (moss on north faces in wet climates, sun-bleaching, soot above hearths), via the same shader overlays.
- **Cut stone and quarried faces:** quarrying rock produces flat, tool-marked faces (crisp, as worked stone is), while natural rock stays naturally sharp-but-irregular.
- Structural analysis (v2 §14.2) is unchanged. Natural terrain anchoring uses voxels whose fill is above the surface threshold.

---

## 7. Trees and foliage

### 7.1 Trunks and branches
- The v2 §6.2 tree growth model (species skeletons from space colonization or L-systems) now produces **smooth meshes**: generalized cylinders with species taper, root flares and buttresses, knots and branch collars, with bark textures by species (procedural recipes like §4.2, mapped along the trunk).
- The voxel grid still records **trunk and thick-branch occupancy** for light, fluids, structure, felling and the ecology. Collision uses the skeleton's capsules (§8.1), not voxel cubes.
- **Felling:** the cut tree falls as a rigid body built from its skeleton (v2 §6.2), breaking thin branches as it lands; limbing and bucking produce log items with matching meshes.

### 7.2 Leaves
- Leaves are **clusters** attached to terminal twigs: small meshes of leaf cards per species (broad oak leaves, birch leaves, needle sprays, palm fronds), alpha-tested with alpha-to-coverage, two-sided, with simple translucency so canopies glow when backlit at sunrise and sunset.
- **Foliage occupancy** voxels (kind *foliage*) store canopy density for passability, slowdown, noise, hiding and **partial sky-light reduction** (realistic dappled shade; v2 §6.2). Rendering and simulation agree because both come from the same growth model.
- **Wind:** hierarchical sway (trunk, branches, twigs, leaves) by species stiffness and the v1 wind field.
- **Seasons:** leaf color, flowering, fruiting and leaf drop per species from v2 §4.3 phenology. Bare deciduous trees show their real branch structure in winter.
- **LOD chain:** full clusters near → merged and simplified cluster meshes mid-range → **octahedral impostors** per species, age class and season far away → LOD canopy surface at the horizon. Transitions dithered.

### 7.3 Ground cover
Grass, flowers, ferns, shrubs and small plants stay GPU-instanced (v2 §6.7), now **seated on the smooth surface** (snap to the mesh height and align to its normal) and **bent by the player and animals** passing through (a cheap interaction texture). Shrubs and bushes become small instanced meshes with leaf clusters, still passable per v2.

### 7.4 Budgets
A foliage density setting per graphics preset. Report triangle and draw counts for forest scenes in benchmarks (§12). Dense rainforest at High must meet the frame targets.

---

## 8. Movement, collision and interaction on smooth ground

### 8.1 Collision (authoritative on server and client)
- **Characters and animals collide with the density field directly:** a capsule against the trilinear-interpolated fill field, resolved by sampling distance and gradient with iterative depenetration. No terrain collision meshes are needed, so the server doesn't have to mesh terrain, and results are deterministic everywhere.
- Trees collide via skeleton capsules (§7.1), built pieces via their own collision shapes (boxes and hulls from data), items and debris via simple shapes.
- Re-derive player body dimensions from v2 §9.2 (realistic shoulder width and height) instead of v1's box.
- Many-animal performance: animals far from players use cheap ground queries (height and normal sampling); full capsule resolution near players, within the v2 §21 budgets.

### 8.2 Walking on slopes (realistic)
- Walk smoothly up and down slopes: no step-hopping on natural ground. A small step-up height remains for ledges and built steps.
- **Speed and effort by slope:** uphill slows you and costs more energy (feeds v2 §9.3 metabolism); steep downhill slows you for control.
- **Maximum walkable slope** depends on material friction, wetness, ice and footwear (v2 §10.3). Beyond it you **slide**, faster on wet clay, snow and ice, and you can only go up by climbing (v2 §9.2) or cutting steps.
- **Loose ground:** on scree and sand, footing occasionally slips and moving disturbs the surface (§8.4).
- Footstep sounds, dust and footprints (v2 §7.6 decals) come from the material under the foot.

### 8.3 Targeting and editing
- **Raycast** through the voxel grid with a sub-voxel root-find in cells that contain the surface, returning the exact hit point, normal and material.
- **Digging and gathering sculpt:** each dig action removes the **real volume** its process defines (v2 §11.2: a basket load, a stone maul blow, an antler pick stroke) as a tool-shaped brush aligned to the surface normal at the hit point. Yields are the removed volume × material density, so **mass is conserved**: digging and refilling a hole returns the same material.
- **Placing loose material** (soil, sand, gravel, snow, clay) adds volume as a pile at the target, shaped by the material.
- **Preview:** before an action, a soft highlight shows the region that will change, replacing the cube outline.
- Built pieces still place on the grid with the existing snapping and structural preview (v2 §14.2).

### 8.4 Slumping and settling (angle of repose)
When loose materials (sand, gravel, soil, mud, snow) are disturbed or piled past their angle of repose (dry or wet value), a local, budgeted **settling simulation** moves fill to lower neighbors until slopes are stable, conserving volume. This makes spoil piles, dug pit walls, dunes and snowdrifts behave realistically. Rock never slumps; unsupported rock overhangs follow the existing structural collapse rules (v2 §14.2). Run settling only where something changed (never globally), on the server, deterministically.

### 8.5 Navigation for animals and people
Rebuild the navigation grid from the smooth surface: walkable cells by slope against each species' climbing ability, costs by slope, material and wetness, jump and drop links at ledges. Herds favor gentle routes and game trails form along them (v2 §5.6). Humans (V2.1) use the same grid with human abilities.

---

## 9. Water, snow, ice and caves

- **Water:** the fluid simulation stays level-based on the grid. Render its surface **smooth**: corner heights interpolated from neighboring water levels, smooth slopes on flowing water, proper waterfalls, and shorelines that meet the smooth ground cleanly (the existing depth-based foam and soft edges, v1 §9.3). Puddles and floods follow the smooth terrain.
- **Snow:** snowfall adds snow fill on top of the surface (accumulating in hollows and on leeward sides by wind), melts by temperature, and drifts by wind. Snow layers become smooth blankets instead of slabs. Walking leaves deep footprints in fresh snow.
- **Ice:** lake and river ice forms as a smooth sheet; glaciers are sharp-ish packed-ice material with crevasses.
- **Caves:** cave walls use the rock's sharpness (crisp fractured limestone, smooth water-worn passages where streams run), with the shader's stratigraphy banding and wet sheen. Giant caverns (v1 §6) should look spectacular.

---

## 10. Bodies, animals and objects (smooth forms to match)

### 10.1 People and animals
- Keep the skeletons, body plans, procedural animation and gait systems (v2 §7.2). Replace cuboid body parts with **smooth procedural skinned meshes**: each body plan defines its shape as blended primitives per bone (capsules and ellipsoids with smooth blending), meshed with the same surface mesher at fine resolution (a few centimetres), then skinned to the skeleton. Do this at load time per **phenotype bucket**, cached, not per frame.
- **Genetics drives shape** (V2.1 §4.3): height, build, proportions, facial structure parameters, hair form, and sex dimorphism feed the shape parameters, so relatives visibly resemble each other and children grow into adults smoothly (V2.1 §7.2).
- **Faces** get simple expressive features (eyes, brows, mouth shapes) driven by the emotion system (V2.1 §5.2), so feelings read at conversation distance.
- **Coats and skin:** procedural textures from the existing coat and pigmentation recipes, UV-mapped on the generated meshes. Hair and fur as shell or card layers on the mid and high presets, with cheaper versions on Low.
- **Clothing** (v2 §10.3) as layered meshes fitted to the body shape.
- **LODs:** several mesh LODs per bucket, then impostors for distant herds and flocks (v2 §15). Crowds and herds must stay within the v2 §21 budgets.
- Keep the style consistent with §1.1: smooth, believable forms with clean silhouettes, not photoreal.

### 10.2 Items and objects
Tools, weapons, containers, loose stones, logs, carcasses and debris become smooth meshes generated from **form × material** (v2 §3.1): knapped stone with facet patterns, hafts with bindings, pottery with real profiles. Generate automatically from item data so new materials and forms still need no hand-made models.

---

## 11. UI, journal and docs touched by this change
- Replace cube outlines with the §8.3 preview highlight everywhere. Update the builder's view (v2 §14.2) to show contact with smooth ground.
- Update journal sketches and icons to the new look.
- Update `docs/design/` for every changed system, and the guide plan in Amendment R §9.2 (digging, piles, slopes, building on ground). Update `README_PREVIEW.md` if it's in the repo.

---

## 12. Performance and quality targets

### 12.1 Baseline (S0, before any change)
Using the existing benchmark tools on the reference machine (8-core CPU, RTX 3060 / RX 6600 class) at 1440p, record for the High and Low presets: flythrough frame time p50, p99 and worst; GPU time per pass; triangles and draws per frame; VRAM by category; meshing throughput (cubes/s); edit-to-visible latency; memory per surface cube; save size per explored area; network delta size per edit. Store the results in `BENCHMARKS.md` as **Baseline-S**, along with the screenshot suite.

### 12.2 Targets (same scenes, same presets, same machine)
- **Frame time:** p50 within **+10%** of Baseline-S and p99 within **+15%** at High; at or better than Baseline-S at Low. Report a forest scene and a mountain-vista scene separately.
- **Meshing:** at least **2,000 surface cubes/s** on 8 cores (CPU path); player edits visible within one frame near the camera.
- **VRAM:** terrain material textures ≤ ~1.5 GB at High, ≤ ~400 MB at Low; total within the existing budget.
- **Memory and saves:** memory per surface cube and save size per explored area no more than 1.5× Baseline-S.
- **Collision:** 300 animals and 150 humans near players stay within the v2 §21 and V2.1 §17 budgets.
- **LOD:** all v1 §8.4 targets still pass.
If a target can't be met after a real optimization pass, don't hide it: record the numbers, what was tried and the options in `DECISIONS.md` under "Needs owner confirmation", and keep going.

### 12.3 Graphics presets
Extend the presets with: terrain mesh detail (full / simplified near-mid), material texture resolution, blend count (3 / 2), parallax (off / on), foliage density and LOD distances, impostor distance, hair/fur quality, body mesh LOD bias. **Low** must run well on integrated graphics at 1080p.

### 12.4 Visual quality review
Re-run the full screenshot suite (v1 §14 cohesion check, v2 milestones, V2.1 era reviews) and compare with Baseline-S side by side in `docs/review/smooth-world.md`. Check specifically: no cracks at cube seams or between LOD levels; no shimmering or tiling patterns; soft materials soft and rock crisp; nothing blobby; beaches, riverbanks and dunes look natural; cliffs show strata; caves look like caves; water edges clean; forests look like forests near, mid and far; bare winter trees look right; people and animals look like they belong in the same world as the terrain; no color jump at the LOD transition.

---

## 13. Tests
- **Data:** fill quantization round-trips; regenerated fill equals the stored fill for unmodified cubes; save migration from the previous format.
- **Meshing:** watertight and crack-free across cube boundaries (automated edge-matching checks on randomized fields); deterministic output; material weights sum to one.
- **Collision:** capsule-vs-field depenetration converges; no tunnelling at sprint and fall speeds; identical results on server and client (determinism test over a scripted walk).
- **Editing:** mass conservation (dig then refill returns the same material and volume; total material in a closed test region is constant through digging, piling and settling); settling reaches stable slopes at each material's angle of repose; network deltas reproduce edits exactly.
- **Movement:** maximum walkable slope and sliding behave per material, wetness, ice and footwear.
- **Foliage:** foliage occupancy matches rendered canopy extent within tolerance; light reduction under canopies within designed ranges.
- **Bodies:** phenotype → mesh is deterministic; relatives' shape parameters correlate as designed.
- **Performance:** the §12.2 targets as benchmark assertions in the soak suite.

---

## 14. Milestones

Each milestone: implement, update design docs and data, add tests, run `scripts/check.sh`, `hearth content lint` and the benchmarks, update `PROGRESS.md`, commit.

**S0 — Baseline and prototype.** `MIGRATION_SMOOTH.md`; Baseline-S benchmarks and screenshots (§12.1); the mesher prototypes on the test scenes (§3.1); a material-blending and biplanar-shading prototype on two materials; the algorithm decision with evidence. *Accept:* `DECISIONS.md` has the choice with screenshots and numbers; Baseline-S recorded.

**S1 — Fill data and editing core.** Fill values in cubes and generation (§2), material properties, saves and migration, network delta format, raycast with sub-voxel hits, mass-conserving dig and place brushes, settling simulation (§8.3–8.4). *Accept:* §13 data and editing tests pass; a dug pit and spoil pile settle realistically in a headless test.

**S2 — Smooth terrain rendering.** The production mesher (§3.2), compact vertices, mid-range simplification, material blending, procedural PBR materials and compression (§4.2), biplanar shading with height blending and anti-tiling, shader overlays (wetness, snow, moss, litter, scorch), sub-metre stratigraphy, voxel AO and trilinear light, material packs. *Accept:* §13 meshing tests; the screenshot suite shows smooth terrain everywhere; frame targets met for terrain-only scenes.

**S3 — Movement, collision and navigation.** Field-based capsule collision on server and client, slope walking, sliding and footing, footsteps and footprints on smooth ground, nav grid rebuilt for animals and humans, built-piece skirts and Level ground (§6, §8). *Accept:* §13 collision and movement tests; animals and humans path well over hills, scree and riverbanks; budgets met.

**S4 — Distant terrain.** Fixed-point LOD heights, smooth LOD meshing, matched shading and overlays, canopy shapes in LOD (§5). *Accept:* no visible seam or color jump at the near/far transition in the screenshot suite; v1 §8.4 targets still pass.

**S5 — Trees and foliage.** Smooth trunks and branches, felling rigid bodies and log meshes, leaf clusters with translucency and wind, foliage occupancy and dappled light, seasons, the foliage LOD chain with impostors, ground cover seated and bendable (§7). *Accept:* each species' silhouette screenshot set at three ages and four seasons; forest benchmark targets met.

**S6 — Bodies and objects.** Smooth procedural skinned bodies for every body plan, genetics-driven shape, expressive faces, coats, hair and fur, fitted clothing, body LODs and impostors; smooth item meshes from form × material (§10). *Accept:* a three-generation family screenshot set shows resemblance; herds and crowds stay within budgets; every item form renders for every material.

**S7 — Water, snow, ice, caves and built-piece polish.** §9 in full and §6 visual polish. *Accept:* shoreline, waterfall, snowdrift, frozen lake, glacier and cavern screenshots reviewed; no gaps where buildings meet the ground.

**S8 — Performance and cohesion pass.** Optimize to the §12.2 targets at every preset; complete `docs/review/smooth-world.md` (§12.4); update docs, journal art and the Amendment R guide plan (§11). *Accept:* all §12 targets met or honestly documented for owner decision; the review shows one coherent world.

Then resume the remaining H milestones.

---

## 15. Resume protocol additions
Keep all earlier protocols. Also keep current: `MIGRATION_SMOOTH.md`, Baseline-S and the latest numbers in `BENCHMARKS.md`, and `docs/review/smooth-world.md`. Add a **Smooth World Status** row to `PROGRESS.md` (milestone, targets met/unmet). On restart, run the meshing seam test, the collision determinism test and the terrain benchmark along with `scripts/check.sh`.

---

Begin with §0.3. The bar for this amendment: someone seeing a screenshot should think "that's a real valley", and someone playing should feel the ground: sand giving way underfoot, a slick clay bank they can't climb, a pile of dirt that slumps when they dig, without the game running any worse than it does today.
