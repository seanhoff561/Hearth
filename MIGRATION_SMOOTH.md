# Migration to the smooth voxel world (Amendment S)

Amendment S (`docs/spec/amendment-s-smooth-world.md`, 2026-10-07) makes natural terrain smooth on
the same 1 m voxel grid: each voxel gains a signed 8-bit **fill** beside its material, natural
ground is meshed through the fill's zero crossing, characters collide with the field, digging
moves real volume; trees, foliage, bodies and items get smooth forms to match. Every simulation
system stays on the grid. This file maps each subsystem the change touches to **Keep**,
**Modify** or **Replace**, with where it is today and the S milestone that changes it. It is kept
current through S0–S8 (S §15).

What the world is today (the S0 survey, 2026-10-07):

- Terrain is a **2D heightmap**: `Terrain::sample_with` gives a column's continuous height, and a
  block is ground below its rounded height. Cliffs, worm caves and caverns are continuous tests
  (a cliff column's `(top − y − 0.5) + noise·cliffiness·8`, a worm's ellipsoid, a cavern's
  perturbed distance) quantized to blocks. There is no 3D density field and no fill.
- Partial fill exists only as discrete block states: snow layers (eighths), water levels
  (eighths, with litres in `WaterSim`), slabs and branch shapes.
- Near terrain is **meshed on the server** (`Stream::work`, `LocalWorld::mesh`, rayon) into
  greedy 16-byte `PackedQuad`s and 64-byte `GeneralQuad`s with corner AO and per-corner light,
  sent as `ToClient::Mesh`; the client uploads ≤ 256 meshes a frame and draws them through
  GPU-driven culling (cave visibility walk, frustum, two-phase Hi-Z, indirect-count draws).
- Textures are original 16×16 procedural pixel art (`hearth_texgen`) in one texture array; there
  is no resource-pack loader in use and no texture hot reload.
- Collision is an AABB swept axis by axis against block boxes (`hearth_physics`), steps of 0.6 m
  free and 1.05 m scrambled; the client predicts and the server adopts its moves.
- Distant terrain (`hearth_lod`) is a quadtree of 32² tiles of integer column heights, meshed as
  flat-topped columns with skirts, crossfaded by an 8-block dither band; crowns as boxes.
- Saves regenerate terrain from the seed and keep player edits as an overlay of block states
  (`edits.rs`, D73); save format 4.
- Animals and people are boxes on skeletons (`hearth_fauna::rig`, `hearth_character::rig`), drawn
  as instanced unit cubes (`figure.wgsl`); a thing lying in the world is one coloured box.

| Subsystem | Where | Fate | Note | When |
|---|---|---|---|---|
| Cube storage (palette, 16³, light nibbles) | `hearth_world::{cube, palette, storage, fill}` | **Modify** (done, S1) | Gains an optional fill array (signed 8-bit, ±1.5 voxels) for surface cubes only; uniform cubes carry none, as single-value palettes now. Compressed in memory and in saves. | S1 |
| Block registry and definitions | `hearth_world::block`, `data/hearth/blocks/` | **Modify** (done, S1) | Each state gets a voxel kind from data: natural (meshed smooth), structure (a built piece), fluid, foliage occupancy, empty. Block states stay the game's vocabulary. | S1 |
| Material data | `hearth_content::schema::material`, `data/hearth/materials/` | **Modify** (done, S1: ground families in `reference.ron`; the surface recipe comes with S2's textures) | Adds sharpness, angle of repose (dry, wet), friction (dry, wet, icy), slump, walk sound and a surface recipe; friction and sound move here from the block templates. | S1 |
| World generation | `hearth_worldgen::{cubegen, region, caves}` | **Modify** (done, S1) | Emits fill from its continuous values (height minus y, the cliff test, cave distances) instead of discarding them; unmodified cubes regenerate their fill from the seed. | S1 |
| Snow layers and ice | `crates/hearth/src/season_cover.rs` | **Modify** | Snow becomes fill on top of the ground (hollows and lee sides first), ice a smooth sheet; layers states retire. | S7 |
| Water simulation | `hearth_world::water` | **Keep** | Level-based on the grid. | — |
| Water surfaces | `hearth_render::{mesh::fluid, water}`, `water.wgsl` | **Modify** | Corner heights interpolated, smooth slopes and falls, shorelines meeting smooth ground. | S7 |
| Near-field terrain mesher | `hearth_render::mesh` (greedy quads) | **Replace** for natural ground | Surface Nets with sharp features (D222; `hearth_smooth`, chosen at S0 over Surface Nets and Dual Contouring): a 2-voxel apron, gradient normals, the feature solve by sharpness × trust, up to 4 blended materials a vertex; S2 adds compact vertices (≤ ~24 B), `meshopt`, light and voxel AO. | S0 (done) → S2 |
| Block models and built pieces | `hearth_render::{models, joints}` | **Keep** (then polish) | Grid-snapped, crisp; the ground mesher treats structure voxels as empty and closes around them; pieces gain buried skirts, bevels and weathering. | S3, S7 |
| Meshing threads, priority, upload budget | `crates/hearth/src/{server, scene}.rs`, client `pump` | **Keep** | Same nearest-first rayon batches and per-frame budget; the payload becomes smooth vertices. Edits near the camera still re-mesh at once. | S2 |
| GPU-driven culling and draws | `hearth_render::{terrain, cull}`, `cull.wgsl`, `hzb.wgsl` | **Keep** | Frustum, Hi-Z and visibility graph apply to smooth meshes; the draw path moves from expanded quads to indexed vertex buffers. | S2 |
| Terrain shading | `terrain.wgsl`, `common.wgsl` | **Replace** for natural ground | Biplanar PBR with height-based blending of up to 3 materials a pixel, anti-tiling, distance fade to material averages, overlays (wetness, snow, moss, litter, scorch, frost), sub-metre strata from the geology. | S2 |
| Corner AO | `hearth_render::mesh::ao_value` | **Replace** | Voxel AO sampled from the fill field at mesh time; optional GTAO on High. | S2 |
| Light engine (sky and block light) | `hearth_world::{light, lighting}` | **Keep** | Vertices sample it trilinearly. | S2 |
| Terrain textures | `hearth_texgen` (16×16) | **Replace** for natural ground | Procedural PBR sets (albedo, normal, roughness, height, AO) from each material's surface recipe, ~1024² with mips, BC-compressed at build time. Pixel art stays for UI, icons and built pieces. | S2 |
| Texture array | `hearth_render::atlas` | **Modify** | A material array of PBR sets beside the existing array. | S2 |
| Resource packs | `Options.resource_packs` (unused) | **New** | A material pack format (data and textures) for terrain, with hot reload; the vanilla-layout loader serves UI, icons and built pieces. | S2 |
| Distant terrain | `hearth_lod`, `hearth_render::lod`, `lod.wgsl` | **Modify** | Fixed-point heights (1/16 block) from the fill; smooth heightfield tiles stitched with the skirts; the same material averages and overlays as the near field; per-species canopy shapes. | S4 |
| Collision | `hearth_physics::{mover, world}` | **Replace** for terrain | A capsule against the trilinear fill field with iterative depenetration, deterministic on server and client; boxes remain for built pieces and items, capsules for trees. | S3 |
| Walking, slopes, sliding | `hearth_physics::mover`, `hearth_player` | **Modify** | Smooth slopes, speed and effort by slope, a walkable limit by friction, wetness, ice and footwear, sliding beyond it, slipping footing on loose ground; the step-up kept for ledges and built steps. | S3 |
| Animal and people footing, navigation | `hearth_fauna::{nav, live::Ground}`, `crates/hearth/src/fauna.rs` | **Modify** | Footing from the surface; the nav grid rebuilt by slope, material and wetness with ledge links. | S3 |
| Raycasts and aiming | `hearth_world::ground::raycast`, `hearth_world::query`, client `pick_block` | **Modify** (root-find done, S1) | Sub-voxel root-find returning the exact hit, normal and material (S1, used by digging); the client's aim and a soft preview highlight of what an action will change come with S2's smooth surface. | S1, S2 |
| Digging, spoil, slumping | `hearth_world::ground`, `crates/hearth/src/workshop/ground.rs` | **Replace** (done, S1) | Tool-shaped volume brushes conserving mass; piles of loose material; a local settling simulation to each material's angle of repose. | S1 |
| Structural analysis and collapse | `hearth_world::structure`, `crates/hearth/src/structure.rs` | **Keep** | Natural anchoring reads voxels whose fill is above the surface. | S1 |
| Building on the ground | `crates/hearth/src/building.rs` | **Modify** | Buried skirts, a Level ground action that moves real soil. | S3 |
| Saves | `crates/hearth/src/edits.rs`, `hearth_save` (format 8) | **Modify** (done, S1) | Edits carry fill; format 5 with a migration (old edits full or empty, then a smoothing pass against the regenerated field). | S1 |
| Protocol | `hearth_protocol` (`Cube`, `Mesh`, `EditTops`) | **Modify** (cube payloads carry fill: done, S1) | Cube payloads carry fill; meshes are smooth; Amendment R's deltas include fill. | S1, S2 |
| Trees | `hearth_flora::{skeleton, template}`, `hearth_worldgen::trees` | **Modify** | Smooth trunk and branch meshes from the same skeletons; occupancy kept on the grid; leaf clusters with wind, seasons and translucency; impostors far away. | S5 |
| Felling | `TreeFalls`, `workshop::fell` | **Modify** | The tree falls as a rigid body from its skeleton; logs with matching meshes. | S5 |
| Ground cover | cross-sprite plant blocks | **Replace** (rendering) | Instanced meshes and cards seated on the smooth surface, bent by passing bodies. The plant blocks stay as the ecology's record. | S5 |
| Animal bodies | `hearth_fauna::{rig, skin, anim}`, `figure.wgsl` | **Replace** (rendering) | Smooth skinned meshes per phenotype bucket on the same skeletons and gaits; coats UV-mapped; fur shells on Mid and High. | S6 |
| People and the player | `hearth_character::{rig, animate}` | **Replace** (rendering) | Smooth skinned bodies from genetics-driven shape parameters, expressive faces, fitted clothing. | S6 |
| Things in the world | client `thing_boxes`, `carcass_boxes` | **Replace** | Smooth meshes from form × material. | S6 |
| Graphics presets | `hearth_core::options` (Fast, Fancy, Fabulous) | **Modify** | Terrain detail, material texture size, blend count, parallax, foliage density and distances, impostor distance, fur quality, body LOD bias. | S2–S8 |
| Benchmarks and gate | `crates/hearth/src/bench.rs`, `scripts/perf-gate.sh`, `BENCHMARKS.md` | **Modify** | Baseline-S recorded at S0; forest and mountain-vista scenes reported apart; meshing and collision targets as assertions in the soak suite. | S0, S8 |
| Screenshots | `crates/hearth/src/screenshot.rs`, `tools/shots/` | **Keep** | Extended with the debug views (wireframe, normals, weights, fill slices, cube bounds, simplification level, the developer-only blocky view). | S2 |
| Ecology, climate, geology, physiology, crafting, knowledge, humans, births | — | **Keep** | They run on materials, volumes and positions, not on how cubes are drawn. | — |

## Notes for the baseline (S0)

- The amendment's reference machine for frame times is an 8-core CPU with an RTX 3060 / RX 6600
  class GPU at 1440p; this project's GPU numbers so far come from the owner's PC (an RTX 4060
  Laptop GPU, 1920×1080, preset Fancy). The cloud machine that builds the game has no GPU — it
  renders on a software device (llvmpipe) whose frame times say nothing (D190's note). Baseline-S
  therefore has two parts: what the cloud can measure honestly (meshing throughput on its CPU,
  triangles, draws, visible cubes, memory per cube, save size per explored area, edit-to-mesh
  latency) recorded at S0, and the GPU frame times, GPU passes and VRAM captured on the PC at the
  same commit with the same scenes (`scripts/baseline-s.sh`).
- Recorded at S0 (`BENCHMARKS.md`, Baseline-S): the CPU half on the cloud machine (today's near
  terrain with `hearth bench --terrain-only`; the prototypes with `bench smooth`) and today's
  look (`tools/shots/s0_baseline.shots`). The GPU half waits for the owner's PC.
