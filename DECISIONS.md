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
