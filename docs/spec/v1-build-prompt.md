# BUILD PROMPT — High-Performance Voxel Survival Game in Rust (Minecraft-style, clean-room)

You are an autonomous senior engine programmer working alone in this repository. Your job is to build, from an empty directory, a complete, polished, highly optimized voxel survival-sandbox game in Rust whose feel, controls, settings and visual style closely follow the current Minecraft: Java Edition, with a deliberately trimmed content set, a Distant Horizons–style level-of-detail system for near-infinite view distance, and a lightweight but beautiful atmospheric renderer.

Read this entire document before writing any code. It is the specification. Where it is silent, make the decision a veteran engine developer would make, record it in `DECISIONS.md`, and keep going.

---

## 0. Ground rules (non-negotiable)

1. **Clean-room implementation.** Do not bundle any Mojang assets (textures, sounds, fonts, models, language files). Never use the words "Minecraft" or "Mojang" in the game's UI, window title, crate names or identifiers.
2. **Project codename:** `hearth` (crate prefix `hearth_`, binary `hearth`). The user may rename it later; keep the name in one constant.
3. **Work autonomously to completion.** Do not stop to ask questions. Do not end your turn after a plan or after a single milestone. Continue milestone after milestone until every acceptance criterion in §14 passes, or until you truly cannot continue (then follow the Resume Protocol in §15).
4. **No stubs in shipped code.** No `todo!()`, `unimplemented!()`, empty match arms that swallow features, or "placeholder" systems left behind. If something must be deferred, it goes in `PROGRESS.md` under "Deferred" with a reason — never silently.
5. **Always green.** After every meaningful change: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Never leave the tree in a non-compiling state between milestones. Commit (git) at the end of each milestone with a descriptive message.
6. **Stable Rust, current crates.** Use the latest stable toolchain. Before adding a dependency, check its current version (`cargo search` / `cargo add`) instead of trusting memory. Prefer well-maintained crates; avoid abandoned ones.
7. **Performance is a feature.** Every hot path is designed for cache locality, zero per-frame heap allocation, and parallelism from the start — not "optimized later".

---

## 1. Technology stack

- **Language:** Rust (edition 2024), cargo workspace.
- **Graphics:** `wgpu` (Vulkan / Metal / DX12 backends), WGSL shaders. Use reverse-Z depth with an infinite far plane. Use camera-relative rendering (floating origin) so precision holds tens of thousands of blocks from spawn.
- **Windowing/input:** `winit`, `gilrs` for controllers. Raw mouse input via device events.
- **Math:** `glam` (f32 for rendering, f64/i64 for world positions). `bytemuck` for GPU data.
- **Parallelism:** `rayon` plus a dedicated priority job system (crossbeam channels) for chunk generation, meshing, LOD building and IO. Main thread never blocks on workers.
- **Audio:** `kira` (or `cpal` + own mixer if kira is insufficient) with spatial audio, category volumes, and a low-pass "underwater" filter.
- **Serialization:** `serde` + `ron`/`json` for data files; custom binary chunk format compressed with `zstd` (LOD cache may use `lz4_flex` for speed).
- **Hashing:** `rustc-hash` / `ahash` for hot maps.
- **Profiling:** `tracy-client` (feature-gated) and an in-game frame-time graph. GPU timestamp queries where supported.
- **Modding runtime:** `wasmtime` with the component model (WIT-defined API) for script mods.
- **Debug UI:** `egui` only for developer tools (feature-gated). All player-facing UI is a custom renderer that looks and behaves like the Minecraft UI (pixel font, 9-slice panels, buttons, sliders, scroll lists).

---

## 2. Workspace architecture

Design for moddability first: **the base game itself is registered through the same public API that mods use ("vanilla is a mod").**

```
crates/
  hearth_core       ids, registries, resource locations ("namespace:path"), events, tick scheduler, config
  hearth_math       coords (BlockPos/CubePos/ColumnPos, all i32), AABB, raycast (DDA), directions
  hearth_world      cubic-chunk storage (cubes + column heightmaps), palettes, light engine, block states, block entities
  hearth_worldgen   noise, climate, continents, biomes, terrain density, caves, aquifers, ores, trees, rivers
  hearth_lod        Distant-Horizons-style LOD data, fast surface sampler, LOD quadtree, LOD cache on disk
  hearth_entity     ECS (use `bevy_ecs` standalone or `hecs`; pick one and justify), physics, AI, pathfinding
  hearth_content    ALL vanilla content as data + the Rust systems registered via the mod API
  hearth_render     wgpu renderer: terrain, LOD, entities, sky, water, clouds, weather, post-processing
  hearth_ui         menus, HUD, inventory screens, fonts, localization
  hearth_audio      sound engine, sound events, ambience, music
  hearth_input      action map, rebinding, controller, input contexts
  hearth_save       region files, world metadata, player data, autosave, versioned migrations
  hearth_server     integrated authoritative server (simulation), runs in-process on its own thread
  hearth_client     client state, interpolation, prediction, renders what the server sends
  hearth_modapi     stable public API crate + WIT interfaces for WASM mods
  hearth_texgen     build-time/tool crate that generates the default texture & sound pack
  hearth            the binary (launcher, main loop, settings file)
tools/
  bench             headless benchmark runner (flythrough, worldgen throughput, meshing throughput)
assets/
  hearth/           generated default resource pack (textures, models, sounds, lang)
data/
  hearth/           data pack: blocks, items, recipes, loot tables, biomes, features, mobs, tags
```

**Client/server split:** even in single-player, simulation runs in `hearth_server` on its own thread at a fixed 20 TPS and communicates with the client over an in-memory message channel using the same message types a future network layer would serialize. This keeps multiplayer possible later without a rewrite. Rendering runs at an uncapped, independent frame rate with interpolation between ticks.

---

## 3. Game feel — match current Minecraft Java behavior

Match these observable behaviors closely:

- 20 ticks/second simulation. Day/night cycle of 24,000 ticks (20 real minutes), with dawn/dusk transitions and 8 moon phases.
- **Cubic chunks, no height limit.** The world is stored, generated, loaded, lit, saved and rendered as independent 16×16×16 **cubes** addressed by `(cx, cy, cz)` in i32 — there are no fixed-height columns, no build limit and no bedrock floor. Only cubes near the player (and near the surface) are generated; a mountain 2,000 blocks tall or a trench 3,000 blocks deep costs nothing until someone goes there. Sea level is Y = 0 (simplifies all depth/altitude math). Block Y is i32; keep an internal soft limit of ±2²⁴ blocks purely for safety. Horizontally the world is a **wrapping planet** (§6.2): X wraps modulo the planet circumference C, Z runs pole to pole. All code that compares, subtracts or looks up positions (physics, entities, rendering, audio, pathfinding, LOD, saves) must be wrap-aware in X from day one — put the wrap logic in `hearth_math` coordinate types so nothing else can get it wrong.
- Player: 0.6×1.8 AABB (1.5 tall sneaking, 0.6 swimming/crawling), eye height 1.62. Walk ≈ 4.317 m/s, sprint ≈ 5.612 m/s, sneak ≈ 1.295 m/s. Jump apex ≈ 1.25 blocks. Gravity 0.08 blocks/tick² with 0.98 vertical drag, ground friction via block slipperiness (ice slippery). Step-up height 0.6. Sprint-jumping, sneaking edge-protection, swimming, ladder climbing, fall damage (> 3 blocks), drowning with bubble meter, lava is **not** in the game (see §5), fire damage only from future content.
- Block interaction reach 4.5 blocks; entity reach 3 blocks (survival). Creative reach 5.
- Mining: per-block hardness, correct-tool multipliers, tool tiers (wood < stone < iron < diamond; gold is fast but low durability), mining crack overlay in 10 stages, block-break particles, instant break in creative.
- Health (20 half-hearts), hunger, saturation and exhaustion as in current survival; natural regeneration when hunger ≥ 18; starvation damage by difficulty. Difficulty: Peaceful / Easy / Normal / Hard.
- Game modes: Survival, Creative (flight with double-tap jump, creative inventory with search tabs), Spectator.
- Death screen, respawn at bed or world spawn, item drop on death (with `keepInventory` game rule).
- Beds: set spawn, sleep through night (and thunderstorms), cannot sleep with hostiles nearby.
- Chat and commands: `/gamemode`, `/time set|add`, `/weather clear|rain|thunder`, `/tp`, `/give`, `/kill`, `/seed`, `/gamerule`, `/difficulty`, `/locate biome`, `/summon`, `/effect` (only effects that exist), `/fill`, `/setblock`, `/help`. Tab-completion.
- F3 debug overlay faithful in spirit: FPS, frame time graph (F3+2 style), coordinates, facing, cube and column coordinates, biome, light levels, targeted block/state, memory, GPU name, chunk/LOD counts, worker queue depths. F3 key combos: F3+A reload chunks, F3+B hitboxes, F3+G chunk borders, F3+T reload resources, F3+H advanced tooltips, F3+Esc pause without menu.

---

## 4. Controls & settings — match the current Minecraft options screens

### 4.1 Default key bindings (all rebindable)
W/A/S/D move · Space jump · Left Shift sprint · Left Ctrl sneak · Left Alt inventory · X drop (Ctrl+X drop stack) · Mouse 5 swap offhand · 1–9 hotbar and mouse wheel · LMB attack/destroy · RMB use/place · MMB pick block · T chat · / command · Tab player list · F1 hide HUD · F2 screenshot (PNG into `screenshots/`) · F3 debug · Mouse 4 cycle perspective (first / third back / third front) · M world map (flat / globe) · F11 toggle fullscreen. (Creative "saved hotbar" keys are not needed.)

### 4.2 Controls screen
- Key Binds list grouped by category (Movement, Gameplay, Inventory, Creative, Multiplayer, Miscellaneous, Debug), with click-to-rebind, conflict highlighting in red, per-binding Reset, and Reset All. Supports keyboard keys, mouse buttons, and modifier combos.
- Mouse Settings: sensitivity, invert Y, raw input, scroll sensitivity, discrete scrolling, touchscreen mode omitted.
- Toggle vs Hold for Sneak and Sprint. Auto-jump (off by default).

### 4.3 Video settings (match names/behavior; add the extras listed)
- Graphics preset: Fast / Fancy / Fabulous-equivalent quality levels, plus a **Custom** preset once any sub-setting changes.
- Render Distance (full-detail chunks, 2–32), **Vertical Render Distance** (full-detail cubes above/below the player, 4–32), and **Simulation Distance** (5–32), separate.
- **LOD Distance** (Distant Horizons–style, 0 = off, up to 4096 chunks; see §8).
- Max Framerate slider 10–260 plus **Unlimited**. VSync toggle. Present-mode selection (Immediate / Mailbox / FIFO) under an advanced sub-menu.
- **Display mode: Windowed / Borderless Fullscreen / Exclusive Fullscreen**, resolution & refresh rate selector for exclusive mode, monitor selector. Remember window size/position.
- GUI Scale (Auto, 1–N), Brightness (Moody ↔ Bright; see §9.5), FOV (30–110) with FOV Effects slider, View Bobbing, Distortion Effects, Screen Effects, Clouds (Off / Fast / Fancy / Volumetric), Cloud height, Particles (All / Decreased / Minimal), Mipmap Levels (0–4), Biome Blend (0–15 blocks), Entity Distance, Entity Shadows, Smooth Lighting, Chunk Builder mode (Threaded / Semi-blocking / Fully-blocking), Attack Indicator, Anisotropic filtering, Anti-aliasing (Off / FXAA / TAA), Render scale (50–200%).
- **Shader Quality** group: Water quality, Shadows (Off / Low / Medium / High), Volumetric fog/light shafts, Sky quality, Weather effects, Auto-exposure.

### 4.4 Other settings
Sound (Master, Music, Weather, Blocks, Hostile, Friendly, Players, Ambient, Voice/Speech not needed; output device selector; subtitles), Language (English complete; architecture fully localized via lang files), Chat settings, Accessibility (text background opacity, high-contrast, reduce motion, darkness pulsing, glint strength n/a), Skin customization omitted (single built-in player model with a selectable default skin).

All settings persist to `options.toml` next to the executable (or platform config dir), are applied live without restart, and have correct defaults.

---

## 5. Content scope (intentionally minimal but complete and cohesive)

### Explicitly EXCLUDED
No structures of any kind (villages, temples, mineshafts, strongholds, ruins, dungeons, ships, etc.). No Nether, End, portals, or other dimensions. No redstone, redstone ore, lapis, emeralds, copper, amethyst, enchanting, brewing, XP orbs, villagers, trading, lava, obsidian, bedrock (the world has no floor — see §3). No biomes beyond those in §6.6 (no jungle/acacia/dark-oak/mangrove/cherry trees, no badlands, mushroom fields or other fantasy biomes). No mobs beyond those listed.

### Blocks (each with proper states, sounds, hardness, drops, tool, textures)
- Terrain: grass block, dirt, coarse dirt, podzol, rooted dirt, mud (wetland edges), clay, sand, red sand, gravel, stone, deepslate, granite, diorite, andesite, tuff, calcite, sandstone and red sandstone (+ smooth/cut; desert mesas, layers under dunes, beaches and sandy sea floors), mossy stone surfaces, snow layer, snow block, ice, packed ice (glaciers), water (source + flowing, with level states).
- Ores (stone and deepslate variants): **coal, iron, gold, diamond only.** Raw iron/raw gold drops, smelt to ingots; blocks of coal, iron, gold, diamond.
- Vegetation: **oak, birch and spruce only** — logs/wood/stripped/planks/leaves/saplings (leaves decay when unsupported); short & tall grass, fern & large fern, flowers (a curated temperate set: dandelion, poppy, cornflower, oxeye daisy, lily of the valley, azure bluet, alpine-style blue/purple flowers for high meadows), mushrooms, moss & moss carpet, vines, lily pads, reeds/sugar cane at water edges, cactus and dead bushes (deserts), dry grass (steppe/savanna), seagrass, kelp, sweet berry bushes (spruce forests), blueberry-like low shrubs optional, pumpkins, wheat crops (via seeds from grass), farmland, fallen logs as decoration.
- Crafted/building: planks, slabs, stairs, fences & gates, doors, trapdoors, ladders, crafting table, furnace, chest (single and double), torch & wall torch, lantern, glass & glass panes, bricks (clay → brick), stone bricks (+ mossy/cracked), cobblestone (+ mossy), polished stones, wool (natural sheep colors: white, light gray, gray, black, brown), carpets, bed (wool colors available), bookshelf is out (no enchanting), hay bale, composter.

### Items
Wood/stone/iron/gold/diamond: pickaxe, axe, shovel, hoe, sword. Leather/iron/gold/diamond armor (with visible armor on player & zombies/skeletons). Shield. Bow, arrows. Shears, bucket (empty/water/milk), flint (from gravel), flint and steel is out, compass (points to spawn), clock, bone, bone meal (growth), string, feather, leather, stick, wheat, seeds, bread, apple, all raw & cooked meats (pork, beef, chicken, mutton), rotten flesh, egg (throwable, can hatch chicks), sugar, cake, pumpkin pie, sweet berries, mushroom stew, lead, name tag (craftable here as a small accommodation), boat (oak/birch/spruce), bowl, saddle (ride horse), pack (crafted with leather, put in backpack slot (armor) for additional inventory slots or on horse)
Complete recipe set for everything above (crafting 2×2 and 3×3, shaped/shapeless, smelting with fuel values). A recipe book UI with search and "craftable only" filter.

### Mobs
- Passive: **pig, cow, sheep, chicken, horse** — wander, flee when hurt, follow held food, breed with correct food, baby variants that grow up, correct drops, sheep shearing and grass eating/wool regrowth, cows milkable, chickens lay eggs and flutter-fall. Horses can be saddled and ridden and 1 pack can be attached to their back.(pigs are not rideable.)
- **Wolf / dog:** spawns in packs in spruce and mixed forests (rarely in oak/birch); tame with bones; sit/follow toggle; teleport to owner when far; attacks what the owner attacks and what attacks the owner; heal with meat; collar; begging head tilt; wet-shake animation after rain/water.
- Hostile: **zombie** (burns in sunlight unless helmet/shade/water, breaks doors on Hard, calls reinforcements lightly, baby zombie variant), **skeleton** (bow AI with strafing and retreat, burns in sunlight, drops bones/arrows), **spider** (wall-climbing, leaps, neutral in bright light, drops string; spider jockeys omitted).
- Spawning follows modern rules: hostiles spawn only at block light 0 and sky light low enough, respecting spawn caps per category, despawn distances, and peaceful difficulty. With unlimited depth, hostile spawning is restricted to cubes within the simulation distance of a player (in 3D), so deep cave systems don't fill with idle mobs. Passive animals spawn primarily with chunk generation in grassy biomes.
- AI: goal-selector architecture (prioritized goals like the real game's) + A* pathfinding over a navigation grid with jump/fall/door/water handling, path caching, and per-tick budget. Entity physics shares the player's collision code.
- Each mob has a proper model (box-based, animated walk/idle/attack/hurt/death), hurt flash, knockback, death poof particles, and sound events.

### Combat
Attack cooldown with attack indicator, critical hits when falling, sweep attacks for swords, shield blocking, knockback, armor & toughness damage reduction, invulnerability frames, bow draw strength.

---

## 6. World generation — a whole Earth-like planet

Goal: exploring a world should feel like **exploring the real Earth at a smaller scale**. The world has an equator and poles. Continents have the shapes, mountain types, river systems and coastlines that real continents have, *for the same physical reasons*. Climate and biomes land where they would on Earth: rainforest along the equator, great deserts in the subtropics and behind mountain ranges, grassland steppes in continental interiors, temperate forests on mid-latitude coasts, taiga and tundra toward the poles, ice caps at the poles. A seasoned player should be able to look at a map of a new world and reason about it like a geographer ("that's a west coast in the subtropics next to a cold current, so it's desert; that plateau behind the great range is dry and cold").

Do **not** pick biomes from independent temperature/humidity noise the way the real game does. Instead, simulate a simplified, deterministic **planet model**: tectonics → elevation → erosion & drainage → atmospheric circulation & ocean currents → temperature & precipitation → climate class → biome. Noise only adds natural variation on top of those physical causes.

Horizontal distances are compressed so a continent is explorable in a play session; vertical relief is exaggerated relative to horizontal so the extremes feel epic when you find them. Everyday terrain is modest and walkable; **the spectacular features are rare on purpose**, so finding one feels like a discovery.

Expose at world creation (details in §6.2): **Planet Size** (a scale preset, with the most playable one auto-selected), **Vertical Scale** (auto-matched to the planet size, overridable), **Feature Rarity** (Rare / Standard / Common for great mountains, trenches and giant caverns), **Land Fraction** (Earth-like ~30% default; 20–50%), and **Spawn Climate** (Temperate default / Random). Defaults: Standard planet (auto), auto vertical scale, Rare, Earth-like, Temperate.

### 6.1 Architecture requirement (critical for LOD and cubic chunks)
All generation above the per-block level must be expressed as **pure, deterministic, thread-safe sampling functions of (seed, x, z[, y])** so that the LOD system can evaluate them at arbitrary coarse resolutions, and so a single cube at any altitude can be generated without generating the cubes above or below it:
- `planet(x, z) -> {latitude, longitude, sphere_point, plate_id, plate_type, boundary_type, boundary_distance, crust_age}`
- `climate(x, z) -> {mean_temperature, temperature_range, annual_precipitation, dry_season, prevailing_wind, ocean_current_temp, climate_class}`
- `surface_height(x, z) -> f32` (land or sea floor), `water_level(x, z)` (sea level 0, or a lake/river surface), `biome(x, z[, y])`, `surface_material(x, z)`, `tree_canopy(x, z)`, `river(x, z)`, `snow_line(x, z)`, `tree_line(x, z)`
- `density(x, y, z)` for the 3D refinement (overhangs, cliffs, caves), evaluated only inside the cube being generated.

Large-scale analyses that need whole-planet context (distance to coast in each direction, which side of a continent a point is on, ocean basins and their currents, upwind ocean fetch, continental drainage) are computed once per world on a **global planet grid** on the sphere (e.g., an icosphere or cube-sphere grid of ~1–4 million cells, independent of planet size) during world creation, with a progress bar, and saved in the world folder. Finer detail (erosion, local drainage) is computed on demand in deterministic tiles with overlapping margins, cached with LRU, and always refined from the global grid so results are seam-free and identical regardless of which tile computed them. Every sampling function is wrap-aware in X.

Cube generation classifies each cube cheaply first: **entirely above terrain and water → empty (air) without evaluating noise; entirely far below the surface and outside any cave-region bounding volume → solid stone/deepslate fill fast path**; only cubes that intersect the surface band, a cave region, an ore vein check, or water are fully evaluated. This is what makes unlimited height free. Decorations that cross cube borders (trees, boulders, fallen logs, cacti) are placed in a separate **population** stage that runs for a cube only once its needed neighbours have terrain, and each feature is seeded by its origin position so the result is identical no matter which order cubes load in. Full generation must agree with `surface_height` at the surface; test this agreement (§14).

### 6.2 The planet: a wrapping sphere
The world is a whole planet. A voxel grid cannot be literally round without bending blocks at seams, so use the approach that keeps every block square while making the world behave like a globe:

- **Geography is simulated on a true sphere.** Plates, continents, noise, currents and climate are all computed on the unit sphere (3D noise sampled at points on the sphere; plate seeds on a jittered Fibonacci sphere). This guarantees one coherent planet with no seams, correct latitudes, and oceans and continents that connect properly all the way around.
- **The playable world is a conformal (Mercator-style) projection of that sphere.** X is longitude and **wraps seamlessly**: X is taken modulo the planet circumference C, so sailing or walking east long enough brings you back to where you started. Z is latitude through the Mercator mapping φ(z) = atan(sinh(z / R)), R = C / 2π, which keeps landforms correctly shaped everywhere (no stretching distortion), at the cost of high-latitude regions being larger to traverse, just like on a Mercator map. The Z range is ±C/2 (φ up to ≈ ±85°), so the world is a C × C square.
- **Crossing a pole:** the last few degrees before each pole edge are a flat, featureless polar ice plateau (sea ice in one hemisphere and a high ice sheet in the other if the plate layout allows, like Earth), generated so the two sides of the pole match, with persistent polar blowing snow limiting visibility there. Walking past the pole edge places you on the far side of the pole (X + C/2) heading the other way, as on a real globe; the whiteout hides the transition. Don't render terrain across the pole edge.
- **Time zones:** the sun's position depends on longitude. The server keeps one global clock; local solar time = global time + (X / C) × 24,000 ticks. When it's noon at X = 0 it's midnight on the opposite side of the planet. Sky rendering, sunrise/sunset, mob burning, spawning light levels and ambient sounds use local time. Sleeping in single-player advances the global clock to local morning at the bed. `/time` sets local time at the player.
- **Latitude effects:** the sun's arc follows latitude: overhead at noon on the equator, lower toward the poles, with paler light, longer shadows and longer twilights at high latitudes. Design the sun model so an optional **seasons** setting (axial tilt, configurable year length, polar day and night) can be added later; it's a stretch goal, off by default.
- **F3** shows latitude and longitude in degrees, local solar time, climate class and biome. The compass points north by default (spawn-pointing as an option). Add a **world map screen** (M key) that shows explored areas both as a flat map and as a rotatable globe, so the player can see where they are on the planet.
- Spawn in a temperate mid-latitude lowland (≈ 35–55°) near a coast or river, unless Spawn Climate is Random.

**Planet Size presets** (C must be a multiple of the largest LOD tile size, e.g. 4,096):

| Preset | Circumference C | Scale vs Earth | Auto vertical scale | Feel |
|---|---|---|---|---|
| Tiny | 16,384 | ~1:2,450 | 1:8 | quick worlds, continents a short walk |
| Small | 32,768 | ~1:1,220 | 1:6 | compact but varied |
| **Standard (Recommended — auto-selected)** | **65,536** | **~1:610** | **1:4** | about 3 hours to sprint around the equator; continents 3,000–15,000 blocks; the best balance of variety and travel time |
| Large | 131,072 | ~1:305 | 1:3 | long expeditions |
| Huge | 262,144 | ~1:150 | 1:2 | epic journeys |
| Vast | 1,048,576 | ~1:38 | 1:1 | near-real distances |
| Earth 1:1 | 40,075,264 | 1:1 | 1:1 | real size: 8,800-block peaks, 11,000-block-deep trenches |
| Custom | any multiple of 4,096 ≥ 16,384 | — | chosen by rule below | — |

The Create World screen shows the preset with its travel-time estimate and "Recommended" on Standard. All macro features (continent sizes, ocean widths, shelf widths, plate sizes, the whole climate pattern) scale with C. Block-level features (trees, caves, cavern size, river bank shapes) do not. **Slopes must stay believable at every size:** horizontal compression is always stronger than vertical compression, so the generator constrains each landform's footprint to realistic slope limits relative to its height. On smaller planets mountains become relatively wider (taking up more of their continent) rather than turning into spikes. The auto vertical scale in the table is chosen by that rule; the user can override it (0.25×–2× of auto). All Y numbers in §6.3 are for Standard at auto vertical scale and scale proportionally.

### 6.3 Vertical profile (numbers for the Standard planet at auto vertical scale)
Model the real Earth's hypsometric curve (most land is low; most ocean floor is deep), compressed roughly 1:4 vertically:
- **Coasts & lowlands:** most land sits between Y 0 and ~Y 120 — plains, rolling forested hills, river valleys. This is where the player spends most of their time.
- **Uplands & hills:** Y 120–400, common in continental interiors and on old shields.
- **Great mountain ranges (rare):** foothills rising to massifs with peaks of **Y 1,000–2,200**, the tallest summits rare enough to be landmarks. Glaciers (packed ice + snow) in high cirques and valleys; snow line and tree line derived from the climate model (high near the equator, low near the poles, lower on wet windward slopes). Alpine meadows above the tree line, bare rock and scree above that. Peaks break through the cloud layer.
- **High plateaus (rare):** broad, cold, dry uplands at Y 500–900 behind the greatest collision ranges.
- **Continental shelf:** from the coast, shallow sea (Y 0 to about −60), wide off passive-margin coasts (~400–800 blocks on Standard, scaling with planet size) and narrow off active-margin coasts (~50–150 blocks), sandy/gravelly with kelp and seagrass in cool water.
- **Continental slope:** a steep drop from the shelf edge down to the abyss over a few hundred blocks — standing at the shelf edge underwater should feel like staring into a void.
- **Abyssal plains:** the typical deep-ocean floor at about **Y −800 to −1,100**, flat to gently rolling with sediment (clay/gravel/sand), scattered seamounts and guyots.
- **Ocean trenches (rare):** long, narrow arcs where oceanic plates subduct, reaching **Y −2,500 to −3,000**.
- **Mid-ocean ridges:** long, raised underwater mountain chains running through the middle of ocean basins where plates diverge.

Realistic underwater light: sunlight fades exponentially with depth (§9.3), so the deep ocean is pitch black below a few hundred blocks. Water pressure is not simulated.

### 6.4 Tectonics & landforms (why things are where they are)
- **Plates:** a tectonic map of jittered, domain-warped spherical Voronoi plates, each continental or oceanic, with a rotation (Euler pole) that gives it a realistic drift direction on the sphere. Continental crust clusters into a realistic size mix for a whole planet: one or two very large continents (the biggest spanning up to about a third of the circumference), several mid-sized continents, a subcontinent that has collided into a larger one, large islands, and island arcs. Land fraction targets the Land Fraction setting.
- **Coastlines:** fractal and varied — peninsulas, bays, gulfs, inland seas, straits, capes, barrier islands along low coasts, estuaries, and **fjords** on high-latitude mountainous coasts.
- **Boundary types produce the landforms real Earth has:**
  - *Continent–continent collision* → the rare great ranges (long, branching chains with a main divide, Himalaya-like), with a **high plateau** behind the widest ones.
  - *Ocean–continent subduction* → a long **coastal mountain chain** parallel to the coast (Andes-like) with volcanic peaks, a narrow shelf, and a **trench** offshore.
  - *Ocean–ocean subduction* → curved **volcanic island arcs** with a trench on the outer side.
  - *Divergent on land* → **rift valleys** with long, deep lakes; *divergent at sea* → mid-ocean ridges.
  - *Transform* → linear fault valleys and offset ridges.
  - *Passive margins* (continent edges far from any boundary) → wide coastal plains, broad shelves, large river deltas.
  - *Old interiors* → low, flat **shields and cratons** (plains, lake-dotted in cold regions) and **old eroded ranges** (rounded, forested, Appalachian-like) where ancient boundaries used to be.
  - *Hotspots* → isolated **volcanic island chains** in the ocean, youngest and tallest at one end, progressively more eroded and sunken toward the other end, finishing as atolls and seamounts. Volcanoes are stone cones with craters (sometimes crater lakes); no lava.
- **Erosion for realism:** a coarse, deterministic hydraulic + thermal erosion pass on the tiled heightfield (16–32 blocks per sample, overlapping tile borders so results are seam-free). Produces dendritic valleys, sharp ridgelines, talus slopes and alluvial fans. **Glacial carving** in cold, high or high-latitude regions: U-shaped valleys, cirques, fjords, and many small lakes on scoured shields. Arid regions erode differently: mesas, buttes, dry washes, and escarpments.
- **Rivers & drainage basins:** flow accumulation on the eroded heightfield produces continental drainage basins with continental divides along mountain crests. Great rivers drain large basins toward passive-margin coasts and end in deltas; short steep rivers run off coastal ranges; some interior basins have **no outlet** and end in salt-flat-like dry lakes (white sand/calcite surface) or saline lakes in arid zones. Rivers widen, deepen and meander downstream, never flow uphill, and step down with waterfalls where they cross steep terrain. In deserts, rivers from distant mountains can cross the desert as green ribbons (oasis corridors).

### 6.5 Climate model (the heart of the realism)
Compute climate on the coarse macro grid from physical causes, then smooth and add mild noise:
- **Temperature:** from latitude (warm equator, cold poles), minus altitude lapse rate (~6.5 °C per real km, scaled with Vertical Scale), plus **ocean current** influence on coasts, plus **continentality** (interiors far from the sea have more extreme and on average colder temperatures at high latitudes).
- **Atmospheric circulation by latitude band:** equatorial low (rising air, very wet), subtropical highs at ~20–35° (sinking air, dry — the desert belts), mid-latitude westerlies (wet, stormy), polar highs (cold, dry). Prevailing winds per band: easterly trade winds in the tropics, westerlies in mid-latitudes, polar easterlies near the poles.
- **Ocean currents per basin (rule-based):** warm currents flow poleward along the **east** coasts of continents in the subtropics and mid-latitudes (warmer, wetter coasts); cold currents flow equatorward along the **west** coasts (cooler, drier, foggy coasts). At high latitudes on the eastern side of ocean basins, warm drift keeps western continental coasts mild (Western-Europe-like).
- **Precipitation:** starts from the latitude band value, then moisture is carried by the prevailing wind from the upwind ocean — highest on windward coasts, decreasing with distance inland, boosted by **orographic lift** on windward slopes, and strongly reduced in **rain shadows** leeward of mountain ranges and on high plateaus. Cold currents suppress rain; warm currents enhance it. Seasonal monsoon-like wetness on large tropical continents' coasts is a stretch goal.
- **Climate class:** classify each cell with a simplified **Köppen-style** scheme: tropical rainforest, tropical savanna, hot desert, hot steppe, cold desert, cold steppe, Mediterranean, humid subtropical, oceanic, humid continental, subarctic (boreal), tundra, ice cap, plus highland overrides from altitude.
- The result must reproduce Earth's recognizable patterns: deserts on the west sides of continents in the subtropics and in continental interiors and rain shadows; rainforest along the equator; savanna between them; Mediterranean climates on subtropical west coasts poleward of the deserts; lush humid-subtropical east coasts; broad boreal forest belts across high-latitude continental interiors; mild wet oceanic west coasts at mid-high latitudes; tundra and ice toward the poles.
- **Weather uses the same model** (§9.4): weather cells drift with the prevailing wind of their latitude band; storms are frequent in the equatorial belt and mid-latitude westerlies, rare in desert belts; fog banks form along cold-current coasts; snow falls wherever it is cold enough by latitude or altitude.

### 6.6 Biomes (Earth-like zones, kept compact)
Each climate class maps to a small set of biomes, using only **oak, birch and spruce** trees (with varied growth forms per climate so zones still look distinct). Transitions are wide and gradual, with mixed edges and natural clearings:
- **Tropical rainforest** (equatorial) — dense, towering "giant oak" growth forms with large crowns, vines, ferns, thick undergrowth, many rivers and swampy lowlands; the lushest, greenest tints.
- **Savanna / tropical grassland** — tall golden grass, scattered flat-crowned oaks, dry-season yellow tints.
- **Hot desert** — sand and red sand dune seas (shaped by prevailing wind direction), rocky regs (gravel/stone plains), mesas and escarpments of sandstone and red sandstone, dry washes, salt flats in closed basins, cactus and dead bushes, rare oases (small lakes with grass and oaks) where water tables or rivers reach the surface.
- **Steppe / dry grassland** — short, dry-tinted grass, few or no trees, rolling plains in continental interiors and rain shadows; cold deserts in high interiors (gravel, sparse grass, frost).
- **Mediterranean scrub** — dry grass, sparse small oaks and shrubs on sunny hills near subtropical west coasts.
- **Temperate plains / meadows** — open, rolling, flowers, scattered oaks and copses.
- **Temperate broadleaf forest** — oak and birch, varied tree sizes, rare large old oaks, fallen logs; birch-dominant patches; lush humid-subtropical variants on warm east coasts.
- **Oceanic rainforest-like coast** — very wet mid-latitude windward coasts: huge mossy spruces and ferns.
- **Mixed forest** — oak/birch with spruce, transitional toward boreal and on mid-altitude slopes.
- **Boreal forest (taiga)** — dense spruce, podzol and moss floor, ferns, berry bushes, countless lakes on glacially scoured shields; snowy variant in the colder part.
- **Tundra** — low grass, moss, lichen-colored coarse dirt, small shrubs, frozen lakes, permanent patchy snow; no trees.
- **Ice cap / polar ice sheet** — vast snow and packed-ice plateaus, glaciers flowing to the sea, ice shelves and icebergs along polar coasts.
- **Highland zones** (from altitude, on top of any climate): montane forest → krummholz near the tree line → alpine meadow → rocky slopes & scree → snowcaps and glaciers. Tropical mountains have a much higher tree and snow line; desert mountains are bare rock with sparse life.
- **Water & coasts:** rivers, lakes, rift lakes, wetlands and marshes in flat poorly drained lowlands, beaches on gentle coasts, stony shores and sea cliffs on steep coasts, barrier islands and lagoons on low passive coasts, fjords at high latitude.
- **Oceans by depth and temperature:** shelf seas (turquoise and clear in the tropics, green-grey in cold waters), deep ocean, trenches, **coral-free** warm shallows with bright sand, cold seas with kelp forests, polar seas with sea ice and icebergs.
Biome blending with the Biome Blend setting; grass/foliage/water/fog tints come from climate (temperature × precipitation colormaps) rather than per-biome constants, so color shifts smoothly across the world just as it does from space.

### 6.7 Surface detail for realism
- Soil depth varies with slope, climate and position: thick soil in valleys and wet climates, thin soil on ridges and in arid zones, bare stone on slopes steeper than ~45° and on cliff faces; gravel and scree at cliff bases; exposed rock outcrops in hills.
- Trees: procedurally varied shapes per species and climate (not a few fixed templates), trunk height and crown size varying with age and moisture, denser at forest cores, thinning at edges and toward dry or cold limits, avoiding steep slopes and water. Tree density responds smoothly to precipitation, temperature and altitude.
- Coastal realism: sand where waves deposit it (gentle, sheltered coasts; broad beaches on passive margins), cliffs where terrain meets the sea steeply, sea stacks off eroding cliff coasts (rare), dunes behind wide beaches.
- Boulders, fallen logs, moss patches, forest-floor variation, flowers in clusters rather than uniform scatter.

### 6.8 Underground
- Stone with blobs of granite/diorite/andesite/tuff (granite more common in old shields and mountain roots, tuff near volcanic arcs); sandstone layers under deserts; stone blends into **deepslate** starting around Y −40 to −60 below the local surface/sea floor; below Y −512, deepslate dominates. Deep underground there is no floor — stone continues forever.
- **Caves, scaled for rarity and awe:**
  - Normal cave networks (winding tunnels and small chambers) are **sparser than in the real game** — you can dig a mine without constantly breaking into caves — concentrated in cave-rich regions (karst-like zones in humid climates and fractured mountain roots).
  - **Giant caverns (rare):** vast chambers hundreds of blocks across and 100–300 blocks tall, with underground lakes, pillars, hanging vines/moss near openings in humid regions, and sometimes waterfalls from rivers above. Roughly one per several square kilometres at Rare setting, placed by a sparse deterministic region grid so each one is guaranteed to feel special. Some connect to the surface through a sinkhole or cliff-side mouth so they can be discovered from above ground.
  - **Deep cave systems** continue far below Y −500, becoming sparser with depth; they're pitch black, silent and slightly foggy.
  - Ravines/canyons (rare; slot canyons in arid regions), sea caves on cliff coasts, flooded caves via local aquifers.
- **Ores** (distribution bands relative to absolute Y, extended for unlimited depth, with realistic geological bias):
  - Coal: common from the surface down to about Y −200, richest in old lowland basins and foothills.
  - Iron: common Y −300 to 0, plus a band inside mountains above Y 200; richer in old shields.
  - Gold: Y −600 to −60, extra in mountain roots, volcanic arcs, and beneath river valleys that drain mountains.
  - Diamond: begins around Y −300, grows more common with depth down to about Y −1,200, then stays constant; slightly richer under old shields; reduced when exposed to air. Deep diamonds are the reward for serious expeditions.
  - Everything is deterministic per cube so cubes generate independently.

### 6.9 Performance targets for generation
Multithreaded, SIMD-friendly noise (evaluate noise in batches on a coarse grid then trilinearly interpolate density). The global planet grid is built once at world creation (target: under 20 seconds on an 8-core CPU for any planet size, with a progress bar). Finer macro data (erosion tiles, local drainage) is computed lazily per tile and cached with LRU. Empty/solid fast paths for cubes. Target ≥ 3,000 cubes/second of surface-band cubes on an 8-core desktop CPU (≈ 400 columns/s equivalent), near-instant air/solid cubes, and far more for LOD surface sampling (§8).

---

## 7. World storage, meshing and near-field rendering

### 7.1 Data (cubic chunks)
- The unit of storage, generation, lighting, meshing, saving and culling is the 16×16×16 **cube**. Cubes live in a hash map keyed by `(cx, cy, cz)`; a per-column (`cx, cz`) record keeps the heightmaps and which Y-range of cubes is loaded. The loaded set is a 3D region around the player: horizontal Render Distance × **Vertical Render Distance** (in cubes above and below), plus surface cubes out to the horizontal radius so terrain is never missing when you stand on a peak or dive in the sea. Air-only and uniform-solid cubes are stored as a single value with no allocation.
- Cubes store block states with **palette compression** (bit-packed indices; single-value fast path). Light stored as nibble arrays (sky + block), allocated lazily.
- Block state registry is data-driven; each state has precomputed flags (opaque, full cube, transparent, emits light, occlusion faces, collision shape id, render layer).
- Light engine for cubic chunks: block light is plain 3D BFS across cube borders. **Sky light** cannot assume the world's top is loaded, so seed it from a per-column "highest light-blocking block" heightmap that is initialized from `surface_height` (§6.1) before real cubes exist and corrected as cubes load and blocks change (the approach used by cubic-chunk voxel engines). Cubes entirely above the heightmap are fully sky-lit without storing light; cubes entirely below it and not connected to the surface are dark. Underwater sky light attenuates with depth. Incremental updates on block change, batched per tick, multithreaded across independent regions; correct opacity for leaves/water.
- Fluids: water spreading with source-block rules, infinite sources, flowing current pushing entities, waterlogging for slabs/stairs/fences/etc. Ocean water below sea level is generated as source blocks; fluid simulation only ticks in cubes near players.
- Random ticks: crop growth, grass spread, leaf decay, snow/ice formation during snowfall, sapling growth.
- Saves: 3D region files (e.g., 16×16×16 cubes per region file), so a deep dive or tall build doesn't bloat a column file. Cube and region keys are stored in canonical wrapped X, so the world has no seam on disk.
- **Wrap seam:** cube lookups, neighbour access for meshing and lighting, fluid flow, entity collision and pathfinding all go through wrapped coordinates, so the seam at X = 0 / C is indistinguishable from anywhere else. Entities crossing the seam have their X re-canonicalized without any visible jump.

### 7.2 Meshing
- Asynchronous meshing on worker threads with a priority queue ordered by distance and view direction; remesh neighbours on boundary edits; edits near the player remesh with top priority in the same frame when possible (no visible "hole" after breaking a block).
- **Binary greedy meshing** (bitmask-based) for full cubes where it does not break per-face texture/AO/light variation; merge only faces with identical texture, AO and light; fall back to per-face quads otherwise. Non-cube models (stairs, fences, torches, plants) from a baked model cache (vanilla-format JSON block models).
- Compact vertex format: pack each quad into ≤ 8–12 bytes (position within cube, size, face, texture layer, AO, light) and expand in the vertex shader (vertex pulling from storage buffers). Target ≤ 20 bytes of GPU memory per visible face on average.
- Smooth lighting and ambient occlusion computed per vertex at mesh time; biome tint computed per vertex with blending.
- Separate render layers: opaque, cutout (leaves/plants, alpha-test with mip-aware coverage), translucent (water/ice/glass) sorted back-to-front per cube with incremental re-sorting only when the camera crosses cube boundaries.

### 7.3 GPU submission & culling
- All cube meshes live in a few large GPU buffers managed by a sub-allocator (no per-chunk buffers). Upload through a staging ring buffer with a per-frame byte budget.
- **GPU-driven rendering:** compute-shader frustum culling + Hi-Z occlusion culling (depth pyramid from previous frame, reprojected) + per-face-direction backface culling at the cube level, writing indirect draw commands; use `multi_draw_indexed_indirect` (and `_count` where supported), with a CPU fallback path.
- A single shared index buffer for quads.
- Cave culling via cube-visibility graph (flood fill through connected open faces from the camera cube, like the real game's section graph) — with cubic chunks this also skips everything far above and below the player, so deep caves and the surface never render at the same time unless actually visible.
- Texture atlas as a `texture_2d_array` with full mip chain (proper alpha-weighted mip generation for cutout textures) and anisotropic filtering.

---

## 8. Distant Horizons–style LOD system (modelled on DH 3.3 behavior)

The player must be able to stand on a mountain and see continents and ocean to the horizon, with the terrain seamlessly continuing past the full-detail render distance, at high FPS.

### 8.1 Data model
- A quadtree of LOD tiles over the wrapped XZ plane (C is a multiple of the largest tile size, so tiles tile the planet exactly and the seam never splits a tile). Detail level 0 = 1 block per column; each level halves resolution (level n ⇒ 2ⁿ blocks per column). A tile covers a fixed number of columns (e.g., 64×64) at its level.
- Each LOD column stores a short list of vertical **segments** `(bottom_y: i32, top_y: i32, material/color id, sky light, block light)` so overhangs, cliffs, water surfaces, the sea floor thousands of blocks down, and floating features are representable; typical columns have 1–4 segments (a deep-ocean column is just "sea floor segment + water segment"). Colors come from the block's averaged texture color × biome tint, computed once at resource load.
- Heights span thousands of blocks, so LOD meshes use per-tile origin offsets and 16-bit local heights (or tile-relative i32 in storage buffers); never assume a fixed world height anywhere in the LOD pipeline.
- Underground caves are not represented in LOD (only surface-connected overhangs and cave mouths); when the player is deep underground, LOD terrain is culled by the same visibility logic and ambient sky light is gone.
- LOD data is persisted in a separate compressed cache (`lod/` in the world folder), keyed by level and tile, written asynchronously. Target ≈ 50% less disk write work than a naïve approach by batching and only writing dirty tiles.

### 8.2 Generation (the key to speed)
- **High-speed surface generator:** for tiles beyond the full-detail radius, generate LOD columns *directly* from the worldgen sampling functions (§6.1) at the tile's resolution — never by generating full chunks. It samples height, water level, biome, surface material and tree canopy (represent forests as canopy height + leaf color with light noise, so distant forests look like forests).
- When real chunks are generated or edited, downsample them into level-0 LOD data and propagate upward through the quadtree, replacing the approximated data (player builds become visible from afar).
- Generation is prioritized by distance and screen importance, cancellable when the player moves away, and throttled to never starve near-field chunk generation or cause frame hitches.

### 8.3 Rendering
- Mesh LOD tiles into simplified column meshes (greedy-merged tops and exposed sides, skirts to hide cracks between levels). Choose the detail level per tile from distance with hysteresis so tiles don't flicker between levels.
- Near/far handoff: LOD terrain begins where full-detail chunks end, with a small overlap and **dithered crossfade**; LOD tiles under loaded chunks are clipped so there's no z-fighting or double geometry.
- Reverse-Z infinite projection so the far plane never clips distant terrain.
- **Temporal anti-aliasing** option for the LOD pass (and whole frame) to remove shimmer/flicker in distant terrain; **mipmapped** LOD textures/colors to eliminate grainy distant surfaces.
- Distant water rendered with the same water shader as near water (reflections of sky, Fresnel, color by depth), so oceans read correctly at the horizon — the shelf's turquoise shallows visibly darkening to deep navy past the shelf edge.
- From a 2,000-block summit you can see an enormous distance, so the far LOD rings, aerial perspective and fog must hold up at that scale. Apply **planet curvature** in the terrain and LOD vertex shaders (drop terrain by d²/2R, on by default, toggleable) so the view from high peaks shows a real horizon and far-off land sinks below it. Because horizontal distances are compressed more than heights, R is not the wrap radius C/2π (that would make horizons absurdly close); use R_eff = Earth radius × the world's vertical scale, so the horizon distance and dip from any altitude match the real view from the equivalent real altitude. On the Earth 1:1 preset R_eff equals the true radius.
- LOD rendering is wrap-aware: tiles are positioned with the shortest wrapped offset from the camera, so on small planets the view across an ocean can include land that is "behind" you around the globe only if it's actually within LOD distance (cap LOD distance at C/2 so no tile is drawn twice).
- LOD terrain receives the same fog, atmosphere, lighting, day/night and weather as near terrain — the transition must be invisible.
- Frustum + occlusion culling applied to LOD tiles through the same GPU-driven path.

### 8.4 Targets
LOD distance up to 4096 chunks configurable, default 256 chunks. With render distance 12 + LOD 512 chunks at 1440p on a mid-range GPU (RTX 3060 / RX 6600 class) and 8-core CPU: ≥ 144 FPS average, stable 1% lows, and no frame spikes > 2× median while flying at sprint-fly speed. VRAM for LOD bounded by a configurable budget with an LRU eviction policy.

---

## 9. Atmosphere, lighting & shaders ("lightweight but gorgeous")

Implement an HDR, physically based pipeline that stays cheap. Every effect has a quality setting and an Off option.

### 9.1 Pipeline
HDR render target (RGBA16F) → opaque terrain & entities → sky → translucent (water, glass, clouds) → volumetrics → TAA (optional) → bloom (lightweight, few mips) → auto-exposure (histogram, compute) → tonemapping (AgX or ACES-fitted; pick one and keep the Minecraft-like palette recognizable) → color grading → UI. Render scale option for upscaling.

### 9.2 Sky, sun, moon, stars
- Physically based atmosphere (Rayleigh + Mie + ozone) using small precomputed LUTs (transmittance, multi-scattering, sky-view) in the style of Hillaire's approach; recompute the sky-view LUT at low resolution per frame.
- The sun and moon follow the player's **latitude and local solar time** (§6.2): overhead at the equator's noon, low and slanting near the poles, with longer twilights at high latitudes. The star field rotates about the celestial pole, which sits at a height above the horizon equal to the player's latitude (so northern and southern skies differ).
- **Sunrise/sunset:** deep orange/red horizon glow, pink/purple zenith tones, the sun disc reddening and flattening near the horizon, long warm light on terrain, clouds lit from below at dusk.
- **Moon:** pixel-art style moon with 8 phases, correct orientation, moonlight intensity tied to phase.
- **Stars:** procedural star field with realistic magnitude distribution, subtle twinkle near the horizon, a faint Milky Way band, fading in with twilight and hidden by clouds/rain.
- Sky and fog colors blend by biome.

### 9.3 Water (inside and out)
- Surface: normal-mapped waves (sum of Gerstner/FFT-like layers, cheap), wind-driven, calmer in rivers/lakes, rougher in ocean and storms. Keep the blocky water-level geometry of the game; waves are a shading/normal effect with gentle vertex motion on the surface only.
- Reflections: screen-space reflections with sky/LUT fallback, Fresnel (Schlick), sun specular highlights (glitter path at sunset).
- Refraction of the underwater scene with depth-based absorption (Beer–Lambert, per-biome water color), shoreline foam from depth difference, soft shore edges.
- **Underwater:** blue-green exponential fog with distance and depth-dependent darkening — sunlight falls off exponentially with depth so the shelf is bright and turquoise, the continental slope fades to deep blue, and the abyss below a few hundred blocks is completely black (only torches/held light reach there). Light shafts (god rays) from the surface, animated caustics projected on shallow sea floors, Snell's window and total internal reflection when looking up at the surface, bubble particles, muffled audio (low-pass), slight view distortion (respecting Distortion Effects).
- Rain creates ripple normals on water surfaces.

### 9.4 Clouds, weather and fog
- Clouds: a Minecraft-style blocky layer (Fast/Fancy) **and** a lightweight volumetric option (ray-marched 2.5D cloud layer at quarter resolution with temporal reprojection). Cloud coverage follows weather. Cloud base sits around Y 250–450 (above sea level), so the rare great mountains pierce the cloud deck and you can stand above a sea of clouds; climbing through the cloud layer puts you in dense fog.
- **Altitude effects:** the sky deepens to a darker, more saturated blue at extreme heights, air haze thins, the sun gets brighter and whiter, and wind audio grows stronger. Temperature falls with altitude, driving snow instead of rain.
- **Weather system:** regional weather cells generated from the climate model (§6.5) that drift with the prevailing wind of their latitude band and wrap around the planet (so you can see a storm approaching over the ocean or see rain falling in the distance). States: clear, partly cloudy, overcast, fog, rain, thunderstorm, snow (by temperature/altitude). Realistic durations and transitions (clouds build before rain, rain tapers off, occasional morning fog in valleys/near water, clearing after storms). Rain shadows behind mountain ranges are drier; windward slopes are wetter; high mountains and cold regions get snow instead of rain and accumulate snow layers; orographic clouds cling to peaks.
- Rain & snow particles (GPU-instanced, occluded by overhead blocks via a heightmap), splashes on surfaces, wet surfaces darken and gain specular sheen, wind sways leaves, grass and crops (vertex animation, toggleable).
- Thunderstorms: lightning bolts (with sky/terrain flash, delayed thunder by distance), darkened sky, heavier wind.
- **Fog:** physically based height/distance fog (aerial perspective from the atmosphere LUT) so distant LOD terrain turns hazy blue, valley fog at dawn, denser fog in rain/swamps, cave fog darkening with depth. Optional low-res froxel or ray-marched volumetric fog with sun/moon light shafts through trees and cave openings (quality-scaled).

### 9.5 Lighting & darkness
- Keep the game's block/sky light values (0–15) for gameplay (spawning, crops), but render with an HDR lighting model: sky light × current sun/moon/sky irradiance, block light as warm (≈ 2700 K) torch light with smooth falloff, per-vertex AO.
- **Realistic darkness:** night brightness follows real ratios — a moonless, overcast night is near-black; a full moon gives dim, bluish, navigable light; caves without torches are pitch black. Auto-exposure is clamped so the eye adapts only within realistic limits. Add a subtle Purkinje shift (desaturation and blue shift at scotopic light levels). Torches are the lifeline at night — this should feel atmospheric, not frustrating.
- The **Brightness** setting shifts the exposure floor (Moody = strict realism, Bright = generous minimum) so players can choose.
- Optional cascaded shadow maps from sun/moon (Off by default on Fast, 2–3 cascades on higher presets, cached/rarely re-rendered cascades for distant ones), with light leaking prevented in caves by combining with sky-light values.
- Dynamic held-item light (holding a torch lights surroundings) as an option.

---

## 10. Textures, models, audio (original assets, familiar style)

- **Default textures:** `hearth_texgen` procedurally generates an original 16×16 pixel-art resource pack at build time in the familiar style: limited palettes, noise-dithered stone, grass-top tinted via colormap, log rings, leaf cutouts, ore specks in characteristic colors, animated water/flowing water strips. Also item icons, GUI panels, font, mob skins (box-UV layouts), particle sprites, sun/moon/phase textures, and grass/foliage colormaps. Make them look good — iterate on palettes until they read clearly at a glance.
- **Resource-pack compatibility:** the loader accepts resource packs in the standard vanilla folder layout (`assets/<namespace>/textures/block/*.png`, `models/block/*.json`, `blockstates/*.json`, `.mcmeta` animations, `sounds.json`, lang files) with a mapping table from vanilla resource names to `hearth:` ids, so the user can drop in any compatible pack they own and get the look they want. Support pack stacking/ordering in a Resource Packs screen and F3+T hot reload. Support 16× through 128× resolutions and optional PBR maps (normal/specular in the common LabPBR convention) used by the shader pipeline when present.
- **Audio:** original procedurally synthesized or CC0-licensed sounds (document sources in `ASSETS_LICENSES.md`) for footsteps per material, block break/place, water, rain, thunder, wind, cave ambience, mob sounds, UI clicks; calm ambient music tracks optional. Positional audio with occlusion-lite (muffle through solid blocks).

---

## 11. UI & HUD

Faithful layout and behavior of: title screen (with a slowly panning panorama rendered from a real generated world), Singleplayer world list (create/edit/delete/rename/backup), Create World screen (name, game mode, difficulty, seed, Planet Size with the Recommended preset pre-selected and travel-time estimates, vertical scale, feature rarity, land fraction, spawn climate, game rules, and a **planet preview**: a rotatable globe of the generated planet, colored by biome, that updates when the seed or options change), world map screen (M) with flat and globe views of explored areas, pause menu, options screens (§4), inventory (2×2 crafting, armor slots, offhand), creative inventory with tabs & search, crafting table, furnace, chest, recipe book, death screen, chat, HUD (hotbar, hearts with shake at low health, hunger with saturation shimmer, armor bar, air bubbles, crosshair, attack indicator, item name tooltip on switch, boss bar not needed, subtitles). Inventory interactions must match exactly: click, right-click split, shift-click quick move, drag-distribute (left and right drag), double-click collect, number-key swap, Q/Ctrl+Q drop, middle-click clone in creative. GUI scale handling identical in spirit.

---

## 12. Modding & extensibility (first-class)

1. **Data packs:** blocks, items, recipes, loot tables, tags, biomes, worldgen features (trees, ores, vegetation), mob spawn rules and mob stats are JSON/RON files under `data/<namespace>/…`, loaded in pack order with overrides. The base game ships as the `hearth` data pack.
2. **Resource packs:** as in §10.
3. **Script/code mods:** WASM components loaded from `mods/*.wasm` via `wasmtime`, sandboxed, with a WIT-defined, versioned API: register blocks/items/entities/recipes/commands; hook events (tick, block place/break, entity spawn/damage/death, player join, chunk generated, worldgen feature placement); query/modify world; custom block behaviors; UI screens via a simple retained-mode widget API. Capability-based permissions. Clear error messages; one broken mod never crashes the game.
4. **Rust-native extension:** all systems register through `hearth_modapi` traits so future features (new dimensions, redstone-like logic, networking) slot in without touching core crates.
5. Deliver an `examples/` folder with: a data-pack example (new ore + tool tier), a resource-pack example, and a WASM mod example (new block with custom behavior + command) with build instructions. Write `MODDING.md` documenting everything.
6. Registry ids are namespaced strings mapped to dense numeric ids at load time; saves store the id→name mapping so adding/removing mods never corrupts worlds (unknown blocks become a visible placeholder block and are preserved).

---

## 13. Performance engineering checklist

Apply all of these and verify with benchmarks:
- Fixed 20 TPS simulation on its own thread; rendering fully decoupled and interpolated; input sampled as late as possible before rendering for low latency.
- No heap allocation in steady-state frame/tick loops (use arenas, pools, `SmallVec`, reused buffers). Add a debug-mode allocation counter to assert this in benchmarks.
- Data-oriented layouts (SoA where hot), cache-friendly cube iteration, SIMD noise, bit-packed palettes.
- Job system with priorities and cancellation; work stealing; bounded queues; main-thread budget per frame for uploads/integration.
- GPU-driven culling, indirect draws, large shared buffers, persistent staging ring, minimal pipeline/bind-group switches (bindless-style texture arrays).
- Pipeline caches warmed at startup to avoid shader-compile hitches; loading screen compiles all pipelines.
- Region-file IO off-thread, zstd with dictionary for chunk data; LOD cache with lz4.
- Memory budgets (RAM and VRAM) with LRU eviction for chunks, meshes and LOD tiles; visible counters in F3.
- `[profile.release]` with `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`; a `dist` profile; optional PGO notes in `BUILDING.md`.
- Low-end fallback path: if compute/indirect-count features are missing, degrade gracefully (CPU culling, no volumetrics) rather than failing.

---

## 14. Milestones & acceptance criteria

Work through these in order. After each milestone: format, lint, test, update `PROGRESS.md`, commit. Mark each item `[x]` only when verified.

**M0 — Foundation:** workspace, CI script (`scripts/check.sh` running fmt/clippy/test), `PLAN.md`, `ARCHITECTURE.md`, `DECISIONS.md`, `PROGRESS.md`. Window opens, clear color, settings file loads/saves, input action map with rebinding logic unit-tested.
**M1 — Voxel core (cubic chunks):** registries, block states, palette cubes, cube map keyed by `(cx, cy, cz)`, column heightmaps, coordinate types with i32 Y everywhere, raycast; unit tests for palette round-trips, coordinate conversions (including negative and very large Y), raycasting across cube boundaries vertically.
**M2 — Worldgen macro & terrain:** plates, climate, erosion tiles, drainage/rivers/lakes, biomes, vertical profile, density terrain, caves (normal + rare giant caverns), aquifers, ores, trees. Tool `tools/bench worldmap --seed N --planet standard` writes whole-planet PNG maps (equirectangular and Mercator: biome, climate class, shaded relief, bathymetry with shelf/slope/abyss/trenches, rivers, plates, currents) plus `worldmap --slice` side-profile cross-sections, so you can inspect continents, coastlines, mountain ranges and ocean depth. Tests: determinism (same seed ⇒ identical cubes across threads and in any generation order); generation order never changes the final blocks of any cube (including decorations that cross cube borders); surface sampler agreement with full generation (≤ 1 block height error for ≥ 99% of columns); rivers never flow uphill; spawn is on temperate land; the wrap seam is invisible (a cube at X = C − 16 and its neighbour at X = 0 mesh, light and collide exactly like any other pair; walking and boating across the seam and over a pole works); the Mercator mapping round-trips; climate statistics over many seeds reproduce Earth's patterns (hot deserts concentrated at 15–35° latitude on western/interior continent sides and in rain shadows, rainforest within ~10° of the equator, boreal forest at 50–70°, ice caps poleward of ~70°, wetter windward coasts than leeward interiors); statistical checks on a large area that the hypsometry matches §6.3 (most land below Y 120, typical abyssal floor Y −800 to −1,100, great peaks and trenches present but rare at the default rarity) and that giant caverns occur at the intended rarity; empty/solid cube fast paths are hit for the vast majority of cubes far from the surface.
**M3 — Rendering near-field:** texture generation, atlas/array with mips, greedy meshing, smooth lighting/AO, GPU-driven culling, translucent sorting. Offscreen render mode `hearth --screenshot <spec>` renders fixed camera shots to PNG headlessly (use a software adapter if no GPU is present) so you can inspect output.
**M4 — Player & interaction:** physics, collision, movement constants, block breaking/placing, inventory model, hotbar, HUD, crafting/smelting, containers, saving/loading worlds.
**M5 — Light engine & fluids:** cubic-chunk sky light seeded from heightmaps, block light BFS across cube borders, incremental updates, underwater light attenuation, water flow, waterlogging, random ticks. Tests for light correctness on scripted scenarios, including a cave opened to the sky from below and a cube loaded before the cubes above it.
**M6 — Sky & atmosphere:** atmosphere LUTs, sun/moon/stars, day-night cycle, tonemapping, auto-exposure, realistic darkness, fog/aerial perspective.
**M7 — Water rendering:** surface, reflections, refraction, absorption, foam, underwater fog/caustics/god rays/Snell's window.
**M8 — LOD system:** quadtree, fast surface generator, LOD cache, meshing, seamless handoff, TAA option. Screenshot test from a mountain peak showing the horizon; benchmark fly-through.
**M9 — Entities & AI:** ECS integration, models/animation, pathfinding, the 8 mob types with full behaviors, spawning rules, combat, drops, taming.
**M10 — Weather & polish effects:** weather cells, rain/snow/thunder, wet surfaces, wind sway, clouds (blocky + volumetric), shadows, bloom, particles.
**M11 — UI completeness & audio:** every screen in §11 (including the planet preview globe and the world map), full options screens in §4, all key-binding features, controller, sounds and ambience, subtitles, localization plumbing.
**M12 — Modding:** data-pack loading/overrides, resource-pack loader with vanilla-layout mapping and hot reload, WASM API + three examples, `MODDING.md`.
**M13 — Optimization pass & verification:** run all benchmarks, profile with tracy, fix top hotspots, meet §8.4 targets where hardware allows (record actual numbers and hardware in `BENCHMARKS.md`), verify zero steady-state allocations, test on the software adapter for correctness.
**M14 — Final QA:** fresh clone → `cargo build --release` works; README with controls and features; `BUILDING.md`; `MODDING.md`; `ASSETS_LICENSES.md`; all tests pass; no clippy warnings; no `todo!`/`unimplemented!` (`grep` to verify); a scripted 10-minute automated soak test (bot player walking, mining, placing, time-lapse day/night and weather cycles) runs without panics or leaks.

**Global cohesion check (do this at M13 and M14):** play-through the scripted bot plus screenshot suite at dawn, noon, sunset, full-moon night, new-moon night, underwater on the shelf, at the shelf edge looking down the slope, on the abyssal floor, in a giant cavern, in a cave with torches, in a thunderstorm, in snowfall, above the clouds on a great summit, from that summit at LOD 1024, in a desert dune sea, in equatorial rainforest, on the boreal shield among lakes, on a cold foggy west coast, at the pole, and at the same global time on opposite sides of the planet (one in daylight, one at night). Review every screenshot. Anything that looks inconsistent (LOD seams, color mismatch between near/far terrain, fog popping, water edges, over-bright nights, texture style clashes) is a bug — fix it.

---

## 15. Resume Protocol (for long runs)

This project is larger than one context window. Protect continuity:
- Keep `PROGRESS.md` current at all times: completed milestones, the item in progress, exact next 3–5 steps, known bugs, deferred items, and benchmark numbers.
- Before your context fills, stop starting new subsystems, get the tree compiling and green, commit, and write a precise handoff in `PROGRESS.md`.
- When starting (or restarted with "continue"), first read `PROGRESS.md`, `PLAN.md`, `DECISIONS.md` and `git log --oneline -30`, run `scripts/check.sh`, then continue from the next unchecked item. Never redo completed work or rewrite working subsystems without a recorded reason.

---

## 16. Quality bar

The result should feel like one coherent, intentional game: the pixel art, the lighting, the sky, the water, the far-off hazy continents and the weather should all belong to the same world. Favor a smaller number of flawless features over many rough ones — but the feature list above is the target, and "done" means all of it works together, runs fast, and is easy to extend.

Begin now with M0.