# Rendering

*Status: implemented (v1 M3 terrain; V2-1 sky, lighting, exposure, seasons and weather; the
core of v1 M8 distant terrain, pulled forward from V2-6). Code: `crates/hearth_render`,
`crates/hearth_lod`. Details: `ARCHITECTURE.md` §7, `DECISIONS.md` D18–D19, D29, D32–D34, D37,
D52–D54.*

## Purpose
Fast, correct voxel rendering that scales to large view distances, lit physically so that
days, nights, seasons and weather look like Earth's.

## Model

### Terrain (v1 M3)
Procedural textures in a texture array; greedy meshing of uniform faces into 16-byte packed
quads, 64-byte general quads for models/fluids/translucent; vertex pulling from sub-allocated
storage buffers; reverse-Z infinite projection with camera-relative origins; CPU cave culling;
two-phase Hi-Z GPU occlusion culling with indirect-count draws on Vulkan (CPU draw lists
elsewhere); translucent back-to-front with near re-sorting; headless screenshots with a GPU vs
CPU culling pixel check.

### Frame (V2-1, `scene.rs`)
1. Atmosphere lookup tables (compute): transmittance 256×64 and multiple scattering 32×32 when
   the aerosol density changes, the camera's sky view 256×128 every frame.
2. Opaque and cutout terrain into an HDR target (RGBA16F), GPU-culled.
3. Distant (LOD) terrain beyond the full-detail area.
4. Sky pass where nothing was drawn: sky view, sun and moon discs (the moon lit by its phase),
   stars rotating about the celestial pole, a cloud layer drifting with the wind.
5. Translucent terrain, then rain and snow.
6. Highlight metering (compute) and tonemapping (ACES) with a night shift toward blue.

### Atmosphere (`atmosphere.wgsl`, D29)
After Hillaire (2020): Rayleigh, aerosol (Mie, Cornette–Shanks phase) and ozone on an
Earth-sized planet, with the planet's shadow for twilight. The multiple-scattering table is
stored as a logarithm because in twilight it falls tenfold every degree or two. The same model
runs on the CPU (`hearth_env::sky`) to give the lighting in lux; a GPU test integrates the
sky-view table and checks it against the CPU within 20 % from high sun to the sun 8° down
(they agree within 10 %). Calibrated against measurements (D32): about 100 klx at a clear noon
with 10–20 % from the sky, ~750 lx at sunset, 4 lx at the end of civil twilight, 0.3 lx two
degrees later.

### Lighting and exposure (`terrain.wgsl`, `scene.rs`, D33)
Lighting is physical and pre-exposed: direct sun (or moon) on faces open to the sky (sky light
15; there are no shadow maps yet), sky irradiance scaled by the voxel sky-light level and face
orientation, a floor of starlight and airglow, firelight from the block-light level.

**Aerial perspective** (`common.wgsl`, D53) is physical and has nothing to do with how far the
terrain is loaded: the light of a surface is dimmed by the air on the way (Rayleigh per colour,
aerosol grey, both integrated through an exponential atmosphere at the real altitudes of the
two ends — heights are blocks of 1/vertical-scale metres) and replaced by the light the air
scatters toward the eye, which tends to the sky's colour in that direction (or the grey of a
cloud deck and falling rain or snow). The aerosol grows with the weather's haze, which grows
with humidity and precipitation; rain and snow add their own extinction. On a clear day
distant land turns hazy blue and stays visible for tens of kilometres. The ground also drops
with the planet's curvature, d²/2R with R = Earth's radius × the vertical scale, so horizons
lie where they would from the equivalent real altitude. The eye adapts to the horizontal
illuminance (brightening in about a second, darkening more slowly) down to full-moon light;
below that scenes are simply dark; in dim light it compensates only partly, so dusk looks like
dusk. A histogram of the frame then darkens it just enough to keep the bright end (90th
percentile) in the tonemapper's colourful range — sunsets keep their colours and the ground
goes to silhouette — with no readback (the tonemap pass reads the result directly).

### Distant terrain (`hearth_lod`, `lod.rs`, `lod.wgsl`, D52, D54)
Beyond the full-detail cubes the land continues as LOD tiles out to the LOD distance, or to the
horizon when that is farther (so the land never stops short of the skyline):
- **Tiles**: a quadtree over the wrapped planet, 32×32 LOD columns per tile, a column of level
  L being 2^L blocks (level-7 tiles are 4,096 blocks, and every planet circumference is a
  multiple); a tile is split while the camera is within four tile sizes of it, so columns stay
  a few pixels wide. About 1,000–1,600 tiles and 1–2 M quads out to 8–40 km, built in
  0.3–0.7 s on 16 threads.
- **Columns** are sampled straight from the surface sampler (never by generating cubes): the
  top block from the soil and rock models, water with its tint and the bed showing through
  the shallows, forests as a canopy roof in leaf colour over a shaded forest floor. Vertices
  carry the texture's average colour, the tint kind and the climate code, so the LOD shader
  colours grass and leaves by season with the same functions as the full-detail terrain, and
  lays seasonal snow and sea ice.
- **Meshes**: row-merged tops, the sides that show, skirts along tile edges.
- **Handoff**: across an 8-block band at the edge of the full-detail area the cubes thin out by
  an ordered dither while the LOD, drawn a hair behind them in depth, shows through their
  gaps; inside the area the LOD gives way entirely. The LOD shares the terrain's globals
  (lighting, aerial perspective, curvature).
- **Streaming**: the preview re-selects tiles when the camera moves 16 blocks and builds the
  missing ones nearest first on a small thread pool of its own; screenshots build every tile
  before rendering and fail if that takes longer than `lod_timeout`.
- **Regression test**: `lod_horizon` renders from a peak at LOD distance 512 with aerial
  perspective on and off, reads back the depth buffer, and requires that more than 30 % of the
  pixels below the horizon lie beyond the full-detail area and fewer than 0.2 % show no ground
  (it measures 43 % and 0.01 %).

### Seasons and weather (D34, D37)
- Grass and leaf colours from a climate code per quad and the date (`docs/design/seasons.md`);
  deciduous leaves thin out and show twigs as they fall, in the cutout pass.
- Clouds: a 2D layer at the weather's cloud base, lit by the sun and sky. Under a thick deck or
  in rain and snow, the sky, haze and distance fade to the grey of the cloud base.
- Rain and snow: particles generated in the vertex shader in a 48×32×48 m box that wraps
  around the camera (fixed in the world), up to 40,000 at full intensity, falling with the
  wind; a 128×128 map of the highest sky-blocking block per column hides them under roofs,
  trees and overhangs, and the depth test behind terrain. Falling rain and snow add extinction
  from the visibility they leave (~4 km in a heavy shower, ~600 m in heavy snow).

## Parameters
Atmosphere constants in `atmosphere.wgsl` and `hearth_env::sky` (kept equal; the
`sky_consistency` test guards them); exposure constants in `scene.rs` and `post.rs`
(adaptation floor 0.3 lx, highlight target 1.2, at most 16× darkening); particle counts and
box in `precip.rs`.

## Known simplifications
- No shadow maps: direct light reaches faces with full sky light, so there are no cast shadows
  from trees or overhangs yet beyond the sky-light falloff.
- LOD tiles are heightfields (no overhangs), are not cached on disk, do not yet reflect edits
  to the world, are frustum- but not occlusion-culled, and have no VRAM budget; LOD water is an
  opaque tinted surface (the water shader comes with V2-2e). No TAA.
- Clouds are a single textured layer (no volumetric clouds, no cloud shadows on the ground).
- No lightning, fog banks, wet or snowy surface shading, puddles or splashes yet.
- Rain streaks are thin and alias at a distance.

## v2 extensions (planned)
Water and ice rendering (V2-2), passable foliage and branch models (V2-6), LOD showing
vegetation state and the rest of v1 M8 — disk cache, edits, occlusion culling, VRAM budget,
TAA (V2-6), instanced animals (V2-7), fire, smoke, glowing hot items (V2-5), shadow maps and
volumetric light (V2-16 at the latest).
