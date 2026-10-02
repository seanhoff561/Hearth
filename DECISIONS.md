# Decisions

Each entry: the decision, why, and what would make us revisit it.

## D1 — Toolchain and core crate versions (2026-09-30)
Rust 1.98.1 stable (installed via rustup; MSVC target). wgpu 30.0.1, winit **0.30.13** (latest
stable; 0.31 is still beta and egui-winit targets 0.30), glam 0.33, smallvec 1.x (2.0 is beta).
Versions were checked with `cargo search`/`cargo info` rather than memory.

## D2 — ECS: `hecs`
`hecs` over `bevy_ecs`: tiny dependency footprint and compile time, no scheduler we would fight
(the server already has its own fixed-tick scheduler), archetype storage with fast iteration,
and entities/components are plain Rust types that are easy to expose through the mod API.
Revisit if we need change detection at scale (can be layered with tick stamps).

## D3 — Options live in `hearth_core`; key bindings stored as strings
`Options` is needed by render, UI, audio and client code, so it lives in the lowest crate. Key
bindings are persisted as an `action id → "ctrl+x"` map and interpreted by `hearth_input`,
keeping core free of input types. Only non-default bindings are written.

## D4 — Default key layout follows the spec, not the reference game
Shift = sprint, Ctrl = sneak, Left Alt = inventory, X = drop, Ctrl+X = drop stack, Mouse 5 =
swap offhand, Mouse 4 = perspective, M = map. In inventory screens the *drop binding* is used for
dropping (§11 mentions "Q" only because that is the reference game's default).

## D5 — Debug chords are rebindable actions in a `DEBUG_CHORD` context
F3+A/B/G/T/H/Esc/1/2 are ordinary actions that only fire while the debug key is held. They
never conflict with gameplay bindings (A = strafe left) and suppress the overlay toggle when
used, matching the reference behaviour.

## D6 — Game directory resolution
`--game-dir` → `HEARTH_GAME_DIR` → a `portable` marker file next to the executable → the
platform data directory (`%APPDATA%/Hearth/data` on Windows). Keeps `cargo run` from writing
saves into `target/`.

## D7 — Build profiles
`dev` builds dependencies at opt-level 3 and our crates at 1 (a voxel engine is unusable
unoptimised). `release` is fat-LTO, 1 CGU, panic=abort per spec. `dev-opt` (release without
LTO, 16 CGUs, incremental) is used for local benchmarks and screenshots; `dist` = release +
strip.

## D8 — Planet grid parameterisation (planned, M2)
The global planet analysis grid is an N×N grid over the Mercator world square with
sphere-correct metrics (cell size weighted by cos φ), instead of an icosphere/cube-sphere. It
covers exactly the playable world (±85°), wraps trivially in X, makes zonal sweeps (prevailing
winds, moisture advection, current classification) simple row operations, and maps 1:1 onto
world blocks. All noise is still sampled on the unit sphere, so geography is seamless.

## D9 — Order-independent features without a neighbour-wait population stage (planned, M2)
Features (trees, boulders, logs, cacti) are pure functions of their origin position and the
pure surface functions. Each cube gathers the features whose bounds intersect it, sorts them by
(layer priority, origin hash) and writes only its own blocks. This gives the spec's guarantee
("identical no matter which order cubes load in") without delaying cubes until neighbours
exist.

## D10 — Erosion split between the planet grid and analytic detail (planned, M2)
Hydraulic erosion (implicit stream-power law) and drainage are simulated on the planet grid,
which is where they shape valleys, basins and the river network. Below grid resolution the
erosion character (sharp ridges, dendritic gullies, talus) comes from an analytic erosion
filter — a pure function, seam-free by construction and evaluable at any LOD resolution.
Tile-local simulation cannot be both seam-free and consistent with the global river network
without blend artifacts in rivers.

## D11 — `hearth_protocol` crate (planned, M4)
The client↔server message types get their own small crate (not listed in the spec) so neither
side depends on the other's internals and a network layer can later serialize the same types.

## D12 — Planet grid resolution by planet size
2048² (≈4M cells) for Standard and larger, 1024² for Tiny/Small (cells ≤ 16 blocks there).
Standard gets 32-block cells, fine enough for dendritic valleys and dense river networks.

## D13 — Planet storage layout
Elevation and water at full resolution; smooth fields (climate, uplift, coast distance) at
half resolution (`Field::scale`, sampled with full-grid coordinates); river network sparse.
≈90 MB at 2048² instead of ≈240 MB. Saved as byte-plane-shuffled zstd; the tectonic layout is
regenerated from the seed on load.

## D14 — Biomes in code, colours from climate
Biome *selection* is inherently structural (climate class + altitude zones + local water/
slope/coast), so it lives in code. Grass/foliage/water colours come from temperature ×
precipitation colormaps. Per-biome decoration weights move to data packs in M12.

## D15 — Ore bands scale with vertical scale
Ore Y bands in the spec are Standard-planet numbers; like §6.3 they scale with the world's
vertical scale (clamped to 0.5–2×) so geology stays proportional. The deepslate transition
(~50 blocks under the local surface) is block-scale and does not scale.

## D16 — Data-pack templates
Block files may reference templates (`"template": "rock"`) defined in `_*.json` files; keeps the
~190 base blocks compact and gives modders the same mechanism.

## D18 — Screenshot mode renders three frames
Headless shots render the frame three times so GPU culling reaches its steady state (frame 1
draws everything in phase 1, frame 2 learns occlusion, frame 3 draws the survivors in phase 0);
`verify_cull=true` also renders with CPU culling and fails on any pixel difference. The same
check runs as the `render_cull` integration test whenever a capable adapter exists.

## D19 — GPU occlusion culling is Vulkan-only (for now)
On DX12, `multi_draw_indexed_indirect_count` doesn't add the base vertex / first instance to
`vertex_index` / `instance_index` (wgpu only patches these for plain multi-draws), which broke
vertex pulling (verified on WARP and on the RTX 4060 with DX12). Metal has no indirect-count
draws. Those backends use the CPU-built draw lists with identical output. Revisit if wgpu adds
draw-id or per-draw constants to indirect-count draws.

## D17 — Rivers and lakes at block level
River channels come from the grid network (jittered nodes, domain-warped query point for
meanders, monotone levels). Sea and lake water fill low ground only where the smoothed local
water fraction is high; low ground just outside gets a berm, so there are never water walls.

## D22 — Content files: RON first, Rust structs as schemas
Content is `(schema: N, entries: [...])` in RON (JSON accepted). The serde structs in
`hearth_content::schema` are the schemas; unknown fields are reported through `serde_ignored`
(serde's `deny_unknown_fields` doesn't mix with `flatten`, so entries avoid flatten and get their
standard fields from the `entry!` macro). Bare ids take the file's namespace while parsing.
Fixed-size arrays are written as RON tuples (`size_m: (0.1, 0.2, 0.3)`).

## D23 — Entries default to `Planned`
Data authored ahead of its system is hidden in game and only warned about by the lint; a
system marks the entries it uses `Implemented`, which makes reachability problems errors.

## D24 — Saves: JSON metadata, 8³-cube zstd regions
`level.json` is JSON because migrations edit a generic value tree and JSON keeps enum variant
names that RON's untyped values drop. Regions hold 8×8×8 cubes in 4 KiB sectors with
per-record zstd. Every generated cube is saved, so later world-generator changes never create
seams in existing worlds.

## D25 — Unknown content keeps its name
A world's block registry is built from the content plus placeholder blocks for every saved
state the content lacks, with the same name and properties. They render as "unknown" and
round-trip exactly, so removing and re-adding a pack loses nothing.

## D26 — Injury healing time scale
Injuries whose real healing takes more than about a week run on the year scale; shorter ones on
the day scale. A mild sprain (4 days, day scale) and a fracture (6 weeks, year scale) therefore
take comparable game days, which keeps both meaningful at the default calendar.

## D27 — What V2-0 removed, and what stays until its replacement
Removed: ore blocks and the ore bands, crafting table, furnace, chest, composter, cake, lantern,
glass, ladder, torches (they return as consumable light sources), wool/carpets/beds, bricks and
polished/cut/smooth stone variants, farmland, wheat, pumpkin, hay, sweet berry bush, planks and
wooden building sets, stripped logs, and the mining-tier block fields (`tool`, `tier`,
`requires_tool`, `drops`). Kept as transitional placeholders: the natural terrain, log/leaf and
plant blocks (until V2-2 geology and V2-6 flora replace them). Engine tests that need special
shapes or emitters use a separate test block pack (`hearth_world/testdata`).

## D28 — Knowledge dates as years before 1950
`history.years_bp` follows the radiocarbon convention (before 1950); inventions after 1950 are
negative. The human-readable `date` string carries the familiar form (BCE/CE, "million years
ago").

## D20 — v2 plan folds unfinished v1 engine work into the first milestone that needs it
v2 lists its milestones in order but they depend on engine parts v1 had not built yet (saves,
sky, weather, fluids, water rendering, client/server, UI, audio, LOD, ECS). Each goes into the
first v2 milestone that needs it (saves → V2-0, atmosphere → V2-1, water → V2-2, client/server
and UI/audio → V2-3, LOD → V2-6, ECS/animation → V2-7, resource packs and WASM → V2-15,
optimization/QA → V2-16), keeping v1's acceptance criteria for those parts.

## D21 — Technology timeline source
v2 §12.2 mentions a Britannica technology timeline supplied by the user; it was not in the
direction-change document. The knowledge graph uses the milestones and approximate dates given
in §12.2 and Appendix C, expanded into realistic intermediate steps from general
history-of-technology knowledge. Dates are approximate and marked in data.

## D29 — One atmosphere model on GPU and CPU
The sky is Hillaire's LUT atmosphere (transmittance, multiple scattering, sky view) on the GPU;
the lighting and exposure come from the same model on the CPU (`hearth_env::sky`, the same
tables and integration), so the sky and the terrain it lights can't drift apart. The
`sky_consistency` test integrates the GPU sky-view table and compares it with the CPU from
high sun to the sun 8° down. Two fixes came out of it: the transmittance and multi-scattering
tables need a clamping sampler (the sky view's wrapping sampler blended the horizon column with
the zenith column and lit twilight with daylight), and the multi-scattering table is stored as
a logarithm (linear interpolation smeared daylight into the dusk). The planet's shadow is
applied to single scattering explicitly.

## D30 — Weather in two layers
What the player sees (clouds, rain, snow, wind, the day's temperature) is a day-scale field
advected over the sphere whose wet fraction follows the seasonal climate. What accumulates
(snowpack, lake ice) is integrated on the year scale from the climate normals, not from the
events the player happened to see. The snow is then right at any calendar speed and everywhere
at once, and the displayed weather never has to be simulated for the whole planet.

## D31 — The moon keeps its month-to-year ratio
The synodic month is scaled with the game year (12.37 months per year), not with the game
day: at the default 32-day year a full cycle of phases takes ~2.6 game days. Tied to the day it
would take 29.5 game days — almost a whole default year — and nights would sit in one phase
for a season.

## D32 — Aerosol calibrated to clean continental air
Hillaire's default aerosol (optical depth ≈ 0.005) gave half the sky light of measured clear
skies. Haze 1 is now optical depth ≈ 0.05 (ten times the aerosol), which matches the CIE clear
sky within ~15 % (12.7 klx diffuse with the sun at 60°) and published twilight illuminance
(750 lx at sunset, 3.4 lx at the end of civil twilight; the model gives 780 and 4.0). Humidity
and rain raise the haze. The terrain's aerial perspective uses the same extinction.

## D33 — Exposure: an adapting eye plus highlight metering
Lighting is in lux and pre-exposed. The eye adapts to the horizontal illuminance (about a
second brightening, slower darkening) down to 0.3 lx (full-moon light); darker scenes stay
dark. In dim light it compensates only partly (a key falling from 1 at 1000 lx to 0.3 at the
floor), so dusk reads as dusk. A GPU histogram of the frame then scales the exposure down just
enough to keep its 90th percentile at 1.2 (the tonemapper's colourful range), at most 16×,
adapting over a second; the tonemap pass reads the result directly, so there is no readback
and headless screenshots adapt instantly. The sun disc's radiance is scaled down to stay
within half-float range (it saturates after tonemapping either way).

## D34 — Weather rendering
Rain and snow are generated in the vertex shader from the vertex index in a box that wraps
around the camera, so they cost no CPU work and stay fixed in the world while the camera moves.
A 128×128 map of the highest sky-blocking block per column (the column heightmaps, streamed
from the world thread when the camera moves or terrain arrives) hides them under cover. Under a
thick cloud deck (above ~55 % cover) or in precipitation the sky, haze and distance blend to the
grey of the cloud base (the sky irradiance over π) instead of the clear sky's colour, and falling
rain and snow add extinction from the visibility they leave (Koschmieder, 3.9 / visibility).

## D35 — Vegetation colours use the dry-season strengths, not the climate class
The shader's dry-season type came from the Köppen class, so a winter-dry place too cool to be
"tropical savanna" (coldest month 15.8 °C) kept green grass through a dry season its weather
and phenology knew about. The type now comes from the planet's winter/summer dry-season
strengths with the same threshold as `Normals::is_dry_season`; only deserts and steppes use the
class (always arid).

## D36 — Seasonal snow and ice as block states, refreshed in steps
The generator lays only perennial snow and ice. Seasonal cover is laid on terrain when it loads
and refreshed on loaded terrain every five days of the year (the snow model's step): the old
cover comes off (plants it buried are remembered and come back, ice thaws to water) and the
date's is laid again; only blocks that differ are relit and remeshed. Snow layers don't change
light, so the refresh is cheap. The sea is left open until sea ice arrives with the coasts
(V2-2) — the lake-ice model would freeze oceans that real ocean heat keeps open.

## D37 — Leaf fall and snow on canopies
Deciduous leaves thin out in the cutout pass: a stable hash of each leaf texel's world position
is compared with the leaf cover from the same phenology formula the CPU uses, and a sixth of the
fallen texels stay as grey-brown twigs. No remeshing, no extra geometry. Snow follows the
canopy: bare deciduous crowns pass it through to the ground (three quarters of the open-ground
depth), conifer crowns hold two layers and shelter the ground beneath.

## D38 — Natural blocks are generated from content
Every rock type in `geology/rocks.ron` is a block of the same name, generated when the block
registry loads (`hearth_content::generate::natural_blocks` → `hearth_world::datapack`), with the
`rock` template, a hardness from the material's compressive strength and the material's map
colour, and a texture drawn from the material's appearance (`hearth_texgen::material`). A new
rock in data is a new block in the world with no code. The block loader reads the packs'
content itself, so every registry built from packs (game, tools, tests) has the same blocks;
pack JSON overrides a generated block of the same name. Blocks gained an optional `material`
field for the same purpose.

## D39 — Geology: provinces from the tectonic history, strata cut by today's terrain
Provinces are data (setting, sequence, basement, intrusions, folding, conditions, weight). A
cell's setting comes from the planet's tectonic codes; conditions and a weighted hash of a
warped region choose among the provinces of that setting. The sequence's top is the smoothed
regional surface plus the exhumation (0.6 of the uplift): erosion is what exposes old rock in
mountain cores, so modelling it keeps basins young at the surface and ranges old. Structures
are at the block scale (a block is a metre) while thicknesses scale like relief, because the
planet's 610× horizontal compression would otherwise turn every dip vertical.

## D40 — Generic stone and deepslate are gone
`stone`, `deepslate` and `calcite` were removed from the base pack (and the rock-variety veins
from the generator): every underground block is now a real rock type. Engine tests that need a
plain opaque block use `stone` from the engine test pack (`hearth_world/testdata`); saves that
contain the old blocks keep them as named placeholders (D25), as the format-2 fixture checks.

## D41 — Soils by formation fit, at the block scale
A soil is chosen per column by scoring the formations in `geology/soils.ron` against the
place: listed climates, parent rocks, drainage and slope limits must hold; exact climates beat
groups, a parent-rock match beats vegetation (rendzina, terra rossa, podzol and andosol are
what their rock makes of the climate), landforms (floodplain, wetland) beat plants; a soil with
no conditions is the fallback. Horizons stay in real metres because soil is a local thing (a
metre per block), which makes profiles one to a few blocks deep, thinned by slope. v1's
biome surface kinds still say what covers the ground (turf, sand, gravel, rock, snow); the soil
decides what lies under it.

## D42 — Deposits: Poisson bodies per 256-block cell, frequencies at game scale
Deposit models are data (`geology/deposits.ron`: resource, geometry, provinces, host rocks,
conditions, depth, size, thickness, extent, grade, frequency, indicators, era). The generator
draws a Poisson number of candidates per model in every 256-block cell (mean = frequency ×
cell area) and keeps those whose province, conditions and host rock hold, so a frequency is
bodies per km² of *suitable* ground. Place conditions (river, lake, wetland) are ORed and
searched for inside the cell; the others (arid, humid, warm, cold, near a volcano, near
granite — for placers within 500 blocks upstream) must all hold. A cell's bodies are
computed once and cached; each cube draws the bodies that reach it, so generation order does
not matter. Depths of buried bodies scale with the vertical scale like the strata; surface
bodies (crusts, flows, bogs, placers) stay at the block scale. Frequencies are far above
Earth's: the Standard world compresses Earth ~610× horizontally, so a province patch of a
kilometre stands for hundreds of real kilometres, and deposits must be findable at the game's
scale while staying rare against the rock around them.

## D43 — Resource blocks derive from the content
A deposit body is made of its resource's block: a rock resource is the rock (and its
`<rock>_cobbles` on stream bars and as float), an earth or pigment is its earth block, an ore
mineral is `<mineral>_ore` (the rock matrix flecked with it) — or, in crusts and bog ores,
`<mineral>_crust` (an earthy crust of it) — with `<mineral>_placer` (gravel with its heavy
grains) and `<mineral>_float` (loose pieces). Stains and gossans are the ore block of the
indicator's mineral. Names, hardness, map colour and texture come from the material, so a new
deposit model in data brings its blocks with no code.

## D44 — Resource coverage: workable bodies, reach limits by era, substitutes
The V2-2 acceptance ("each Era 0–5 resource reachable from any continent within a reasonable
distance at Standard size, or the lint flags the gap") is measured as follows. A census of
every body on the planet keeps those that can be worked in the resource's era (no more than
4 blocks of ground over them in Eras 0–2, 12 in Era 3, 40 from the Bronze Age on). Continents
are landmasses of at least 50 km² (scaled with the planet's area). For each continent and
resource, the distance within which 90% of its land lies from such a body (a Dijkstra
distance over 256-block cells, across the sea too) must be within the era's reach: 5 km for
Eras 0–2 (a long day's walk), 8 km for Era 3, 12 km from Era 4 (a journey), scaled with the
circumference. A resource out of reach is covered when a substitute is in reach: a resource
yielding the same metal (so metals were added as planned materials and ore minerals given
their yields), a mineral of the same formula (pyrite and marcasite), or one whose material
shares a tag ending in `_ore` or a tag item forms and processes select by (`knappable`).
`hearth content lint --coverage` generates Standard worlds and reports open gaps as warnings
and covered ones as notes; `bench deposits --coverage` and `bench worldmap` print the table.
Gaps that remain are resources that are regional in reality (kimberlite, coal, volcanic
sulfur and ash, travertine, fire clay).

## D45 — Province climate conditions are preferences, not rules
Today's climate is only a weak guide to where rocks formed (continents drift, climates
change). Structural conditions (coastal, inland, old, young) still decide which provinces
may occur; climate conditions (arid, humid, warm, cold) now multiply a province's weight by
0.15 where they do not hold. Coal measures and evaporites keep favouring today's wet and dry
lands but are no longer confined to them, which spreads coal, salt and gypsum across
continents as on Earth.

## D46 — Plutons per province: how many, how deeply eroded
Provinces set the share of pluton sites that hold an intrusion (default 0.35) and the depth
of the plutons' roofs below the top of the sequence (real metres × vertical scale; default
−120 to 1,000). Old shields (0.8; −500 to 250 m) and old orogens (0.6; −400 to 500 m) are
eroded deep enough to expose their granites, which is where kaolin and tin belong. Roofs
are flat with steep walls (600 m × the vertical scale of extra depth at the rim, rising with
the fourth power of the distance from the centre).

## D47 — Resource eras follow the technology eras
A deposit's era is the first technological era (v2 §12) that uses its resource: toolstone
Era 0; pyrite and marcasite, ochres, manganese black and clay Era 1; salt and gypsum Era 3
(salt making and plaster are Neolithic); copper, tin, gold, silver, cinnabar, kaolin and
fire clay Era 4; bloomery iron ores, zinc, coal, sulfur, travertine, pozzolana, garnet and
diamonds Era 5; saltpetre and anthracite Era 6; bauxite Era 8. The coverage lint checks
Eras 0–5.

## D48 — The water table is a subdued copy of the land; voids below it are water
Groundwater is a static field computed from the terrain: the drainage base (lowest ground or
water within 256 blocks, from the planet grid), the land smoothed over ~100 blocks, the share
of relief the table follows (set by the surface rock's new `permeability` in the rock data —
karst drains to the valley floors, tight rock carries water up under the hills) and the
climate's wetness. It never rises above the ground; where it would, springs rise. Every cave
void below it is flooded (the random "aquifer" floods of v1 are gone): realistic, and it gives
the underground regional character — dry caves in hills, karst uplands and deserts, sumps
under wet lowlands — at the cost of many of v1's deep caves now being water. Mines below the
table will flood (V2-13). Computing the drainage base from the planet grid rather than from
terrain samples keeps the cost to about 5 % of surface cube generation.

## D49 — Coast types from temperature, depth, slope and shelter
Sheltered coasts are marked by a slow noise and river mouths (not the coastline's shape);
low sheltered coasts become mangrove (tropics) or salt marsh with mudflats; reefs need warm,
clear, shallow water and take the waves (a reef column is never sheltered). Reefs raise the
terrain in the sampler itself, so every system (biomes, soils, features, maps) sees them.
Mangrove and salt marsh are biomes (36, 37); coral, reef, mangrove, cordgrass and wrack blocks
are data with generated textures; species-level corals and mangroves arrive with V2-10.

## D50 — Sea ice from air temperature below −4 °C
The seasonal cover integrates sea ice like lake ice (Stefan's law) but only below −4 °C of air
temperature and melting above −1.8 °C: sea water freezes at −1.8 °C and the heat stored in
the sea delays freezing. This closes D36's open sea: pack ice where the warmest month stays
below −1.8 °C, seasonal ice where winters are long and cold, open water where they are mild.

## D51 — Two v1 feature bugs fixed with V2-2
Tree and debris placement drew their chance from 8 and 4 bits of a hash (`h >> 16`,
`h >> 20` fed to a function that reads the top 24 bits), so any non-zero tree density grew a
closed forest (steppe, savanna, plains and scrub were woodland) and cacti and fallen logs
filled every eligible cell; they now use the full hash. Underwater plants were never placed:
the feature writer ranks water above every feature, so seagrass and kelp could not take its
place; water plants (and mangrove wood) now may, and nothing else, so canopies still cannot
dip into lakes.

## D52 — Distant terrain pulled forward from V2-6, and it reaches the horizon
Screenshots showed the terrain fading out at a fixed distance. Diagnosis: (a) the terrain
shader blended everything to the sky between 60 % and 95 % of the loaded radius (a
render-distance fog set by the preview and the screenshot tool), and (b) — the root cause —
there was no LOD terrain at all: v1 M8 had been scheduled with V2-6 (MIGRATION.md) and the
`lod_distance` option was read by nothing. The harness could not have waited for LOD (c), and
the projection was already reverse-Z with an infinite far plane (d). The fix builds the core
of v1 M8 now (`hearth_lod`: quadtree, fast surface sampling, meshing; `hearth_render::lod`;
streaming in the preview; screenshots wait for every tile and fail after `lod_timeout`) and
removes the render-distance fog. The LOD distance setting is a minimum: tiles always reach the
geometric horizon (√(2Rh), R = Earth's radius × vertical scale), because a fixed LOD edge
closer than the horizon would be the same hard cutoff one step farther out; distant tiles are
coarse, so this costs a few hundred tiles. Vegetation state in the LOD, the disk cache, edits,
occlusion culling, the VRAM budget and TAA stay with V2-6.

## D53 — Aerial perspective is physical, not tied to any distance setting
The single grey extinction coefficient and the render-edge fade are gone. Aerial perspective
integrates the atmosphere model's own coefficients along each view ray — Rayleigh per colour
(scale height 8 km) and aerosol (1.2 km; its amount follows the weather's haze, which grows with
humidity and precipitation) at the real altitudes of the two ends — plus the extinction of
falling rain or snow, and fills in the sky's colour in that direction (or the cloud deck's
grey). Clear days show distant land hazy blue tens of kilometres away; muggy and wet days
close it in. Planet curvature (d²/2R) is applied to the full-detail and LOD terrain alike.

## D54 — The LOD handoff: cubes dither out over a band, the LOD shows through from behind
A symmetric dithered crossfade (each side keeping complementary pixels by its own surface's
position) leaves holes where the two surfaces at a pixel lie at different places, as with
individual trees against a forest canopy block. Instead the full-detail terrain dithers out
across an 8-block band while the LOD draws everywhere outside the area's interior, pushed a
hair farther in depth so the cubes win wherever they are drawn: rays leave the camera outward,
so any gap in the cubes is backed by LOD terrain. Forest LOD columns are a canopy roof over a
shaded forest floor rather than solid blocks, so rays passing under the roof's edge where the
real trees end meet ground, not sky.

## D55 — River levels follow basin regimes, and floods thin out instead of standing in walls
A river's seasonal level comes from its whole upstream basin, not the climate where it flows:
each river cell of the planet grid contributes the runoff regime of its own climate, weighted
by the water it adds, and the regimes are summed down the drainage network with the flood
wave's travel time. A local regime alone would dry the lower Nile every year; the basin mix
floods it from the Ethiopian rains. The sum is done on Fourier coefficients (a delay is an
exact phase shift) with all harmonics of the 73-step year, then stored as a series per reach
(2 bytes a step): cheap (22k reaches, 0.07 s, 3 MB on a Standard planet) and exact, where
time-domain shifts would blur floods reach by reach and a few harmonics would ring. Two
liberties in the water balance: runoff comes from a landscape of five temperature bands
(±4 °C) rather than a point, which spreads snowmelt floods over weeks as real basins do, and
5 % of rain runs off whatever the season's balance (storm runoff), without which semi-arid
places run off only in their few coldest weeks. The ground's share of runoff (baseflow) scales
with the groundwater model's wetness, so streams of dry lands lose their water and run dry.
Travel times use the real distances the planet's rivers stand for (Earth's circumference over
the planet's), as the climate uses Earth's.
On the terrain, the stage follows Manning (depth ∝ flow^0.6), rises half as fast over the
banks and at most half a block plus a quarter of the channel depth above them — full-scale
flood stages (several metres) at a vertical scale of 0.25 stood blocks above the plains. The
flood level is only known within the river's banks zone, so where it would stand above the
land around the river, it thins toward the zone's edge (0.5 blocks per block) to meet that
land: water spreading over a plain as a film too thin to show, rather than a wall at the edge
of the zone.

## D56 — Distant trees are the generator's trees (as in Distant Horizons)
The first LOD drew forests as a flat roof of leaf colour wherever the tree density passed a
quarter, and nothing elsewhere: scattered trees (parkland, savanna, open woodland) vanished at
the edge of the full-detail area and forests became green slabs. Following Distant Horizons,
whose distant terrain is built from what the world generator actually places, the finer LOD
levels now grow each tile's real trees with the generator's own tree code: the tree writer
became generic over a `TreeSink` (the cube writer with its priority lattice, or the LOD's
canopy map), with the cubes' output unchanged byte for byte (checked on 634 forest cubes).
Tree positions, species, shapes and heights match the full-detail trees exactly, so trees keep
their place across the handoff; a test checks that the distant canopy and the generated
cubes' leaves agree on at least 95 % of columns. Two liberties: a column shows a crown when
its leaf cover beats a per-column threshold between 0.1 and 0.9, which preserves the canopy's
cover on average instead of blowing every lone tree up to a whole column (max-sampling made a
parkland look three times denser at 512 blocks); and levels coarser than 8-block columns
(beyond about a kilometre, where a crown is under a column) do not grow trees, which would cost
seconds per tile, but estimate the cover from the tree density and the biome's usual trees.
The cost is geometry: about +50 % LOD quads and +0.2–0.3 ms of GPU time in a forest at
1080p, recovered by the performance audit's LOD work.

## D57 — Distant terrain detail follows its error on screen; the default allows 2 px
The LOD chose levels by distance alone (a tile splits within four tile sizes of the camera,
keeping columns 3–6 px wide at 1080p). Measured on the benchmark's views, that left the steps
of rough land standing up to 3–46 px from what finer columns would show: mountainsides as
coarse terraces, trees on slopes below a summit as blocky lumps. Now each tile records its
vertical error and rough tiles are split until the error on screen is within a limit (the
audit asked for about 1 px), with hysteresis and the selection balanced so tile borders stay
sealed. The distance rule remains the floor: flat land is not made coarser, because columns
are also colour and trees, and coarser columns blur both (the audit forbids quality loss at
the default). So the refinement only adds tiles, and costs frame time. Measured tiers: 4 px
costs almost nothing and changes little; 2 px fixes the terraces and blocky trees for
+0.5 ms of GPU time on the summit (−27 % average FPS there, −6 to −10 % in the other open
scenes); 1 px also refines distant ridges, for +1.5 ms (−53 %). The default (Fancy) allows
2 px — the bound is on the largest step of each tile, so typical steps stay well inside it —
Fabulous 1 px, Fast 4 px (`VideoOptions::lod_detail`). This is the first change to fall more
than 5 % below the previous benchmark, deliberately: a visible quality gain at a measured cost.
The next audit step (LOD quads grouped by face, back-facing groups skipped) won it back:
the summit runs at 805 FPS with the 2 px detail, against 810 with the distance rule before.

## D58 — Render scale upscales spatially (FSR 1) for now; temporal upscaling waits for TAA
The audit asked for render scale with a quality upscaler as an option, never the default at
high presets. A temporal upscaler (FSR 2/3, or our own) needs motion vectors, a jittered
projection and history handling that the renderer will only have with TAA (V2-6), so the
render scale upscales with FSR 1's two spatial passes (EASU, RCAS), written from AMD's
published MIT algorithm, and filters down above 1. It is off (1) in every preset: at 1080p on
the reference laptop the frame is bound by vertices and the CPU rather than by pixels, so 0.67
gains only 2–16 % and costs visible fine detail (SSIM 0.79–0.98 against native); it is for
weaker GPUs and higher resolutions. When TAA lands, a temporal upscaler replaces EASU behind
the same option.

## D59 — The performance gate builds the baseline and runs both, alternating
The audit asked for a gate at the end of every milestone: the quick benchmark scenes, failing
on a drop of more than 5 % in average FPS or 1 % lows, to be fixed or justified here. Stored
numbers make a poor baseline on a laptop: identical builds differ by 3–10 % from run to run
with clocks and temperature, more from day to day, and one driver hitch of 40–50 ms halves a
run's 1 % lows. So `scripts/perf-gate.sh` keeps the baseline as a commit (`perf/baseline`),
builds it in a git worktree next to the current tree, runs the two alternately (three rounds by
default, each with its own data packs and world cache), and compares the medians per scene
(`hearth bench --judge`). It fails when either median falls more than 5 % below the baseline's.
The 1 % lows are judged as the 1st percentile of frame rates (1000 / the 99th-percentile frame
time), not the average of the slowest 1 % that the reports also show: comparing a commit with
itself, half of the forest runs of both builds had one 10–30 ms driver hitch, which cut that
average to 160–330 FPS from about 570, while the percentile held within ±2 %; it still moves
with anything that slows more than 1 % of frames.
The baseline moves only on purpose (`--accept`, on a committed tree), so small regressions
cannot pile up milestone after milestone unnoticed: every accepted regression has an entry
here. Runs on another GPU are not compared, nor scenes whose definition changed (accept a new
baseline when a benchmark scene is redefined).

## D60 — The rendering audit's end state, slower tails on the summit and coast accepted
Measured against the audit's first commit in one session (alternating builds), every scene's
average frame rate rose (forest +13 %, summit +5 %, coast +10 %, underwater +13 %, cave
+89 %, storm +8 %), but the 1st-percentile frame rate of the summit fell 5 % (3 % at LOD
1024) and the coast's 7 %: about 0.1 ms on their slowest frames, which wait on the GPU. On the
summit the heaviest view directions draw the finer distant land of D57 (742 tiles instead of
454, 1.90 M triangles instead of 1.67 M) that grouping quads by facing could not fully pay for
there. The coast draws fewer triangles than before, and its GPU time is up 0.03 ms from the
passes every frame now runs, dithering and the LOD occlusion cull. Both are kept for what they
show or save elsewhere: the rough land's detail is the point of D57, dithering removed visible
banding, and the cull halved the cave's GPU time. This end state is the performance gate's
baseline.

## D61 — Finite water: litres and quality per block beside sustained reservoirs
v2 asks that oceans, rivers and lakes be sustained reservoirs while water the player moves is
finite, conserved, levelling and flowing downhill, with its quality per block. The reference
game's model (infinite sources, flow levels that are not volumes) cannot conserve anything, and
block states alone cannot either: containers hold litres (a pot, a bucket), a block a cubic
metre. So natural water stays as generated (`water[level=0]`, never simulated) and acts as a
constant-head boundary — it fills what opens beside it at its height or under it and takes what
comes down onto it — and finite water is a table of integer litres and qualities per block in
`WaterSim` (the truth), shown by the blocks in eighths (`water[level=1..8]`; the property shrank
from 0..15 and level 0 now means natural). Flow is a cellular automaton over the blocks that
changed: fall, then pairwise evening out (half the difference, in turn — the snapshot version
with a fifth of each difference oscillated once small differences were rounded up), then the
reservoirs; integer transfers make conservation exact. Because pairwise evening out stops at
one-litre steps (which add up along a channel) and cannot carry pressure through full water,
water at rest is levelled per connected body, rising at most a block a pass. Wells fill by
seepage below the static water table at a rate set by the rock's permeability; evaporation
(a warmth term plus the mass-transfer law, calibrated to desert lakes at about 10 mm a day — the
textbook mass-transfer coefficients alone gave 26) leaves salt as crusts the player can harvest,
which makes salt pans work. Coastal salt pans (sabkhas) appear on hot desert coasts, using the Köppen class
rather than a rainfall threshold: the planet's driest sheltered coasts get 230–270 mm a year,
and a fixed 250 mm cut kept all but a handful away.

## D62 — Water shading: refraction of a scene copy, sky reflections, the same model far away
v1 §9.3 asks for wind-driven waves on the blocky water, Fresnel reflections with a sky
fallback, sun glints, refraction absorbed by depth, shore foam and distant water shaded like near
water. The water is drawn in the translucent pass: before it, the part of the screen the
translucent cubes cover is copied (a full copy cost 0.05 ms at 1080p every frame, also in a cave
with no water in sight), and the pass holds the depth buffer read-only so the water can read
the depth behind it without a second copy. Waves are a tiling, mipmapped slope texture (the
audit's precomputed normals) rather than per-pixel wave sums. The water body is physical in
form (Beer–Lambert per channel, back-scattered light from the tint) but its constants are
chosen by eye. The LOD cannot refract (no scene behind its water in the same pass), so each
water column bakes what the near shader would show from above at a slant and the LOD shader
adds the same reflection and glitter; near and far water then meet without a seam. Shader
quality Low keeps the old translucent surface (with reflections); High is reserved for
screen-space reflections. Measured against the previous build (alternating, two rounds each,
1080p, Medium): average FPS forest −3.6 %, summit −6.7 %, coast −17 % (GPU 0.78 → 0.95 ms),
underwater −13 %, storm −5.4 %, cave −1.6 % — the cost of shading water where it covers the
screen, accepted for the look (SSIM against the old images 0.78 on the coast, 0.96 on the
summit, 0.99 in the forest).

## D63 — Under water: light by real depth from a map of water surfaces
The voxel sky light falls by two levels a block in water, so shallow shelves went dark a few
blocks down and the sun stopped at the first block. Underwater surfaces are lit instead by the
light that reaches their real depth (Beer–Lambert with the diffuse attenuation of clear sea
water), which needs to know where the surface is: a 256² map of water surfaces around the
camera, built from the columns' highest sky-blocking block where it is water (as the rain's
map), read per vertex (per pixel it cost 0.07 ms in the forest; per vertex 0.01). The voxel sky
light still gates it (a cave under the sea floor stays dark). The renderer cannot see the
world, so the camera is under water when it is below the surface of its column in the map, and
then the tonemap pass applies the view through the water (the depth gives the distance; the
surface overhead clips it), with the eye adapting to the light at the camera's depth. Measured
against the surfaces commit: −0.8 to −2.3 % in the open scenes, −4.4 % under water.

## D64 — Screen-space reflections on water only at High
SSR in the water shader makes a lake mirror its treeline. Measured at Medium (the default):
10 steps on every reflection that shows cost 0.07 ms in the forest and 0.13 ms on the coast;
8 steps on grazing views only, 0.04 and 0.10 ms. On the coast the rays find only sky (the
fallback already shows it), so the cost bought almost nothing, and in the forest the gain is
subtle and flickers a little on ripples without temporal smoothing (TAA, V2-6). So SSR is a High
(Fabulous) feature: 20 steps, −8.6 % average FPS in the forest and −17 % on the coast against
Medium; Medium keeps the sky's reflection and copies only the water's part of the screen.
Hits right by the reflection (lily pads on the water) are ignored, and rays are traced on a
surface calmer than the ripples.

## D65 — V2-2's performance gate: the water's cost at the default quality accepted
The gate (2e45a05, the end of the rendering audit, against the end of V2-2; Fancy at 1080p, the
quick scenes, three alternating rounds) failed first on 1 % lows: forest −29 %, cave −26 %. The
benchmark rebuilt the rain-cover map and, since D63, the 256² map of water surfaces on its frame
thread whenever the camera had moved 32 blocks, and the water map's 65,536 lookups take 1.4 ms
(bisected: the 1 % lows fell with D63's commit). The game never does that work on its frame
thread — the streamer builds both maps and the frame uploads them — so the benchmark now builds
them on a long-lived thread of its own, as the streamer does, and uploads them when they arrive.
The water's copy of the scene was also trimmed: it covers the screen box of the translucent
quads themselves rather than of their 16³ cubes, clipped where the box passes the camera's
plane instead of taking the whole screen (forest 0.046 → 0.036 ms, cave 0.014 → 0.004 ms).

What remains is the water itself at Medium, the cost D62–D63 measured step by step and accepted
for the look: average FPS forest −9.0 % (1 % lows −6.5 %), summit −8.0 % (−3.2 %), cave −3.2 %
(−3.3 %). GPU time per pass shows where: in the forest the sky, the water's scene copy and the
translucent pass take 0.254 ms where the sky and the old translucent water took 0.167 (water
shading +0.05, the copy 0.036); on the summit the distant water of the LOD pass +0.096 ms; VRAM
+17 MiB (the scene copy). The baseline moves to the end of V2-2. Frames stalled 5–50 ms inside
`queue.submit` (one to three in some runs) showed up in both builds alike and in a bisection
before V2-2 — the driver or the system, not this code; the gate's 1 % lows (the median run's
99th-percentile frame) are robust to them.

## D66 — The body: a core and a regional shell, needs on the day scale, one reference adult
`hearth_body` models the player's body (v2 §9) from data. Heat balance: Gagge's two-node model
with two changes. The shell is split into the eleven body regions, each region's skin solved as
the balance between the heat reaching it from the core and what it loses through its own cover;
with one mean skin, the cold hands, feet and neck of a well-dressed body drained the core as if
they were warm, and a person in furs by a fire froze. And the shell's conductance at full
vasoconstriction is the whole shell's tissue insulation (10 W/m²K) rather than Gagge's
skin-layer 5.28 W/m²K, with which a naked person shivering in 5 °C rain never became
hypothermic. Skin diffusion uses the skin's vapour resistance (0.6 m²kPa/W) instead of Gagge's
6 % of the evaporative capacity, which for a near-naked body in wind gave three litres a day of
insensible loss. Calibrated against human data in `tests/realism.rs`.

Needs, heat and short illnesses run on the day scale (a game day is a real day for the body);
injuries and illnesses use their data's scale; stamina is in seconds of play. Time to die of
thirst follows heat and work as it does in people — 3.3 days of hot, active days, about 11
resting in the shade — so "about three days" is a hot, active life, not a fixed timer. One
reference adult (the middle of the data's ranges) for every character, because height and build
are cosmetic (v2 §9.1). Garment insulation is local (clo where the garment covers): fur mittens
became 1.5 clo, where 0.4 was a whole-outfit share. Fractures in the data are closed (no
infection); an open one is a fracture and a deep wound.

## D67 — Movement: steps, scrambles and ledges instead of jumps; falls judged by their speed
A person steps up about 0.6 m in stride and jumps about 0.45 m, so a block-tall step cannot be
jumped as in other block games. Voxel hills are made of block steps, so a block (to 1.05 m) is
scrambled up when walking or jogging — slower for a moment and hard work — and ledges up to head
height (1.9 m) are climbed with both hands, strength and stamina, in one to two seconds; two
blocks are out of reach (rock faces with holds are a later feature). Landings report their
speed and the body decides the injuries (v2 §9.2): fractures from a fifth of 3 m falls, death
in half of falls from about 12 m (the commonly cited range). Water passes on about a third of
the speed of entering it. Breath: 45 s held, fainting 25 s after, drowning 60 s after; a body
that cannot act goes limp and sinks, so a swimmer chilled to unconsciousness drowns, as most
cold-water deaths go. Two physiological corrections came with it: shivering and work share one
ceiling of heat production (4.5 METs), and work thins the shell's insulation (K × (1 + 0.15 ×
METs above rest)), so treading water in cold water cools rather than holding warm.

## D68 — An integrated server owns the world and the body; the client moves the player
The preview became a game: a server thread at 20 TPS owns the world (generation, light, cover,
finite water), the clock and the player's body, and a client renders, mirrors the cubes it is
sent and moves the player itself every frame against that mirror, reporting twenty times a
second. Movement on the client keeps it as responsive as the frame rate (no round trip); the
body on the server lives in the weather, water and shelter of the world it owns. In process the
messages carry shared data (each cube's blocks with its mesh) rather than serialised bytes, and
the server meshes as the streamer did; a network transport will serialise cubes and mesh on the
client (`hearth_protocol` says so). The first spawn moved from temperate (35–55°, including
cold humid-continental climates) to warm-temperate lowland (25–45°, subtropical, mediterranean
or oceanic): the character starts in a loincloth, and the first spawn found before put them at
53°S on a cold spring morning, losing heat from the first minute with no clothes or fire to be
had yet. `player.json` (format 1) holds the body and the mover.

## D69 — Sound made as it plays, with no recordings
Every sound is synthesised in the mixer from tones, filtered noise and noise in grains, by
short recipes (`hearth_audio`). Recorded samples would need sourcing and licensing (the project
is clean-room) and many variants each to avoid repetition. Procedural recipes are varied on
every play, cost nothing to ship, and follow the simulation continuously: wind with its speed,
rain with its rate, the heart with the body. The cost is fidelity: a synthesised footstep is
a plausible footstep, not a recording of one. Recipes can be refined (or samples added through
content packs) without changing the engine's commands. Output goes through cpal on the device's
own thread, taking commands over a channel; offline rendering of the same mixer is what the
tests listen to (`tests/mixer.rs`, which also writes `bench-out/sounds/*.wav`). The heart and
the breath are the body's state made audible, for the HUD's minimalism (v2 §9.8): you hear a
pounding heart or a gasp in cold water before a panel says anything.

## D70 — The person: a box rig from anthropometry, moved procedurally; a chest band for a female body
Characters are boxes on a seventeen-joint skeleton whose proportions are anthropometric
fractions of stature, built from the appearance in code rather than modelled by hand. A
modelled mesh would need authoring for every height, build and hair style. The box rig keeps
the world's blocky look while holding to human proportions, and any appearance is built at
once. Movement is procedural, from the activity, speed and phase, with the gait's phases
(stance and swing shares, a flat stance foot, pre-swing knee bend, flight between running
strides) taken from gait studies. That way the bob, stride and timing come out right at any
speed without animation files, and footsteps heard and seen stay in step. The spec starts
every character in only a loincloth. A liberty: a female body also starts with a band of the
same hide or fibre across the chest (`chest_band`, worn and counted in the heat balance like
any garment, 0.02 clo). The figures carry no anatomical detail either way. It keeps the start
suitable for a general audience without changing what the body faces from the cold.

## D71 — Carrying by hands, garments and containers; loads by Pandolf; drags by friction
What a person carries is modelled as a person carries: a thing in each hand (12 kg at most in
one), one in both arms (up to half the body's mass, the commonly cited limit for carrying any
distance), worn garments by layer and region with the attachment points they give, containers
with grids and loads, a back load, and a drag. The loincloth's tie takes one small thing, so a
new person carries almost nothing, as v2 §10.2 asks. The load's cost comes from Pandolf's
equation (load terms only, the body already counts walking), its effect on speed from field
studies (little below a fifth of body mass, about 55 % of walking speed above half). A drag's
pace comes from the power a person sustains pulling (about 120 W) against the ground's sliding
friction, so terrain matters as it does: logs slide on snow and stick in mud. Moves are by
path and all or nothing, and the server keeps the truth (the client shows a move at once and
takes the server's word). The v2 §10.6 controls replace v1's (crouch C, prone Z, inventory
Tab, drop G, interact E, drag F, quick choice Q); lying down to sleep moved to X.

## D72 — Making and knowing: one process engine, discovery by a trigger vocabulary
Everything made is made by a process from the data, run by one pure engine
(`hearth_craft`): the server applies its outcomes and the client lists what is possible from
the same code, and later hominins and simulated humans will run the same processes. A process
names its target in the world (a block by id, material or suffix, a thing lying, water, a fire,
open ground) and what it does to it, so gathering, digging, cooking at a fire and lighting a
laid fire are all processes. Bulk materials are carried in units of a bulk form (a cut, a
handful, a lump, a hide) so they fit the inventory's grids, and processes count kilograms in
those units. Knowledge is gained only through triggers the game emits, named `verb:key` after
what was done to what (its form, material and their tags), plus `do:`, `use:`, `see:`,
`throw:`, `infer:` and a few systems' own. The content's routes listen for them, and the lint
proves every implemented node can be heard from a fresh start, with the world's real blocks.
Anyone can try the experiments that teach (knocking stones, hacking at a carcass, rolling
fibre, holding things in a fire); techniques then open the efficient versions, so learning
happens by doing, not by unlocking. Under Legacy death, knowledge passes on as legends that one
attempt brings back, skills do not. Three stand-ins hold until their systems exist, each
replaced later: a predator's kill turning up nearby every day or two (fauna, V2-7); lightning
setting single trees burning (wildfire, V2-6); nettle, hazel and bramble blocks placed in the
temperate woods (flora, V2-6). Seven Era 0–2 nodes stay planned until their systems arrive
(shelters and painting with building, fish weirs, rafts, wolves, fletching).


## D73 — The player's changes to the terrain are an overlay, saved by name
Terrain is generated afresh whenever a cube loads, so the blocks the player changes (stations,
holes dug, spoil heaps, fires) are kept apart, by cube, and laid back over each cube as it
loads, before the finite water, the seasonal cover and the light. They are saved with the
world as a list of positions and block states by name (`blocks.json`), so they outlive changes
to the registry's numbering, and states of blocks the content no longer has are kept as they
were and not shown. Storing whole edited cubes in the region files would bake the season's
snow and river levels into them; the overlay keeps generation, cover and change apart. When
building (V2-8) makes edits many, the list moves into the region files as a per-cube overlay,
and the LOD takes them in with V2-6.

## D74 — The performance gate alternates which build runs first
At the end of V2-5 the gate failed twice on one measure, the cave scene's 1 % lows (−5.8 % and
−5.9 %; its average FPS −2.7 % and −4.0 %), with the same geometry and draws in both builds
and no difference in the GPU passes beyond noise. Two A/B runs cleared V2-5. Its own build with
and without its new textures (56 more layers in the texture array) ran the cave alike (2185
against 2185 FPS, lows −0.2 %). The baseline against V2-5 with V2-5 run first in every pair
put V2-5 ahead (+0.6 % average, +1.3 % lows). The gate had always run the baseline first, so
any drift within a pair (the GPU warming, clocks settling) fell on one side. It now alternates
which build runs first, round by round.

## D75 — Trees are grown from species templates; limbs are joined bars; foliage is passable
Each temperate species (25, Appendix A's tier 1) has a growth form in the data, and a pure
growth model (`hearth_flora`) turns species, stage and variant into a tree: Chapman–Richards
height and a diameter that goes on thickening after the height levels off, eight stages from
seedling to ancient and dead standing, and a parametric skeleton after Weber and Penn (stems
that lean and taper, decurrent crowns forking into limbs, branches at the species' angle in
whorls or a spiral as long as the crown's shape allows, twigs, droop, foliage clusters). Wood
a metre or more through fills log blocks; thinner wood becomes branch blocks, bars 2, 4, 8 or
12 px thick joined to their neighbours (256 states per wood), drawn as bars and arms with no
hidden faces. The tree is grown once per (species, stage, variant) into a template and cached;
the world places it turned and mirrored as its position's hash says, so eight variants per
stage look like many trees. A tree is the template of its stage, not a continuous growth: its
blocks change only between stages. Species are drawn by how well the climate fits their
envelope (temperature, coldest and warmest months, rain, Köppen class, wet ground), the
biome's affinity and, for the understory, shade tolerance; stand age varies in patches across
the land. Crowns are spaced by their size (a tree stands in its 5 m cell as often as the cell
is a share of its crown), with shade-tolerant saplings and poles between. Where no species
fits (the tropics, deserts, tundra) the old shapes stand until their species come in V2-10.
The distant terrain reads each template's per-column crown and trunk summary instead of every
block, and beyond the levels that grow trees it asks the generator for the likely species and
the stand's age. Foliage is passable: it slows a body by a quarter to over a half by its
density and rustles; thick limbs (8 px and up) are solid and stood on, thin ones passed
through. Felling is a process on any standing tree's trunk: the generator says which tree a
block belongs to, the tree is taken away but its stump and falls about it away from the cutter,
coming to rest on the ground after a few seconds (the client draws it turning as boxes), and
the edits overlay keeps it there.

## D76 — Vegetation is a state the generator grows from: the year and the disturbances
The terrain is generated afresh whenever a cube loads (D73), so trees that age, land that is
cleared or burned and grows back, and felled trees that are replaced must all come out of the
generator. Its input is a small vegetation state (`hearth_worldgen::vegetation`): the year the
trees have grown to (calendar years since the world began) and every disturbance so far (a
tree felled at its foot, ground cleared, land burned: when, where, how far, how much), found by
place in 16 m buckets. A tree site is a sequence of generations drawn from the site's hash: the
tree the world began with (at the stand's age, as before), dying at its lifespan and standing
some years as a snag, then a tree that took the gap (the shade-tolerant, the quicker of them
more often); a disturbance ends a generation early (felled: the stump stands until the next
takes root; burned: the trunk stands charred for years; cleared: gone), and opened ground
grows back with the light-demanding first, weighted by how their seed travels and how fast they
grow (the wind-sown pioneers within a few years, others later), the shade-tolerant coming up
under them after a third of their lives and taking the site when they die. A young stand on
opened ground thins by the crowns of the place's usual species at the stand's age, not by each
tree's own, so the fast-growing pioneers are not thinned first. The ground of a disturbed
place has its phases (burned bare and black for a season and a half; herbs at once; shrubs
from the second year; broken ground for six; light under the young trees closing over forty
years, a felled tree's gap lighter for twelve). Generating a cube gives the year it next
changes (a tree's next stage, a generation's birth or death, a remains falling, a ground
phase). The server keeps each loaded cube's snapshot and that year; when the year comes, or a
new disturbance reaches the cube, it generates the cube with the old and the new state and
lays in what changed where the player has not changed the block and no water stands (under
the seasonal cover, taken off and laid again), then relights and remeshes. A felled tree is
the vegetation's change, not the player's: its blocks are taken without entries in the edits
overlay, so the tree that takes its place is not cut by them. The state is saved
(`vegetation.json`). The world as it began is unchanged: the generator at year 0 with nothing
disturbed is the old generator, and the screenshots and benchmarks use it.

## D77 — Wildfire: a block automaton near the player, cells far away, burned ground kept
Fire in the vegetation runs at two scales. Near the player, where the terrain is loaded, it is
a stochastic automaton over blocks: each block holds a fuel (turf, herbs, foliage, twigs,
limbs, trunks) with a catching chance and a burning time, and each step every burning block
may set its 26 neighbours alight by their fuel, its heat, the fire danger, the wind and the
slope; what burns out becomes what it burns to (bare ground, nothing, charred wood). The
danger is the moisture of fine dead fuel by Simard's equilibrium (humidity and temperature)
with the last two days' rain, times the share of the growth cured, from the climate's month
(Gaussen's dry month); the curing is what makes the dry season the fire season while a damp
temperate summer hardly burns. Rates were set so that a fire's chance of spreading (the
expected blocks each burning block lights) is well over one in a dry month's grass with
wind, about one at moderate danger and well under one in damp air; a fire backs into the wind
at a tenth of its pace or less. What the fire burns is nature's change, not the player's: the
ground it has left is kept in the vegetation state as Burned disturbances with an exact shape
of 4 m squares (taken when no block in a square still burns), so the generator draws it burned
and grows it back, and an unloaded and reloaded area is burned as it was. Beyond the loaded
terrain fire spreads over the 256 m ecological cells by their fuel and the danger there, each
cell kept as a Burned circle; the two hand fires over at the loaded edge. Smoke is drawn as
GPU-generated puff plumes (one per 32 m square of near fire, one per burning cell far away),
depth-tested, lit by sky and sun and faded into the haze, so a far fire's column is seen from
kilometres. Burning blocks are not saved: a fire burning at a save is out on opening, with
what it burned kept.

## D78 — Distant terrain: vegetation and changes in the tiles, a disk cache, a budget, TAA
The distant terrain is built on the client from the generator, so what changes the land must
reach it there: the server sends the vegetation snapshot (cheap: it shares the disturbances)
when a disturbance is added and as the years turn, and the player's changes as one top per
changed column (its highest solid block). The tile builder reads both (burned ground drawn
black for its bare season, the trees as the vegetation has grown them, changes standing over
the ground) and the streamer builds again the tiles a new disturbance or change reaches,
drawing the old ones until then; trees in tiles grow by whole years so a year's turn rebuilds
every tile once, in the background. Built tiles are kept on disk (zstd, one file a tile) with a
stamp: a fingerprint of the build (two probe tiles near the spawn built and hashed, because
there is no build number and a change to generation, meshing or colours must invalidate the
cache) and the vegetation and changes reaching the tile; a tile is read back while its stamp
holds and rebuilt otherwise. The video-memory budget the options already had now holds: the
streamer scales the distance rule of the selection down when the tiles outgrow it (the far
tiles coarsen first) and back when they take under 60 % of it, looking once a second.
Temporal anti-aliasing jitters the projection itself (every pass sees it), resolves after the
translucent pass by reprojecting the history through the depth buffer in homogeneous
coordinates (the sky at infinite depth reprojects by direction alone), clamps it to the
current 3×3 neighbourhood's mean ± 1.25 deviations in YCoCg, and blends a tenth of the new
frame (more when the view moved); the result is copied back into the HDR target so metering,
tonemapping and scaling are untouched. It is opt-in (the Fabulous preset) because it softens
textures and costs a full-screen pass. FXAA, listed in the options since v1, is not built and
counts as off. The fly-through benchmark streams near cubes (pre-generated along the path,
uploaded as they come within reach, at most 256 a frame as the client) and distant tiles
through the game's own streamer, so what it measures is the game's streaming on the frame
thread; its slow frames are the tile selection, which is the first thing to move off the
frame thread.

## D79 — V2-6's frame cost is its forests
At the end of V2-6 the gate against the end of V2-5 (946304d against d4931e6, three alternating
rounds) failed: lowland_forest −8.6 % average (731 → 668 FPS) and −13.6 % 1 % lows (693 → 599),
peak_lod512 −5.0 % and −9.9 %, cave_torches −1.4 % and −6.8 %. The forest's triangles grew from
1.27 to 1.75 million: trees are now grown from their species with limbs as joined bars and
crowns of foliage, an understory of 26 plants stands under them, and the distant tiles carry
the vegetation. Its near terrain pass went from 0.48 to 0.60 ms and its distant terrain pass
from 0.34 to 0.39 ms; the frames wait on the GPU (0.9 ms of 1.5), so that is the loss, and the
1 % lows fall further because they are the densest views of the forest. The cave's GPU time
did not change (0.433 → 0.437 ms, a 0.47 ms frame); its lows are one run's 43 ms stall in
submit. Three measured optimizations were made first: foliage behind two layers of leaves
(+4.1 % average, +5.0 % lows in the forest), a limb's faces toward foliage (−2.4 % triangles)
and a limb's end inside another (−2.5 % triangles, +1.4 %), all with an SSIM of 0.98 or more.
What is left is the content, and the forest still runs at 668 FPS average and 599 FPS 1 % low
at 1920×1080 on the RTX 4060 Laptop. Cutting further would change what is seen (fewer small
plants with distance, opaque leaves far off), which belongs to a lower quality preset, not the
default; it is noted for the next rendering pass. The forest's video memory (245 → 357 MiB)
is the quad arenas doubling past a power of two, room allocated rather than used. The
baseline moves to the end of V2-6.

## D80 — Realms by continent size and latitude; stand-ins from one realm
The spec asks for landmasses grouped into faunal realms by isolation and climate. On the planet
grid the landmasses are found by flood fill (8-connected, wrapping); those of at least 4 % of
the land are continents. Ranking the continents by area and giving the largest the Old World's
realms (Palearctic north of 23.5°, Afrotropical in the tropics) and the next the New World's
(Nearctic, Neotropical) mirrors Earth, where Afro-Eurasia is the largest landmass, and puts the
richest content (the Palearctic tier 1) where a player most often starts: ranking the northern
zones alone gave seed 7's spawn continent, the larger by 21 km², the Nearctic for being 1 km²
smaller north of the tropics. Southern temperate land takes its continent's tropical realm (as
the Cape is Afrotropical and Patagonia Neotropical); islands near a continent share its realm,
remote ones are Oceanian. Most realms have no species of their own yet for most climates, and
the spec allows "a clearly labeled composite where a random planet needs an ecological role
filled": where the realm has none, the species of one stand-in realm fill the place (the
Palearctic outside the tropics, the Afrotropical in them), weighted down fiftyfold so that a
native that suits at all outweighs them, and never two realms mixed. The Nearctic has three
trees of its own so far, so a fifth or more of its trees are Palearctic stand-ins where none of
the three suits; more Nearctic species come with the expansion waves (V2-10). The species chooser
reads the realm from the column sample, so the near terrain, the distant tiles and the
vegetation's regrowth all agree.

## D81 — Populations: groups and numbers, food anchored to the community, regulation at capacity
The spec asks for ecological cells holding densities with age and sex structure, advanced in
abstract steps, stable over long runs by realistic mechanisms rather than clamps. Large animals
are groups with members (so a herd seen twice is the same herd, hunting takes individuals, and a
pack holds a territory); small ones, too many for groups, are numbers per cell. Explicit stocks
of forage that grow, decay and are eaten were tried first and abandoned: with some forty species
eating from the same stocks the outcome hung on every decay rate and access limit, and species
crashed or boomed for reasons of bookkeeping. Each cell instead offers each kind of forage at the
rate the vegetation and season give, shared out by scramble competition, and the absolute amounts
are anchored so that the reference wood feeds its community at the species' usual densities —
the habitat formulas say how places differ, the animals' needs say how much. Regulation is by
mechanisms the spec names: food (lean seasons, poor summers, hard winters, mast failures), prey
refuges (cover, and the type III response of generalist predators; a specialist, which cannot
switch, searches harder), territories (breeding only for holders; floaters die sooner; predators
keep apart), alternative prey and foods (omnivores make up missing meat with plants, hunters only
partly), and density dependence (disease, stress and want of room past what the land about a
group holds, over its home range; fewer young growing up where the place is full). The species'
`adult_survival` and `young_survival` are survival from what is not simulated, as the schema
says; the first data gave total survival in the wild for some small prey, which counted
predation and hunger twice and drove them extinct, and was corrected. Condition moves toward what
the food allows rather than adding up every shortfall, because an animal fed nine tenths of its
need is thin, not dying. Small populations of wide-ranging animals (wolves, lynx, bears) are kept
from dying out for good by animals coming in over the edges of the simulated land, as from land
beyond at its usual numbers. Each cell knows the realm of its animals (a 16 km region of a
standard planet can span the tropics and two continents). A uniform wood of four regions holds
all of its species between a seventh and two and a half times what the habitat holds for thirty
years, and heavy hunting of its red deer takes them down and they come back
(`crates/hearth_fauna/tests/populations.rs`).

## D82 — Animals come into the world from their groups and fold back into them
The spec asks for animals to be simulated as populations far from the player and as individuals
near, with the two agreeing. The groups of D81 make that exact for large animals: a group near
the player becomes its members (by stage and sex), and folding them back puts the living into
the group's numbers and leaves out the dead, so a herd met twice is the same herd less what was
killed, and the hunting the populations feel is the player's. Small species, kept as numbers per
cell, are drawn from the cells about the player the same way each day (the share of each cell
within reach, three at most to a cell, as a sample rather than every vole) and subtracted from
the cell while out. Materializing at 112 m and folding at 150 m gives the gap that keeps a group
at the edge from flickering in and out. Where the blocks are not loaded the generated heights
(their rounded tops, as the blocks have them) serve as ground, so that a group at the edge of the
loaded land is not held back or placed in the air. The server owns the animals and sends their
state ten times a second, as for every other moving thing; the client only eases and draws.
Regions are made one at a time, one per two seconds, on the server thread (about a tenth of a
second each in a release build: three years of spin-up at monthly steps), and the populations
advance with the calendar every few days of game time in steps of at most a thirty-second of a
year (a few milliseconds a region); making regions on a worker is left to the performance work
of V2-7 (j). The regions are saved compressed beside the world's other state.

## D83 — Bodies built from data and moved by gait data; coats painted into one atlas
The spec asks for body plans with shared skeletons at real dimensions, procedural animation from
gait data (walk, trot, canter, gallop, foot IK on uneven ground, head look-at, breathing, tail
and ear motion) with a few authored clips per plan, and coats from a recipe made by
`hearth_texgen`, so that a new species needs only data and a texture recipe. Each species'
skeleton is built from its body plan and a `shape` of proportions, every one defaulting by plan
(neck, head, snout, tail, ears, legs, hump, antlers, horns or tusks), rather than authored per
species; a test holds every species' standing height and length to its data. Movement is
computed rather than keyframed: the gait follows the Froude number against the hip height, the
stride's length Alexander's relation, each foot's timing the gait's phase offsets and duty
factor, and every leg is bent by two-bone IK to the ground under its foot, so that the same
code walks a vole and an aurochs over any ground. The spec's clips (eat, drink, lie down,
sleep, groom, rear, attack) are poses blended in by weights eased over time on the same
skeleton and IK, rather than authored keyframes per plan, which would not follow the ground or
fit forty bodies. Coats are painted once per species and variant (female, winter, male, young)
into one atlas, each box unwrapped six faces to a cross and every pixel painted from where it
lies on the body at rest so that patterns run across boxes; the figure renderer reads a box's
faces from it by two words in its instance, so players and animals stay one instanced draw.
The texel density is the world's sixteen a metre for large animals and finer for small ones
(up to 160), because at sixteen a metre a vole or a robin would be a single colour; a coat is
still a handful of pixels across, which keeps them in the blocky style.

## D84 — Ways searched on block columns, bounded; flights and climbs as their own media
The spec asks for pathfinding on a navigation grid extended for swimming, flying and climbing.
There was no navigation grid (the planned `hearth_entity` crate was never built), so the ways
are searched directly on the loaded blocks through the `Ground` the animals already stand on:
a column at a time, its footing found near the level searched from, with the step rules of the
body (the climb and drop of its legs, the water it wades, whether it swims). A cached grid would
need rebuilding wherever blocks change; searching the blocks needs nothing kept, and its cost is
bounded per search and per step (six searches a step, each at most 1500 columns), measured at
about two tenths of a millisecond a step with a herd of twenty-nine fleeing (release build).
Ways are searched only when an animal takes a new goal and again after a step fails, and drawn
straight where the ground allows, so that animals do not zig-zag along grid cells. Flight is
not searched: birds fly over the trees (a flight's height is set by the tallest crowns sampled
along it, its climb by those near its ends), which is how birds go and costs a few dozen block
lookups. Climbing is a medium of its own (to a trunk by a way, then up it), as is perching in a
tree. Fish are searched as walkers that may only step into water deep enough. Trees' trunks,
limbs and foliage are told apart by their blocks' names (`_log`, `_branch`, `_leaves`), as the
renderer already does for the crowns.

## D85 — Suspicion from senses as rates; habits as utility weights
The spec asks for senses (sight with field of view, acuity and night vision; hearing of the
player's noise; smell carried by the wind) to be real stealth mechanics, and for a utility AI
with weights in data. Each sense gives a rate at which suspicion grows rather than a yes or no,
so that a person at the edge of a sense's reach is noticed only if they linger, close by at
once, and suspicion fades when nothing is sensed; two thresholds (watching, aware) turn it into
behaviour, and awareness rather than distance alone sets an animal running, so an unnoticed
stalker gets close. Hearing falls with the square of the noise so that quiet going is heard
much nearer than loud (as sound energy falls with distance squared), and scent is a cone
downwind whose length grows with the wind, as the spec's "approaching from downwind matters"
asks. The player's noise is computed from their gait and the ground underfoot the same way
their own footsteps are sounded. The choice of what to do at ease is a utility over a few
behaviours scored from the animal's state (thirst, its mother's distance, its distance from
its group's middle, its herd's size) and weighted by the species' habits, which default by diet
and social life so that only species unlike their kind need data; the alarm that sends a herd
running with its first runner is how herds share vigilance. Predators, hunting and the calls
the alarms will sound with come in (g)–(i). Clothing colour does not yet change how plain a
person stands (nothing records clothing colour beyond its look).

## D86 — Attacks only for causes, weighed once an encounter; acceleration bounded
The spec asks that predators avoid people most of the time and attack for real reasons (hunger
in lean seasons, surprise, defense of young, food or kills, cornering, the person seeming small,
territory), that dangerous herbivores charge when threatened, that snakes bite when stepped
near, that counterplay be real (fire, standing tall and facing them with noise, not running from
a cat), and V2-7's acceptance that every attack log a realistic cause. An attack therefore
cannot begin without a cause holding, each cause with its own conditions and odds from the
species' aggression and the world's setting, rolled once an encounter (then not for fifteen
seconds) so that lingering near an animal is not a slot machine; the cause travels with the
charge and is told to the player and logged. Bluff charges are the common outcome for defenders
(as bears' are), and a defender that has struck goes; only hunger presses on. Territorial defense
and habituation to people's food are not modelled yet (no dens, no stores of food that draw
animals); wounds from hunting and the provocation they bring come with (h). Speeds changed from
an exponential ease (which got a deer to two thirds of its top speed in a quarter of a second) to
bounded acceleration (six metres a second every second, more for hunters and small bodies), as
an ambush depends on it: with the ease every prey that noticed a rush was gone before the hunter
reached it.


## D87 — Carcasses and their butchering generated per species; remains kept by the populations
V2-7 replaces the one generic carcass and its two processes with the animals' own. Listing a
carcass, a butchering and a hacking for each species, sex and age by hand would be some hundred
and eighty entries restating the species' data, so they are generated at load from the yields,
masses and body plans (the knowledge that butchery needs lists them as it enables them), and a
new species brings its carcasses with it. Sex and age are separate carcasses only where they
matter (a size dimorphism, or what only males carry); the mass range was read as both sexes about
its middle with a data `dimorphism`, since reading the range's ends as the sexes made a male
trout one and three quarter times a female. Two small engine rules were needed: an output may be
seasonal (fat, antlers), and a carcass gives what is left of it (its `condition`) with its decay
passed on. Found kills come from the deaths the populations already have, kept as remains with a
place and a time where the abstract step kills or loses a large animal, drawn from a stream of
their own so the populations' draws stay as they were; they are few within reach (a person finds
one now and then, and the ravens lead to others a couple of kilometres off), so a person's hides
and meat come mostly from hunting, as they did. The V2-5 acceptance, which needed a kill every
day or two at camp, now has a test-forced natural death near camp instead (like its forced
lightning), the bot not hunting.

## D88 — Wounds by part and depth, bleeding to death; signs near the player only
V2-7 asks for hunting with wounds that behave as they do: a well-placed spear brings a deer down
within a minute and a short run, a poor one wounds and the animal goes on, and the weapon
matters. Rather than hit points, a blow is resolved by where it lands on the body (the rig's own
torso, neck, head and legs) and how deep the point goes against what covers the vitals, from the
weapon's energy and sharpness; what follows is bleeding, as a share of the blood a second, to
death at two fifths lost, with flesh wounds clotting, so that the same spear kills a roe deer
outright and only wounds an aurochs, and a gut-struck deer is found lying up a long way off — as
hunters know. Aiming strays with the thrower's practice (a skill, not a stat). Signs (prints,
blood, droppings) are produced only within 80 m of the player and kept for days by count, since
a world of tracks everywhere would cost much and be seen nowhere; prints only where the ground
takes them, so a summer wood shows few and snow shows all. Reading them takes the tracking
knowledge, learnt by noticing them.

## D89 — Calls from the species' data, at the animals' own pace; the chorus from the numbers
Calls are made from each species' data rather than recordings or hand-made sounds per species,
as every other sound is (D69): a recipe per kind of call, pitched and timed by the species. The
occasions come from the data too; what was decided is the pace: call rates are per real hour, as
the animal lives second by second, not per hour of the compressed calendar, since a robin that
sang thirty times faster because the day passes in forty-eight minutes was absurd to hear. The
dawn and the rut are still windows of the calendar's day, so the chorus lasts a few minutes of
play and is intense while it lasts. Songbirds too small to be drawn are heard from the
populations' own numbers per cell, so the chorus is as rich as the wood, and the far calls of
packs and stags come from their groups' places, so a howl tells truly where the wolves are.

## D90 — Regions made on workers; the young bear density; acceptance on the generated land
Making a region (its habitats from the generator and three years' spin-up) took a tenth of a
second or more on the server thread, a hitch each time the player came near new land. Regions
are now made on worker threads, three at a time, each from a copy of the populations' tables
with group ids of its own a million apart (none can meet those born meanwhile), and taken in
when made; the screenshot tool still makes them at once. The acceptance's fifty years on the
generated land found two flaws the uniform wood had hidden — the young of slow-growing small
species dying at the first year's rate through every year of growing, and crowding killing a
territory's holders with its young — mended as density acts in the wild (on juvenile survival
first), with the long-standing tests holding; its bounds are a twentieth to four times the
capacity (the uniform wood's are a tenth), since a patchy land on the borders of realms and kinds
of land holds its animals in pieces, and species of fewer than ten in the nine regions are left
out (a handful of bobcats or bears is at the mercy of chance).

## D91 — A construction piece is one block of its material, generated; quantities real
Building pieces are blocks generated from the pieces' data and the materials each may be made of
(`hearth:post/hazel_wood`), shaped as the piece sits in its block and textured from the
material, as natural blocks and carcasses are generated rather than listed (D87). A piece is one
block: a post or a beam a metre long, a roof the slope over a metre square; a structure is a
pattern of blocks, which the frame's reckoning (V2-8 (b)) can treat block by block. What a piece
takes is what it would: 56 bark strips for a metre of pitched roof laid twice over, 24 sticks for
a metre of brush, 36 fieldstones for a block of dry-stone wall; one simplification is a pole,
2.2 m, making one post or beam a block long (its offcut is not kept). Posts and beams look two
sixteenths thick at least, though a sapling pole is under one, to read as members at all; their
mass and strength keep the true size. Pieces that stop light (roofs, layers, walls) keep none in
them, so their faces take their light from the air about them, not from inside, where it is
dark.

## D92 — A piece goes beside the face looked at; it must rest on something; a ghost shows it
Where a piece goes is the place across the face of the block looked at (the client finds the face
from the side of the hit box the ray came in by and sends `AimAt::Beside`), or the place of
grass or snow that gives way to it — the ordinary way of building in blocks, which lets a roof be
put up against the beam above an empty space. It faces the way the builder faces. Stages are
kept by a rule of resting rather than by a fixed order: a post on what stands, a beam from a post
or a wall beside it (never balanced on a post's top, since a beam lies across the top of its own
block), a roof or a layer against anything, a wall on the ground, a wall or a lintel; whether it
is strong enough is the solver's (b), so the rule only forbids what could not stay a moment. The
ghost is the edges of the piece's boxes drawn as thin bars in the figure pass (no new pipeline),
pale where it would rest and red where it would not, kept where the work is while it goes on.

## D93 — Stability by statics of runs and columns, not support decay; reckoned whole, on the tick
Valheim's support (a value lost by a fixed share per block, sideways more than down) was weighed
and set aside: it knows nothing of what a piece carries, so a beam under a stone wall and one
under bark would reach as far. Instead pieces are members with weights and strengths from their
materials (D91's real sizes), and the reckoning is the statics a builder would do by rule of
thumb: columns carry loads down and are crushed or buckle; runs (beams, lintels, roofs along
their slope, layers, hung walls) span between what holds them, each span simply held, beyond the
outermost support a cantilever; the moment is checked at every piece and joint, and a reach of
fifty depths stands for stiffness (sag), which governs thin poles before strength does. Joints
carry a share of the moment by their kind — stacked stones none, so a run of slabs cannot span
however strong each slab — which is what makes a dry-stone hut need its lintels and corbels. The
margin is a safety factor of two on every strength. Loads are gathered top down in one pass (at
each level coverings, then runs in an order where a run comes before what it rests on, then
columns); statically indeterminate frames are cut at their supports, which errs safe for spans.
A change has its whole connected structure (up to 8192 pieces) reckoned again, deterministically,
so order of placing never matters; it runs on the server's tick under a budget of 8192 pieces a
tick (a few milliseconds) rather than on a worker, since a hut costs a fraction of a millisecond
and a result a tick late would let a player stand under what has already failed. What fails
breaks at once, half its makings lying where it fell, and what it held is reckoned on the next
tick, so a collapse runs on visibly.

## D94 — Ground over an opening: a width it roofs, from the material; pieces under it take its weight
Natural ground is not given members: a hillside is millions of blocks, and nobody reckons a
tunnel's roof as a beam. What matters to whoever digs is how wide an opening the ground stands
over, which depends on the ground far more than on the load: loose earth runs in at once, clay
and firm soil hold a crawlway, rock holds halls by its strength (a rule after rock-mass
engineering's stand-up spans, here one metre and a fifteenth of the crushing strength in MPa),
with a material free to say otherwise (loess's cave dwellings). Only the ground over the places
a change opened is reckoned, along the four ways through them, so ordinary digging costs a few
hundred lookups and natural overhangs left alone stay as they are. Falling ground breaks loose
and lands on the floor below, so a collapse migrates upward as real ones do (chimneying to a
sinkhole in soil). A piece under ground takes the ground's weight as deep as the opening is
wide (at most eight blocks, the rest arching over, after Terzaghi's loosened zone) — which is
what makes shoring a matter of stout timbers or stone, not of putting anything at all in the
way. The surface's own blocks, which carry no material, are reckoned by their sound as turf or
bare earth rather than given materials that would change what digging them teaches.

## D95 — Decay as stages in the block; shelter from rays, warmth from the fire over the losses
Pieces that rot, wash away or melt carry a `decay` stage (0–3) in their block state rather than
an age kept elsewhere: it saves with the block, shows in the reckoning at once (each stage a
weaker member), and costs four states a piece. A day's weather moves each stage on by chance at
the rate the material sets, three stages over its life, with the chances drawn from the place
and day so that a world weathers the same way each time; rot depends on contact with earth
(the classic failure of posts set in the ground), not on wetness kept per block. Shelter is
reckoned from seventeen rays (level and upward; the ground is under everyone) rather than by
flooding the air of a room: rays cost a few hundred lookups a tick, degrade gracefully for
lean-tos and windbreaks that enclose nothing, and see through brush by its fill. The warmth a
fire gives a hut's air is its heat over the hut's losses — openings at 200 W/m²·K, walls by
their insulation over a small hut's inside — capped at 25 °C; radiant heat from the fire to the
body is kept separate, as it was.

## D96 — The slice reviewed by a scripted year, its moments kept as saves for screenshots
The vertical slice is judged by playing it, and a person cannot play a year in a session; so a
bot plays it (the V2-5 acceptance's, shared), lenient where the acceptance is strict (a goal
missed is logged, not failed), logging each day and copying the world at its moments. The
copies are ordinary saves, and the screenshot tool lays a save over its world (`save=`): the
changes, the vegetation, the things lying about, the date and hour of its clock — so what the
review shows is the world as its player left it, rendered by the same code as the game. The
bot is a review tool, not a test of the world: what it does badly is reported as what a careless
player would do, and fixed in the bot.

## D97 — The time since a death is reckoned in the calendar's days for going off
Populations live on the year scale, and their dead carry the time of death in years. A carcass
taken into the world was aged by a real year's 365 days, while going off runs on the day scale;
in a world of eight-day seasons, a deer dead an hour was reckoned dead half a day and its meat
was going off as it was cut. The time since a death is now reckoned in the calendar's days (its
days per year), so meat from a fresh kill is fresh. How fast scavengers take the remains stays
on the year scale (it is the populations' own business).

## D98 — Things at rest lie on their broadest face
Forms give their sizes in whatever order their authors thought of them (a hide 0.5 × 0.5 ×
0.004, a brick length × height × width), and things lying about were drawn with the second
size upright, so hides and flakes stood on edge. Rather than reorder every form, a thing at rest
is drawn on its broadest face with its longest side along (its sizes sorted: longest along,
least up) — which is how a loose thing settles; things in the hand were already drawn by their
sorted sizes.

## D99 — A piece may have a look of its own: brush is twigs with gaps, wattle is woven
Pieces wore their material's texture, so a brush wall of maple looked like a maple plank and
wattle like boards. A construction piece may now say the pattern its faces show (`look:`); such
a piece gets its own texture per material (`piece/<piece>/<material>`), drawn as a cutout. Brush
is a tangle of twigs and sticks with bark on — greyer and darker than the wood within — with the
gaps between left clear, so the light shows through a windbreak and a bough roof as it does;
wattle is rods woven over and under stakes.

## D100 — Food is eaten where it is carried
Eating was offered only for what was in the right hand, and what is picked up goes into the
pouch first; so food picked up had to be moved to the hand to be eaten — the year bot starved
at camp with meat in its pouch. In the inventory, E eats one of the food under the pointer
wherever it is carried (the server always took any carried place); the tooltip says so.

## D101 — Members on their blocks' middle lines, carried on to meet what they join
Pieces stood where their own block put them: a post in the middle, a panel or a wall at the
side it faced, a beam along the top, a roof in its own block — so a beam stopped half a block
short of the post it rested beside, a panel never met the post it was lashed to, and a roof's
edge hung half a block off its ridge beam. Pieces now lie on their blocks' middle lines (a panel
or a wall upright across the middle, at its own thickness), and where a member reaches the face
it shares with a neighbouring piece, the renderer carries it on into that block until it meets
the piece there: a beam into the post's side, a panel to the post, a wall to the wall at a
corner, a post up under the beam over it, a wall up under the roof, a roof's edge onto its
ridge beam or over a gable. Straight on along its own length a member may go the whole block;
anything else (a roof's edge, a beam's top, a floor) at most to the middle; and never where
nothing in the block lies across its way, so nothing grows across a block it only touches. The
joints are drawn, not built: what bears and what stands is still reckoned block by block (D93),
which is what the player places and takes down. Joining by drawing, not by block states, keeps
the state count flat (connections in states would be dozens per piece) and needs nothing of a
player but to put pieces next to each other.

## D102 — Roofs drawn as smooth slabs over their steps
A roof's shape is eight steps (the frame's reckoning and walking on it want boxes), and drawn as
steps it looked like a staircase. It is drawn instead as the smooth slab the steps stand for: the
band over its column between the line under the steps and a parallel line its thickness above,
two sixteenths thicker than the steps so that the steps and the ends of what meets them lie
inside it. A thick slab passes a little above its block at the high side and below it at the
low, so successive pieces up a slope carry one continuous roof without a notch at each row, and
two slopes meeting at a block's edge make a ridge. A roof carried on along its length (over a
gable, to a post) is carried on as the same slab.

## D103 — Fat carries shivering when the glycogen is spent
A body whose glycogen was spent shivered at half strength, so a hungry person in a hide cape
lying by a dead fire on a dry 7 °C night fell from 35 to 29 °C in under four hours — how the
year bot died, twice. People short of carbohydrate shiver as warm for hours, the fat taking
over the fuel (Haman et al. 2004: sustained shivering with muscle glycogen low makes as much
heat); what fails shivering is a blood sugar run out and a body wasted. Spent glycogen now takes
only the hardest shivering (its ceiling to three quarters), and a body wasted toward its last fat
(from 8 % of its mass down to 3 %) loses it, to a fifth. Hunger still costs warmth through the
weakness it brings and the work it stops; it no longer freezes a person by itself.

## D104 — A joint roasted on a spit
Meat was roasted half a kilogram at a time, half an hour each, so a roe deer's thirty-odd cuts
took a night at the fire — the year bot stood over a dying fire till its core was at 28 °C,
and a player would click through it seventeen times. A joint (two kilograms, a haunch or a side)
turned on a green stick over the coals for an hour (`roast_joint`, known with roasting) is how
people roast a kill; cuts are still roasted or charred singly.

## D105 — A tree's crown drips most of a rain through
Rain cover counted any block that stops light as a roof, leaves included, so a camp under a tree
was never rained on — the year bot's camp under birches stayed dry for a whole year, and a
shelter there kept off the wind alone. A crown now shades and lets seven tenths of a rain drip
through (leaves hold back a fifth to a third of a steady rain: interception), once however many
leaves deep; a roof under it keeps the rest off.

## D106 — What rots away is gone
Food left lying went rotten and stayed for ever: the year bot's camp gathered ninety cuts of
cooked meat long past eating, and a player's camp would fill with them. Anything that perishes,
lying rotten (its decay at one and a half), is gone as carcasses already were — to the flies and
the beetles. What is carried stays to be thrown away.

## D107 — The end of V2-9's gate: the 1 % lows that fell were single stalls, not the build
The gate at the end of V2-9 twice found the forest scene's 1 % lows down (−19.5 % and −13.5 %,
and once the peak's by 9 %) with every average within 4 %. Each fall came from one frame of
fifteen to forty milliseconds spent in submitting (the driver's, not the scene's: the frames
about it took their usual GPU time, and the scene drew the same triangles and draws in both
builds), in one run of three; nothing in V2-9 changes what these scenes draw. Alternating the
two builds on the forest alone for four rounds of 1500 frames gave the new build 667 FPS on
average against 652 and 1 % lows of 578–590 against 476–589 — and the baseline its own
38-millisecond stall. The baseline was moved to the end of V2-9. A 1 % low over 600 frames is
six frames: one stall in three runs moves it by a fifth, so the gate's 1 % lows are to be read
with their runs' worst frames, as here.
