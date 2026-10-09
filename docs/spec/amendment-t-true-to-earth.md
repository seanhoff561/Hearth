# AMENDMENT T — True to Earth
## The owner's latest playtest, a realism gap analysis of the whole game, and the work it leads to

*The owner's amendment of 2026-10-09, as given, after playing `main` at `6b6dfc6` (E4.1 done,
S5/P7 paused at (b)). Its order of work is in `PLAN.md`; the findings are `dev/PLAYTEST.md`
32–42.*

You are the autonomous engine programmer building `hearth`. The owner has played the latest build (`main` at `6b6dfc6`: E4.1 done, S5/P7 paused at (b)) and sent a set of findings. Some are small and clear. Most point to the same deeper goal: **the world should feel like the real Earth at every scale, from continents to the pebbles underfoot, and it should still run well.**

This amendment does three things:
1. fixes the small, clear issues quickly;
2. has you **analyze the whole game against the real world**, rigorously and with evidence, and turn that into a plan;
3. sets the direction for the larger work (the ground at walking scale, water, the near-to-distance transition, people) **while leaving the design choices to you**, judged by evidence.

**The owner's standing permission:** if reaching the goal means rewriting parts of the original cube-based game, that's fine. The vision is a realistic simulation of a generated Earth. Take liberties where they serve that vision. The Quality Charter (Amendment Q) governs all of it: real, lean, fast, whole, organic.

---

## 0. How this fits into the current work

### 0.1 Order of work
1. **T0 — Reach the S5/P7 stopping point:** finish (b), the tree renderer with its species silhouette sheets (three ages, four seasons), to a green commit. Record (c)–(f) as the next steps in `PROGRESS.md`.
2. **T1 — Quick wins** (§2). Small, visible fixes.
3. **T2 — The realism gap analysis** (§3). Evidence first, then a revised plan.
4. **Then the work T2 plans.** The **default sequence** is below. After T2 you may reorder, merge or split it, with the evidence recorded in `DECISIONS.md`.
   1. **S5/P7 (c)–(f) together with G1, the ground at walking scale** (§4.1). P7 (d) and (e), shrubs, boulders, logs, strata and veins, are the same work.
   2. **W1 — Water** (§4.2). It absorbs and replaces the water part of S7.
   3. **L1 — Seamless distance** (§4.3), including trees in the LOD band (S5 (c)) and distant water.
   4. **P7G — Grasses** (as planned, on the new ground and within the new distance chain).
   5. **S6 with H1 — animals, items and human likeness** (§4.5).
   6. **R1 — Rendering realism** (§4.4) wherever T2 finds the biggest gains. It can come earlier if T2 says so.
   7. **S7** (snow, ice, caves, built-piece polish) → **S8** → **P8** → the remaining plan (V2-13 …) unchanged.
5. **Audits** continue under Amendment Q §8.2: one after every three completed milestones. Each audit adds a **realism scorecard** update (§3.6).
6. Push `main` after every milestone, as you've been doing.

### 0.2 Two rules that come from this playtest
- **Show it, don't say it.** If the world can show something (ravens over a carcass, smoke on the horizon, a storm coming), show it, with sound. Don't narrate it in text. Review every text notice the game sends about the world, and replace each with presence in the world where possible. Remove any that repeat.
- **Fix causes, not tests.** When a test fails because the world is wrong, fix the world. The S5 stopping-point note says the finite-water test "found no river bank that holds a channel among the four it looked at, and now looks at some twelve times as many". That's a sign rivers don't fit their terrain. Revisit it in W1.

---

## 1. The owner's findings, with what the code shows
Add each to `dev/PLAYTEST.md`. These pointers are starting points; confirm before fixing.

| # | Finding | Where it comes from (as of `6b6dfc6`) |
|---|---|---|
| 1 | "Ravens are circling to the northeast" appears again and again; the owner wants to see real ravens circling instead | `crates/hearth/src/server.rs` (~line 1587) sends that line every 40 ticks (every 2 s) in daylight while `fauna.ravens(at)` (`crates/hearth/src/fauna.rs` ~806) finds fresh remains in sight. No birds are drawn. `common_raven` exists (`data/hearth/fauna/temperate_birds.ron`, `BirdPerching`) and `hearth_fauna::anim` has flight poses. |
| 2 | Skin is too shiny | `crates/hearth_render/src/shaders/body.wgsl`: two GGX lobes (roughness 0.48 and 0.25, weighted 0.85 : 0.15, F0 0.028). Wrap lighting stands in for subsurface transport. No fine-scale normal detail (pores, fine lines) or cavity/specular occlusion breaks the highlight. The sky's specular is only a grazing sheen. Wetness multiplies the specular by (1 + wet). |
| 3 | The human model doesn't look much like a human | E7's procedurally sculpted body (`docs/design/people.md`, D266–D270) |
| 4 | Selection highlights whole cubes; the owner wants the actual thing highlighted (the berries themselves) | `crates/hearth/src/client.rs` `highlight_boxes()` draws box edges: a block's outline-shape bounds, an item's resting box, an animal's box from its mass. `Aim` has only `Block`, `Item` and `Animal`, with no sub-object targets (a berry cluster, a branch, a patch of soil). |
| 5 | The globe map should be more detailed, with elevation easier to see | E4.1 builds the map from the planet grid (one texel ≈ 20 km, D280) |
| 6 | Water edges are still cubes; the owner wants more realistic water physics, and sees rivers, streams and pools behave oddly relative to the terrain | Water is block-based (`water`, `level:0..8`), meshed as quads by the old block mesher's `fluid()` (`crates/hearth_render/src/mesh.rs`), not by `hearth_smooth`; hence cube edges against the smooth ground. See also §0.2 on the river-bank test. |
| 7 | Finer detail was lost in the move to Earth size; the owner wants it back and better, now that the grid is gone | `crates/hearth_worldgen/src/relief.rs`: physically based refinement levels at 2.4 km, 306 m and ~38 m (Priority-Flood plus implicit stream-power erosion). Below the last, "the blocks take their shape from its surface and a little noise" (plus `region/mod.rs` `DetailNoise`: fBm around 192 m, 96 m and 24 m). **On the Standard planet the grid itself was ~32 m per cell** (65,536 m / 2048), so the planet-wide erosion and drainage shaped the land right down to walking scale. At Earth size nothing physically based acts below ~38 m. That's the detail that was lost. |
| 8 | Small-scale realism everywhere: pebbles, boulders, rocks, soil and terrain right for every biome and cohesive with plants and animals, at every level from tectonics and weather down to hills, bumps and sticks | Whole generator; see §3 and §4.1 |
| 9 | The transition from full detail to LOD is sharp and ugly; the LOD should look as much like the near terrain as possible, especially close by, or the existing system should be optimized further | `crates/hearth_render/src/lod.rs` and `shaders/lod.wgsl`: 33 × 33-vertex height-field tiles; ground shaded with each material's **mean albedo** (`ground_mean`) and a simpler light path (`lod_light`) than the near ground's (`fs_smooth`); **trees as crown and trunk boxes** (S5 (c) was to replace them); a dithered band at the edge of full detail (`near_weight`) |

---

## 2. T1 — Quick wins

### 2.1 Real ravens, not a message
- Remove the repeating text.
- Materialize **ravens** (or the right scavenger birds for the realm and biome: ravens and crows in temperate and boreal lands, vultures in savannas and deserts, kites, eagles) as real animals over fresh remains in daylight:
  - circling on thermals or soaring tens of metres up, calling (positional audio audible at a real distance)
  - landing to feed in turns
  - scattering when the player or a predator approaches
  - leaving when the remains are gone
- They're visible from as far away as real birds would be. Beyond the near field, draw them as cheap impostors or dots so a circle of birds can be seen on the horizon.
- Use the existing fauna systems (`common_raven`, flight animation, the ecology's remains).
- Then **review every other world-describing text notice** under §0.2's rule.

### 2.2 Skin that looks like skin
- The overall target for the character's look is set in §4.5 (the owner's reference). Skin is the first and most visible part of it.
- **Calibrate against measured human skin reflectance** (look up measured face and body skin BRDF studies, e.g., Weyrich et al. 2006, and published production skin-shading references), and against real photographs under the same light (sun, overcast, firelight).
- Expected direction:
  - rougher primary specular, a weaker and broader second lobe
  - **regional variation:** an oilier T-zone, drier and rougher limbs and hands
  - **micro-normal detail** (pores, fine lines by age and region)
  - **cavity and specular occlusion**
  - energy conservation between diffuse and specular
  - **real subsurface scattering:** screen-space or pre-integrated, replacing wrap lighting
  - **image-based ambient specular** from the sky, not just a grazing sheen
- **Wetness:** wet skin gets a thin film layer, not a multiplied specular.
- **Review:** side-by-side close-ups under the four lights, in `docs/review/t1/`.

### 2.3 Highlight the thing itself
- **Targets are real objects and parts of objects**, not cells. Add aim targets for:
  - a plant part: a berry cluster, a nut, a flower, a branch segment
  - a loose object: a stick, a stone, a cone, an acorn
  - a patch of ground: the area a dig or gather would take, from the sculpt preview (Amendment S §8.3)
  - a water surface point
  - an animal's body part where it matters (for butchering)
  - an item; a building piece
  Picking is precise: ray tests against the real shapes (tree-skeleton segments, cluster spheres, item hulls, body-part capsules), or a tiny GPU ID buffer around the crosshair. Keep it cheap (< 0.2 ms, Amendment P §5.1).
- **The highlight follows the object's own surface:** a soft outline or rim glow drawn from the target's own geometry (a screen-space silhouette of its mesh, e.g., a stencil mask plus an edge pass), or, for ground and water, a soft decal following the surface. **Never a box.** Remove the box-edge highlight code.

### 2.4 A globe with real relief
- **Detail:** raise the base map's resolution (e.g., 4096 × 2048) and, when zoomed, stream detail tiles from the 2.4 km refinement level (a coarse caller, within E4.1's rules).
- **Draw elevation clearly:**
  - multi-directional hillshading
  - hypsometric tints on land
  - bathymetry in the oceans (shelves, slopes, trenches visible)
  - rivers and lakes from the drainage network
  - ice and snow
  - a relief exaggeration control (default modest)
  - normal mapping, and optional slight displacement, in the globe shader so ranges catch the light as the globe turns
- **Toggles:** biome/terrain colors, climate, and plain relief.
- **Keep E4.1's targets:** hover instant, rotation smooth, map ready with the planet.

*Accept T1:* no repeating notices; ravens visible over remains and at a distance; skin close-ups reviewed; every aim target highlights its own shape (berries, a branch, a stone, a dig patch); the globe shows relief, bathymetry and rivers clearly; budgets met.

---

## 3. T2 — The realism gap analysis
**Purpose:** find, with evidence, everywhere the game differs from the real Earth in ways a player can perceive, at every scale. Rank the gaps by **perceived realism gained per unit of cost** and turn them into a plan.

Time-box it to about one milestone of effort, and produce evidence (images, metrics, short notes), not essays.

### 3.1 What to compare, at every scale
- **Planet and continents:** plate layouts, continent shapes and sizes, mountain belts, shelves, ocean basins, climate zones, biome distribution. (Many tests exist. Extend them where they're weak.)
- **Regions (10–1,000 km):** drainage networks and basins, river lengths and sinuosity, lake districts, deltas, coastline character, the spacing and form of ranges, plateaus, escarpments, deserts' ergs and regs, karst, volcanic fields, glaciated landscapes.
- **Landscapes (1–10 km), the view from a hill:** valley spacing and cross-sections, hillslope profiles, terraces, floodplains, ridge lines, vegetation patterns across slopes and aspects, how forests meet grassland, how land meets water.
- **Hillslopes and valleys (10 m–1 km):** gullies and rills, channel forms (riffles, pools, bars, cut banks, point bars), banks, colluvial fans, scree and talus cones, outcrops along bedding and joints, tors, landslide scars, terracettes from grazing, dunes and interdunes, moraines, drumlins, kettle ponds, sinkholes, solifluction lobes.
- **The site (1–10 m), what the player walks over:** microrelief (bumps, hollows, tussocks, hummocks, frost heave, **pit-and-mound from fallen trees**, root plates, burrows, wallows, termite mounds, animal trails), rocks and boulders (size distributions, shapes by rock type, burial, lichen and moss by aspect and moisture), soil surfaces (colors, textures, crusts, cracks, puddles, mud, desert pavement, biological soil crusts), litter and deadwood.
- **The ground (cm–1 m):** pebbles and gravel (rounding and sorting), sand ripples, leaf litter layers, twigs, cones and nuts, roots at the surface, moss and lichen textures, footprints.
- **Water at every scale:** see §4.2.
- **Life in the land:**
  - Vegetation responds to microtopography, soil and water: rushes in wet hollows, drier species on ridges, trees on ridges and banks.
  - Animal signs, trails and wallows follow the terrain.
  - Fauna density and behavior look plausible in view.
- **Weather and sky:**
  - cloud types by situation (cumulus fields, stratus decks, cirrus, cumulonimbus with anvils)
  - fronts and rain bands
  - fog types (valley radiation fog, coastal advection fog)
  - wind over terrain
  - snow drift
  - realistic durations and diurnal cycles (Amendment E §4.2)
- **Light and rendering:**
  - sun and sky light levels and colors (Amendment Q §2.1)
  - shadows; ambient occlusion
  - **indirect light** (a common gap in real-time scenes)
  - aerial perspective, materials against the reference ranges, vegetation shading and translucency, water optics, skin
  - night: moonlight, firelight and darkness
- **Distance:** the near-to-LOD transition, distant forests, distant water, horizon haze.
- **Motion and animals:** gaits and speeds (already audited), how flocks, herds and grass move.
- **Sound:** soundscapes by biome and time (briefly; not the focus).

### 3.2 References: what to compare against
- **Photographs of real places**, matched to the game's biomes, scales, times of day and weather.
  - Prefer openly licensed sources (Wikimedia Commons per-file licenses, CC0 libraries).
  - Others are for internal comparison only: keep them in a git-ignored folder, record their URLs and licenses, and never ship them (Amendment Q §4).
- **Real elevation data** for terrain statistics:
  - SRTM (~30 m, public domain)
  - USGS 3DEP lidar DEMs (~1 m, public domain)
  - other national open lidar sets (licenses vary; check each)
  Use them to measure real-world statistics by landscape type: slope and curvature distributions, roughness power spectra across scales, drainage density, valley spacing, channel width against drainage area.
- **Field and scientific data:**
  - geomorphology (hillslope profiles, channel hydraulic geometry, sediment sorting)
  - boulder and clast size distributions
  - soil profiles and surface properties by soil order
  - forest inventory data (stem density, size distributions, deadwood volumes)
  - litter depths
  - cloud and precipitation climatologies
  Cite the sources in data and docs.
- **If network access is blocked,** write fetch scripts and instructions for the owner, and work from published statistics in the meantime.

### 3.3 Methods
- **Matched views:** render the game at matched biome, season, time, weather, camera height and field of view for each reference photo. Use a fixed set (e.g., 20 biomes × 4 scales × 2–3 lighting conditions) rendered by the screenshot tool, kept as **the realism suite** and re-rendered at every audit.
  - If you can view images, compare them side by side and write down specific differences ("the gravel bar's stones are all one size; real bars sort by size downstream").
  - If you can't, rely on the quantitative measures below and ask the owner to review a contact sheet.
- **Quantitative comparison:**
  - Terrain: slope histograms, curvature, roughness spectra by scale, drainage density, valley spacing, hypsometric integrals, river sinuosity, channel width–discharge relations; generated against real DEM statistics for the same kind of landscape.
  - Objects and plants: size distributions, spacing (pair-correlation), burial depth, cover fractions, density by slope, aspect and moisture.
  - Images: luminance and color distributions by biome, spatial-frequency spectra, texture-repetition scores (Amendment Q §3), edge statistics at the near–LOD boundary.
  - Weather: cloud-cover, rain-intensity and duration distributions against climatology.
- **The fine-detail regression:** render the same seed's land on the Standard test planet (Developer mode) and on the Earth planet, at walking scale, in the same biomes. Identify what was lost and why (see §1 row 7), and how much each refinement level or noise contributes.
- **How it's built vs how Earth builds it:** for each scale, write one or two lines on the processes that make the real landscape and the processes the generator uses, so gaps in **process**, not just appearance, are visible.

### 3.4 Output
- **`docs/review/realism/RGA-1.md`:** findings by scale and system, each with evidence (paired images, metrics) and a severity (how much a player would notice).
- **A ranked gap table:** perceived realism gained per estimated cost (CPU, GPU, memory, development effort), with a proposed approach and a measurable target for each.
- **The revised plan:** which milestones change, merge or are added, written into `PLAN.md` with a `DECISIONS.md` entry. The default sequence in §0.1 is the starting point.
- **The realism suite** (§3.3) committed as screenshot specs, its reference set recorded (links and licenses), and its metrics script.

### 3.5 Bound it
T2 is analysis plus small prototypes where needed to judge an approach. It's not implementation. Anything found that is trivial and clearly right (a wrong color, a missing seasonal state) can be fixed in passing and noted.

### 3.6 The realism scorecard (kept up to date)
`docs/review/realism/scorecard.md` is a short table per scale and system: current metric vs target, and the last suite render's verdict. Every audit updates it. It's how the owner sees the world getting more real.

---

## 4. Direction for the larger work (goals and options; choose by evidence after T2)

### 4.1 G1 — The ground at walking scale (with S5/P7 (c)–(f))
**Goal:** walking anywhere feels like walking on real ground of that place, from hills down to the stones and sticks underfoot, consistent with the land's larger structure and with the plants and animals living on it.
- **Physically based processes below ~38 m,** at least near players: extend the refinement levels with a ~5 m level and a ~1 m level, or another scheme you justify, using processes that act at those scales:
  - hillslope diffusion and creep (convex hilltops, concave footslopes)
  - rills and gullies on steep, bare or erodible ground
  - channel and bank forms where small streams run
  - colluvium and fans at slope feet
  - scree and talus at their real angles
  - **tree-throw pit-and-mound** in forests
  - frost features in cold climates
  - **aeolian forms** in sandy deserts and on coasts (dune types by wind regime and sand supply; ripples)
  - **karst** on carbonate rock
  - glacial landforms where ice once was
  Keep them deterministic, tile-local and fast (E4.1's rules).
- **Options to evaluate for walking-scale detail:**
  - *(A) Process-based procedural landforms*, as above.
  - *(B) Example-based synthesis* from real elevation exemplars: public-domain lidar patches by landscape type, guided by the generator's macro structure and drainage.
  - *(C) A small learned generator* trained offline on public-domain DEMs, conditioned on landscape type and the coarse surface. It must run deterministically and fast enough per tile.
  - *(D) A hybrid.*
  Pick by the §3.3 terrain statistics and by speed, and record the evidence. Licensing per Amendment Q §4.
- **Rocks, pebbles, boulders and other surface objects** (the loose-objects layer, Amendment E §7.3, and P7 (e)):
  - **Placement:** clast size distributions (heavy-tailed, as measured in nature); shapes by lithology (rounded granite corestones, platy slate, blocky sandstone, columnar or vesicular basalt, flint nodules); partial burial; sorting and rounding along rivers; scree grading downslope; glacial erratics; desert pavement.
  - **Surface life and weathering:** lichen and moss by aspect, moisture and age; weathering rinds.
- **Soils at the surface:**
  - colors and textures by soil type and moisture
  - surface states: crusts, cracks when drying, puddles after rain, mud trampled near water, biological soil crusts in drylands
  - litter layers by forest type and season; exposed roots on banks and paths
- **Cohesion with life:**
  - Plants (P7) respond to the microtopography and the soil moisture the new levels produce.
  - Animals' trails and wallows follow the land and wear it.
  - Tree-throw mounds come from the forest's own deaths.
- **Targets:**
  - walking-scale terrain statistics within the real ranges for each landscape type (§3.3)
  - no visible noise patterns
  - the realism suite's site- and ground-scale shots judged closer to the references
  - generation within E4.1's budgets (state the per-tile time and the loading-time impact)

### 4.2 W1 — Water
**Goal:** water that looks, flows and behaves like real water at every scale, fits the terrain exactly, and never shows a cube.
- **Water in the generated world as continuous surfaces, not blocks:**
  - Generate a **water surface field** at sub-metre precision from the hydrology:
    - lakes level to their outlets
    - rivers whose surfaces fall smoothly downstream, following real hydraulic geometry (width, depth and velocity scaling with discharge, as in the Leopold–Maddock relations), with riffles and pools, meanders cutting outer banks and building point bars, gravel bars, braided reaches where gradient and sediment call for them
    - springs and seeps
    - waterfalls where channels cross steep steps
    - estuaries and tidal flats
  - **Beds and banks** are carved into the smooth ground consistently with the surface, so rivers always sit in channels that hold them.
- **Dynamic water near the player:** a **shallow-water simulation** on a height field (or a solver you justify) coupled to the fill field, for water the player moves, rain runoff, puddles filling and drying, floods spilling onto floodplains, dams, irrigation channels. Water volume conserved; infiltration into soil by soil type. Vertical flows (waterfalls, pouring) handled specially. Budgeted, and only where something changes.
- **Rendering:**
  - Smooth water surfaces from the surface field, meeting the smooth ground cleanly (depth-based shore blending, foam, wet margins).
  - **Flow maps** from the simulation and hydrology, so rivers visibly flow at their real speed and direction.
  - Waterfalls with sheet, spray and mist; ripples from rain and from bodies.
  - Underwater as already designed.
  - The same water at distance (L1).
- **Interaction:** wading drag by depth and current, swimming against currents, buoyancy and drifting objects, fording risk, drinking and filling at any shore.
- **Replace** the block water (`water`, `level:0..8`) and the old mesher's `fluid()` path for natural water. Migrate saves. Revisit the river-bank test (§0.2).
- **Targets:**
  - no cube edges anywhere
  - every generated river and stream sits in a channel that holds it
  - flow speeds plausible for each river's discharge and slope
  - the dynamic simulation within a stated CPU budget
  - the realism suite's water shots (mountain stream, lowland meander, braided river, lake shore, waterfall, beach, tidal flat) judged closer to the references

### 4.3 L1 — Seamless distance
**Goal:** no visible line where full detail ends. Near the boundary the LOD looks like the near world, and farther out it degrades gracefully the way real distance and haze do.
- **Measure first.** Capture flythroughs and measure the change in image statistics across the boundary (luminance, color, normal and detail energy) and the "pops" (frame-to-frame changes over a threshold). Make it a perf-gate metric.
- **Options to combine** (choose by measurement and cost):
  - *Push full detail farther* by making the near field cheaper (mesh simplification at mid range, GPU-driven work, cheaper cube processing), so the boundary sits where it shows less.
  - *Make the first LOD rings much closer to the near field:*
    - finer height fields near the boundary (e.g., 1–2 m vertices)
    - normals baked from the finer surface
    - **the same material shading** as the near ground (procedural materials filtered for distance, not mean albedo), the same lighting path, AO, shadows and wetness and snow overlays
  - *Morph geometry* across a band (geomorphing) instead of only dithering, and widen or adapt the band.
  - *Trees and plants continuous across the boundary* (S5 (c)): real instances or simplified meshes, then impostors, never boxes; grass handed over to the ground's sward shading (P7G).
  - *Unify near and far* into one continuous LOD for the smooth surface (e.g., clipmap- or octree-based meshes from the same fill and height fields), if measurement shows the two-system design can't reach the goal.
  - *Distant water* identical in look to near water.
- **Targets:**
  - no seam visible in the realism suite's landscape shots
  - pops below threshold in the flythrough
  - frame-time targets met (Amendment S §12)

### 4.4 R1 — Rendering realism (where T2 finds the biggest gains)
Likely candidates, to be confirmed by T2:
- **Indirect lighting:** with only hemispherical ambient light, scenes look flat. Evaluate approaches suited to a voxel world and wgpu: irradiance probes fed by the voxel light grid with bounce color, screen-space GI, or low-resolution voxel cone tracing.
- Better ambient occlusion and contact shadows.
- Vegetation translucency and dappled light.
- Calibrated materials (Amendment Q §2.1) and the night (moonlight, firelight, darkness).
Choose by realism gained per millisecond.

### 4.5 H1 — Human likeness (with S6)
**Goal:** the player's body looks like a real person.
- **The owner's reference:** the player model of the survival game **Rust** (by Facepunch Studios; *the game, not the programming language*) is a good example of what the owner wants:
  - realistic adult proportions and anatomy
  - believable faces
  - skin with a natural, mostly matte look and visible detail
  - hair and beards that read as real at play distance
  - bodies that move with weight
  - a style that sits naturally in a realistic outdoor world
  **More realistic than that is better, as long as the body still fits the rest of the world** (Amendment Q §1, "Whole").
- **Treat it as a quality bar, not a source.**
  - Study publicly available screenshots and videos of it, alongside real photographs, for proportions, faces, skin, hair and motion.
  - Copy nothing: no assets, meshes, textures, rigs or exact designs (the clean-room rule).
  - Keep any reference images out of the repository (git-ignored, §3.2), and mention the game's name only in internal design notes, never in the game, the guide or marketing (Amendment R).
- **Start from real human form data:**
  - **Body proportions:** anthropometric data such as ANSUR II (a US Army survey released publicly; verify its terms).
  - **Base meshes and morph targets:** evaluate openly licensed ones (for example MakeHuman's assets, verifying their license, reportedly CC0).
  - **Never** use models with non-commercial or research-only licenses (e.g., SMPL / SMPL-X, FLAME, the Basel Face Model).
- **Deformation quality:** joint placement, dual-quaternion skinning or corrective shapes at shoulders, hips, elbows, knees and wrists, and muscle and fat that move. Hands with real proportions and fingers; feet; a real face (bone structure, eyelids, lips, ears); a hairline and hair growth patterns.
- **Review** against real reference photos of people (licensed for reference, §3.2) and against the reference bar above, in neutral poses and in motion, with the same lights as §2.2. Use matched views: the same framing, pose and light. Write down specific differences ("shoulders too narrow for the build", "the jaw reads soft", "hair looks like a helmet at 5 m") and fix them. The owner judges the result from a contact sheet (§7).
- The character creator keeps working and updates every frame (E4.1).

### 4.6 Animals and items (S6)
The same method as H1 for animals: real proportions from zoological measurements, deformation, fur and feathers, gait from biomechanics (Amendment Q §2.5). Items: real shapes from archaeological references (knapped stone, hafts, pottery profiles).

---

## 5. When to rewrite the cube-era code
Rewriting is allowed, and expected where it serves the vision. Decide with these tests:
- **Rewrite when** the old structure blocks a realism goal, or adapting it would cost more and stay worse than replacing it.
- **Do it one system at a time,** with a migration path for saves, tests that prove parity or improvement, and `main` green at every commit.
- **Remove the old path in the same milestone** (Amendment Q §5.1).
- **Likely candidates** (confirm in T2):
  - block water and its level-based flow (W1)
  - plants as cross-sprite blocks, and trees as voxels in the mesher (S5/P7)
  - per-block feature placement in `cubegen` (off-grid placement, P7)
  - the `Aim::Block` cell model of interaction (T1 §2.3)
  - the old block mesher for natural things
  - near-distance LOD built from the column-segment model, if it stands in the way of L1
- Record each rewrite in `DECISIONS.md` with its reason and its before/after evidence.

---

## 6. Performance stays a feature
All of this is under Amendment Q §7 and E4.1's rules: every query at its own scale, priorities, memory caps, the main thread never starved. Each new system states its budget in `docs/design/budgets.md`. The perf gate gains:
- the boundary-seam metric (L1)
- the water-simulation budget (W1)
- per-tile generation time for the new ground levels (G1)
- loading-time impact
**Realism improvements that break the budgets aren't done until they fit.**

---

## 7. The owner's part
- When images need a human eye (skin, the human model, the realism suite's contact sheets), put them in `docs/review/` and ask the owner, briefly, in `PROGRESS.md`'s "Owner checks" section.
- If reference data or photos can't be fetched from the build environment, write a fetch script and ask the owner to run it.

---

## 8. Acceptance
- **T0:** S5/P7 (b) complete; silhouette sheets rendered; `main` green and pushed.
- **T1:** §2's acceptance.
- **T2:**
  - `RGA-1.md` with evidence at every scale in §3.1
  - the ranked gap table
  - the realism suite and its metrics script
  - the scorecard
  - the revised `PLAN.md` and its `DECISIONS.md` entry
- **G1, W1, L1, H1, R1:** targets as set in §4 and refined by T2's table.
- Every milestone: the Quality Charter checklist (Amendment Q §8.1) and a scorecard update at each audit.

---

## 9. Resume protocol additions
Keep all earlier protocols. Also keep current: `docs/review/realism/` (RGA reports, the scorecard, the suite's specs and references list), and a **Realism** row in `PROGRESS.md` (last suite render, top three open gaps). On restart, check the scorecard's open gaps before choosing the next task.

---

Begin with T0. The bar for this amendment: the owner walks down from a ridge to a stream at dawn and nothing gives the game away. The slope steepens the way slopes do, stones gather where water put them, the stream fills its channel and runs at its own speed, ravens turn slowly over something dead in the next valley, and the distant hills, in their haze, look like the hill underfoot, only far away.
