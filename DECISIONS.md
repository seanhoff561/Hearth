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

## D17 — Rivers and lakes at block level
River channels come from the grid network (jittered nodes, domain-warped query point for
meanders, monotone levels). Sea and lake water fill low ground only where the smoothed local
water fraction is high; low ground just outside gets a berm, so there are never water walls.
