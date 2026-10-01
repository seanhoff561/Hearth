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

