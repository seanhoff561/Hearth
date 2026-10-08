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
5. The scene so far copied for the water (only the part of the screen the translucent cubes
   cover, plus a margin), then translucent terrain — water shaded as below — and rain and snow,
   with the depth buffer read-only so the water can read it.
6. Highlight metering (compute) and tonemapping (ACES) with a night shift toward blue,
   dithered into the 8-bit output; under water, the view through the water first. At a render scale other than 1 (`render_scale`) the scene
   renders at the scaled size, is tonemapped at that size, then upscaled with FSR 1 (EASU,
   then RCAS sharpening) or, above 1, filtered down; the dither comes last either way.

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

### Water (`water.rs`, `water.wgsl`, `terrain.wgsl`, D62)
Water keeps its blocky levels; waves are shading. **Waves**: a 256² tiling field of wave slopes
built at startup (48 wind-driven waves with whole numbers of crests across the tile, longer
ones taller), mipmapped by averaging so distant water flattens rather than glitters at random,
sampled at two scales (a 40-block swell and an 11-block chop at an angle) drifting with the
weather's wind and steeper as it blows harder. **Surface** (shader quality Medium, the
default): Schlick's Fresnel (2 % head-on) between the water body and the sky a mirror would
show (the sky-view table, grey under a cloud deck); the sun's glitter by GGX on a surface as
rough as the wind makes it, and rougher with distance where the waves average out, so a glitter
path stays at sunset; the scene behind refracted by the waves where the water is deep enough
to bend it and absorbed along the path through the water (Beer–Lambert, 0.45 / 0.07 / 0.035 per
block for red, green, blue in clear water, more in green and brown water by the tint), with the
light the water scatters back (6 % of its tint) taking its place — turquoise over sand, navy
over the deep; and foam where the water thins against the shore, in patches that drift with the
waves. Low draws a translucent surface with the same reflections and glitter but no refraction
(no copy). High adds **screen-space reflections**: the reflected ray (of a surface calmer than
the ripples, which blur reflections on real water) marched across the depth buffer in 20 steps,
denser near the water (reverse-Z depth is linear in screen space), refined by bisection where
it passes behind a surface, ignoring things lying on the water right by it (lily pads) and
fading toward the screen's edges, the ray's end and where it passes well behind what it met;
the shores, trees and hills it finds replace the sky's reflection, which remains the fallback
(the copy then reaches the top of the screen). From below, the world above shows through Snell's window and the water below is
reflected outside it (past 48.6°). **Distant water** (LOD) gets the same Fresnel, sky and
glitter over the swell, on a colour the column is given the same way (its bed through 1.5 times
its depth of water and the scattered light), so near and far water meet without a seam and the
shelf's turquoise darkens to navy past its edge.

**Under water** (D63). A map of the water surface over each column in 256 × 256 blocks around
the camera (the highest sky-blocking block where it is water, rebuilt with the rain's sky map)
tells each terrain vertex how deep under water it lies. There the light that reaches it falls
off exponentially with real depth (0.35, 0.07, 0.045 per metre for red, green, blue: sunlit
turquoise shelves, deep blue slopes, black below a few hundred metres) instead of the voxel sky
light's two levels a block, the sun arrives refracted toward the vertical, and caustics play on
the floor: a 256² map made at startup by bending light through the same wave field and
gathering where it lands, two drifting layers of it, fading out with depth. With the camera
under water (below the surface over its column), the tonemap pass dims each pixel along its
view through the water — to what it shows, or to the surface overhead where the view leaves the
water — at 0.40, 0.08, 0.06 per metre and adds the light the water scatters toward the eye
(blue-green, from the light at the camera's depth), and the eye adapts to the dimmer light.

**Ice** (lake, river and sea ice of the seasonal cover) keeps its texture's colour and
translucency under a Fresnel reflection of the sky (1.8 % head-on, ice's index 1.31) and a sharp
glint of the sun on its smooth face, growing more opaque where it reflects more; the water under
it shows through it with its own shading, so thin lake ice reads as dark, glossy black ice. Far
sea ice is the LOD's sea-ice colour, without the water's reflections.

### Distant terrain (`hearth_lod`, `lod.rs`, `lod.wgsl`, D52, D54, D274–D278)
Beyond the full-detail cubes the land continues as LOD tiles out to the LOD distance, or to the
horizon when that is farther (so the land never stops short of the skyline):
- **Tiles**: a quadtree over the wrapped planet, 32×32 LOD columns per tile, a column of level
  L being 2^L blocks, from roots of the coarsest level (at most 10, tiles of 32,768 blocks)
  whose tiles go round the planet a whole number of times; the coarsest levels, whose columns
  read the terrain's coarsest refinement levels, are the far field out to the real horizon
  (E §9.2, D278: from 3 km up on Earth some 2,200 tiles reach 230 km). A tile is split while the camera is within four tile sizes of it, so columns stay
  a few pixels wide, and rough tiles further (up to three more levels) until the steps between
  their columns stray no more than `lod_detail`'s limit on screen from what finer columns
  would show (Fancy 2 px, Fabulous 1 px, Fast 4 px; D57). A tile split before stays split a
  little longer (hysteresis), and neighbours stay within a level of each other, so the skirts
  along tile edges seal every border. About 1,000–2,000 tiles and 1–3 M quads out to 8–40 km,
  built in 0.5–1 s on 16 threads.
- **Columns** are sampled straight from the surface sampler (never by generating cubes): the
  surface's height in sixteenths of a block, the top block from the soil and rock models,
  water with its tint and the bed showing through the shallows, and the seasons of snow and
  ice from the near cover's year model (D277). Natural ground is coloured by the near ground's
  material table (each material's mean, D275), other blocks by their texture's average colour;
  the tint kind and the climate code colour grass and leaves by season with the same
  functions as the full-detail terrain.
- **Trees** (D56), as in Distant Horizons: on the finer levels (columns of up to 8 blocks,
  out to about a kilometre) each tile's real trees are grown by the world generator's own tree
  code (`FeatureGen::grow_trees` into a `TreeSink`) into a block-resolution map of the canopy,
  so the trees in the distance are the trees the cubes will have and the handoff hides them
  nowhere. A column shows a crown — a box of leaf colour floating from the lowest to the top
  leaves — when its leaf cover beats a threshold drawn per column between 0.1 and 0.9, so on
  average crowns cover as much ground as the leaves (a lone tree does not fill a whole
  column); the ground under a crown is lit as shade; within 512 blocks each trunk stands at its
  own block in bark colour. Coarser levels, whose columns are wider than a crown, raise a
  crown at the usual height of the place's trees, on every column where they close over the
  ground and on as many columns as their cover where they are sparse; there the crowns are a
  canopy surface (D276), rounded toward a stand's edge (conifers pointed), bare in winter and
  the dry season, snowy with the ground.
- **Meshes** (S4, D274): the ground a smooth height field of 33 × 33 corners a tile (each the
  mean of the four columns about it, with the field's normal) with skirts along its edges,
  every tile one instance of one draw; the canopy surface likewise, through the columns'
  middles, cut where its cover falls below a half; crowns on the fine levels as boxes with the
  faces no neighbour crown hides, and trunks as one-block boxes, as quads stored grouped by the
  way they face, each frame only the groups that can face the camera drawn.
- **Handoff**: across an 8-block band at the edge of the full-detail area the cubes thin out by
  an ordered dither while the LOD, drawn a hair behind them in depth, shows through their
  gaps; inside the area the LOD gives way entirely. The LOD shares the terrain's globals
  (lighting, aerial perspective, curvature).
- **Streaming**: the preview re-selects tiles when the camera moves 16 blocks, or when a
  tile arrives rough enough to split (a tile's error is known once it is built), and builds
  the missing ones nearest first, those in view before those behind, on a small thread pool of
  its own; until a tile is ready the tiles it replaces stay drawn. Screenshots and the
  benchmark build every tile before rendering, in rounds (select, build, select again with the
  new errors) and fail if that takes longer than `lod_timeout`.
- **Regression test**: `lod_horizon` renders from a peak at LOD distance 512 with aerial
  perspective on and off, reads back the depth buffer, and requires that more than 30 % of the
  pixels below the horizon lie beyond the full-detail area and fewer than 0.2 % show no ground
  (it measures 43 % and 0.01 %).

### Seasons and weather (D34, D37)
- Grass and leaf colours from a climate code per quad and the date (`docs/design/seasons.md`);
  deciduous leaves thin out and show twigs as they fall, in the cutout pass.
- Clouds: a 2D layer at the weather's cloud base (the lifting condensation level above the
  region's ground: 25 m per percent of humidity short of saturation, 600–3,000 m; E4), lit by the sun and sky. Under a thick deck or
  in rain and snow, the sky, haze and distance fade to the grey of the cloud base.
- Rain and snow: particles generated in the vertex shader in a 48×32×48 m box that wraps
  around the camera (fixed in the world), up to 40,000 at full intensity, falling with the
  wind; a 128×128 map of the highest sky-blocking block per column hides them under roofs,
  trees and overhangs, and the depth test behind terrain. Falling rain and snow add extinction
  from the visibility they leave (~4 km in a heavy shower, ~600 m in heavy snow).

### Globe

The world map's key opens the planet as a globe (`hearth_render::globe`, the minimal spawn
picker of v2 §16): an equirectangular map (2048 × 1024; each texel the biome at its point,
land shaded by its relief, made on its own thread in about half a second the first time it
opens) drawn on an orthographic sphere in one full-screen pass, with mipmaps sampled by
gradients that ignore the date line's jump, a graticule every 30°, a desk-globe light, the
camera's place and the point under the cursor marked, and a thin glow of air at the edge.
Dragging turns it (the land at the centre follows the cursor), the wheel zooms (up to 16×),
the window title describes the place under the cursor (latitude, longitude, climate, biome,
height or depth, temperature and rain) and a click goes there: to the nearest dry, gentle
column, and from the sea or a lake to the nearest coast (`Terrain::spawn_near`).

## Parameters
Atmosphere constants in `atmosphere.wgsl` and `hearth_env::sky` (kept equal; the
`sky_consistency` test guards them); exposure constants in `scene.rs` and `post.rs`
(adaptation floor 0.3 lx, highlight target 1.2, at most 16× darkening); particle counts and
box in `precip.rs`.

## Known simplifications
- No shadow maps: direct light reaches faces with full sky light, so there are no cast shadows
  from trees or overhangs yet beyond the sky-light falloff.
- LOD tiles are heightfields (no overhangs, S §5's smooth shelves not yet drawn); their quads
  (crowns, trunks) are occluded by the near terrain on the GPU, their ground and canopy only
  by the frustum; flat tiles draw as many triangles as rough ones. No TAA.
- Water: screen-space reflections only at High (Medium reflects the sky, also where trees
  stand over far water), without temporal smoothing they flicker a little on ripples; waves
  follow the wind and not a river's flow; under water no god rays, bubbles or muffled sound, one kind
  of water (clear sea) for the view through it, and flooded caves under land are not
  recognised (their columns' tops are dry).
- Clouds are a single textured layer (no volumetric clouds, no cloud shadows on the ground).
- No lightning, fog banks, wet or snowy surface shading, puddles or splashes yet.
- Rain streaks are thin and alias at a distance.

## Still to come
The smooth terrain and its materials (Amendment S, S1–S8), shadow maps and volumetric light
(S2–S8, V2-16 at the latest). Foliage, TAA, instanced animals, fire and smoke are built; see
`flora.md`, `fauna.md` and `fire-and-food.md`.
