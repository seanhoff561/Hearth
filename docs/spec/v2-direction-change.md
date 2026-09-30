# DIRECTION CHANGE v2 — "Earth, Transposed"
## A realistic Earth-simulation survival game on the existing `hearth` engine

You are the same autonomous engine programmer who has been building `hearth` from the original build prompt (`PROMPT.md` / `voxel_game_build_prompt.md`, called **v1** below). The project is changing direction. This document is **v2**. Read all of it before changing any code.

v1 described a trimmed Minecraft-like game on a high-performance engine. v2 keeps that engine and turns the game into something different: **the Earth, with its geology, climate, seasons, plants, animals, human physiology and the long story of human technology, transposed into a randomly generated cube-grid world, as realistically as the grid allows.** The player begins as a lone human in a loincloth in a wild world, and has to figure out, as our ancestors did, how to turn stone, wood, fibre, fire, clay and metal into a life.

This is a long, multi-session project. The Resume Protocol from v1 (§15 there) still applies, and §20 here extends it.

---

## 0. How to use this document

### 0.1 Precedence
- v2 **overrides** v1 wherever they conflict. Where v2 is silent, v1 still applies (engine, rendering, LOD, cubic chunks, planet generation, settings/controls, performance rules, modding infrastructure, clean-room rules, "always green", no stubs).
- The v1 ground rules stay non-negotiable: clean-room implementation, no Mojang names/assets, autonomous work, no `todo!()`/`unimplemented!()`, always compiling and green, stable Rust and current crates, performance as a feature.
- The user expects to keep changing this design. **Malleability is a first-class requirement** (§3). When you have to choose between a clever hard-coded solution and a data-driven one that's slightly slower to build, choose data-driven.

### 0.2 First actions (before touching gameplay code)
1. Read `PROGRESS.md`, `PLAN.md`, `DECISIONS.md` and `git log --oneline -40`, then run `scripts/check.sh`.
2. If you're in the middle of a v1 **engine** milestone (renderer, LOD, cubic chunks, worldgen macro, lighting, weather, saves, UI framework, audio engine, modding infrastructure), finish it to a green, committed state first. Don't abandon engine work half-done.
3. If you're in the middle of v1 **content** work (MC-style mobs, crafting grid, ore tiers, MC items), stop it at the nearest green point and commit.
4. Write `MIGRATION.md`: a table of every v1 milestone and subsystem marked **Keep / Modify / Replace / Drop**, per §2, with a short note on what changes. Then rewrite `PLAN.md` for the v2 milestones (§19), keeping completed v1 engine milestones as done.
5. Commit, then proceed with the v2 milestones in order.

### 0.3 How much liberty to take
The user wants realism above all, and explicitly invites you to take liberties where the design is under-specified. The test for any liberty: **does it make the world behave more like the real Earth, in a way the player can perceive and learn, without making the game unplayable or the code unmaintainable?** Record significant liberties in `DECISIONS.md` with a one-line rationale so they can be revisited.

---

## 1. Vision & design pillars

**The essence:** take the systems of the real Earth (physical, biological, human) and re-express each one as a game system, faithful to how it really works, sized to the cube grid and to human play-time, and connected to the others the way they're connected in reality.

Pillars, in priority order when they conflict:

1. **Physical causality.** Things happen for the reasons they happen on Earth. Deserts are where the air sinks; copper is where hydrothermal fluids deposited it; wolves are where the deer are; bread exists because someone domesticated wheat, ground it, and built an oven. Never place content randomly when a cause can place it.
2. **Effort parity.** Making something in the game should cost effort proportional to what it cost real people: in materials, steps, tools, knowledge, time and risk, compressed by the game's time scales (§4.2) but never trivialized. A bronze axe is a triumph. An iron bloom is an expedition's worth of work.
3. **Legible realism.** Realism must be *discoverable and learnable*, never an invisible trap. Every system gives the player honest feedback (sensations, visible states, journal notes, animal behavior) so a curious player can figure things out the way people did.
4. **Emergent ecosystems.** Plants, animals, weather, fire, seasons and the player form food webs and feedback loops. The player is a participant in an ecosystem, not the center of a spawn system.
5. **Knowledge, not experience points.** Progress is what the player (and, later, simulated humans) *knows how to do* and *has the means to do*. There are no XP bars, no level-ups and no recipe book to unlock.
6. **Cube-grid honesty.** 1 block = 1 metre. Realism lives inside the grid: real sizes, masses, heights and distances in metres, with sub-block models where a metre is too coarse (branches, stones, tools, fences), but the world stays a voxel world.
7. **Cohesion.** Every new system must touch the others (§18 interaction matrix). A feature that doesn't interact with anything is a sign it's in the wrong shape.
8. **Malleable & future-proof.** Content is data. Systems are general. Future eras and simulated human societies (§17) must fit without rewrites.

---

## 2. What stays, what changes, what goes

| Area | Decision | Notes |
|---|---|---|
| Engine: wgpu renderer, GPU-driven culling, meshing, LOD (Distant-Horizons-style), cubic chunks, floating origin, wrap-aware coords | **Keep** | Extend for new foliage, animals, seasons (§15). |
| Planet generation: sphere simulation, Mercator wrap, pole crossing, plates, erosion, drainage, climate model, biomes-by-climate | **Keep & extend** | Add geology provinces, stratigraphy, soils, groundwater, reefs, seasonal climate (§5). |
| Atmosphere, water rendering, realistic darkness, weather cells, clouds, fog | **Keep & extend** | Seasonal sun path & day length, snow cover, phenology tints, wildfire smoke. |
| Settings, controls screen, rebinding, display modes, unlocked FPS, F3 | **Keep** | Update default keybinds for new interactions (§10.6). |
| Saves, region files, registries, data/resource packs, WASM modding | **Keep & extend** | New content schema, save versioning & migrations (§3). |
| Time: 20-minute day, 8 moon phases, fixed-latitude sun | **Modify** | Configurable day length, calendar, seasons, axial tilt, time zones already in v1 (§4). |
| Block set (MC-style stones, ores, woods) | **Replace** | Geology-based rocks, minerals, soils; real tree species (§5, §6, §13). |
| Ores: coal/iron/gold/diamond with MC tiers | **Replace** | Realistic deposit geology and metallurgy (§13). No diamond tools or armor. |
| Tools/armor tiers (wood/stone/iron/gold/diamond) | **Drop** | Replaced by the knowledge-based technology graph (§12). |
| Crafting table grid, furnace, recipe book | **Drop** | Replaced by in-world process crafting, workstations as physical structures, and the journal (§11, §12). |
| XP, enchanting | **Drop** (already out) | — |
| Mobs: pig/cow/sheep/chicken/wolf, zombie/skeleton/spider | **Replace** | Wild fauna by ecosystem; domesticated forms come from real domestication of wild ancestors; monsters replaced by real predators (§7). |
| Darkness-based hostile spawning | **Drop** | Animals come from population simulation (§7.4). |
| Hunger bar, heart health, instant regeneration | **Replace** | Realistic physiology (§9). |
| 36-slot inventory + hotbar | **Replace** | Hands, body slots, clothing and containers with weight & volume (§10). |
| Beds that skip the night | **Replace** | Realistic sleep with accelerated time (§9.5). |
| Player model & skin | **Replace** | Character creator, realistic-proportioned blocky body, clothing layers (§9.1). |
| Structures | **Still excluded**, except hominin nests and tool-scatter sites (§8) | Settlement/structure *systems* are designed now for the future (§17). |
| Lava | **Still excluded** as a fluid | Volcanic *landforms and rocks* now exist (obsidian, pumice, basalt, sulfur). |
| Redstone, lapis (as MC mechanics) | **Still excluded** | Azurite, malachite and ochre supply real pigments instead. |

Old worlds from v1 are **incompatible**: bump the save format version, refuse to load v1 worlds with a clear message, and from now on build real save migrations (§3.5).

---

## 3. Architecture for malleability

### 3.1 Everything is data
All content lives in versioned, schema-validated data files under `data/hearth/` (RON preferred for hand-authoring, JSON accepted), organized by domain:

```
data/hearth/
  units.ron              base units & conversions
  time.ron               day length, calendar, compression factors, realism presets
  materials/             physical materials (density, strength, fuel value, melting point, thermal props, workability…)
  geology/               rock types, minerals, ore deposit models, rock provinces, soils, clays
  flora/                 plant species (trees, shrubs, herbs, grasses, crops, fungi, algae) + growth forms
  fauna/                 animal species, body plans, behaviors, diets, habitats, realms
  hominins/              early-human species, group behaviors, tool traditions
  items/                 item definitions (mostly generated from materials × forms where possible)
  processes/             transformations: actions, inputs, tools, conditions, durations, outputs, quality rules
  knowledge/             technology/knowledge graph nodes and discovery triggers
  workstations/          physical structures that enable processes (fire pit, kiln, bloomery, loom…)
  construction/          building pieces, joinery, structural properties
  body/                  human physiology parameters, injuries, illnesses, nutrition
  clothing/              garments, insulation, pockets/attachment points
  ecosystems/            food-web defaults, succession sequences, fire regimes
  eras/                  era definitions (only one enabled now; §17)
  balance/               global multipliers and realism presets (§3.4)
```

- **Generate, don't enumerate.** Items are composed wherever possible: `form × material` ("knife" × "flint", "plank" × "oak", "ingot" × "bronze") with properties derived from the material. A new wood species or rock then automatically gets its logs, planks, billets, tools and textures without new code.
- Rust code implements **mechanisms** (heat transfer, structural load, metabolism, AI behaviors, growth); data supplies **parameters and content**.
- Every entry carries optional `notes` and `realism_source` fields (e.g., "approximate adult mass from general zoological references; review"). Use well-established approximate real-world values and mark uncertain ones as `uncertain: true` so a later realism review can find them.

### 3.2 Validation & tooling
- `hearth content lint` loads all data, validates schemas and cross-references (every id exists, every process has reachable inputs, every knowledge node is reachable, every species has a habitat that exists somewhere, every food web has producers), checks unit sanity, and reports clearly with file/line. Run it in `scripts/check.sh`.
- `hearth content graph` exports the knowledge graph, the process graph (material flows) and the food webs to Graphviz/SVG and to an HTML page in `docs/generated/`.
- Hot-reload data at runtime (F3+T) wherever the system can; systems that can't hot-reload say so in the log.

### 3.3 Units
Use SI internally everywhere: metres, kilograms, seconds (real and game), joules/kilocalories for food energy, litres for fluids, °C for temperature, Pa/MPa for strengths. Convert only at the UI. 1 block = 1 m³.

### 3.4 Balance layer & realism presets
All tuning goes through `balance/`: global multipliers (hunger rate, thirst rate, injury severity, predator aggression, process durations, growth rates, yields) and **Realism presets**:
- **Authentic** (default): real-world-proportioned values within the time compression of §4.2.
- **Hardy**: the same systems, more forgiving (slower needs, faster healing, fewer deadly encounters). Good for learning and testing.
- **Custom**: every multiplier exposed as a slider in world settings.
Presets are data, so the user can tune without code changes.

### 3.5 Save versioning & migrations
Content will change constantly. Store the content registry mapping in each save, version every persisted struct, and write a migration step whenever a persisted format changes. Unknown or removed content degrades gracefully (a visible "unknown" placeholder that preserves data) instead of corrupting worlds. Add tests that load saves from previous format versions.

### 3.6 Design documentation
Maintain `docs/design/` with one short document per system (purpose, model, parameters, interactions, known simplifications, future extensions). Update it in the same commit as the system. This is how the user and future sessions will steer the project.

### 3.7 Simulation layers
Everything that lives or changes over time runs at one of three fidelities, chosen by distance from the player and importance:
1. **Full** (near the player): individual agents, block-level plants, real physics.
2. **Reduced** (loaded but distant): coarse updates at low frequency (herds as group entities, plants advancing by elapsed time when their cube loads).
3. **Abstract** (anywhere on the planet): statistical state on ecological cells (§7.4), advanced in large time steps.
Transitions between layers must conserve state: animals folded into a population and back out come back plausibly; a forest the player cut down stays cut down and regrows at the right rate.

---

## 4. Time, calendar & seasons

### 4.1 Calendar
- Configurable **day length** (default 48 real minutes; options 20–120) and **days per season** (default 8; options 3–91, where 91 is a real year). Four seasons per year, with the year length derived from the two.
- **Axial tilt** (default 23.44°) drives seasons exactly as on Earth: solar declination through the year, day length by latitude (long summer days and polar day at high latitudes, polar night in winter, near-constant 12-hour days at the equator), the sun's noon height, and opposite seasons in the two hemispheres.
- Tropical regions get **wet and dry seasons** from the seasonal shift of the equatorial rain belt; monsoon coasts get a strong wet season; mid-latitudes get four temperate seasons; high latitudes get long winters.
- The moon has a realistic synodic cycle (scaled to the calendar) with phases and **tides** (stretch goal: sea level offset of a few blocks in coastal cells, spring and neap tides, intertidal zones).
- The world creation screen lets the player choose the **starting season** (default: spring in their hemisphere).

### 4.2 Two time scales (this is how realism fits into play time)
A game day is short, but a game year is much shorter still relative to reality. So real durations map onto the game in two ways:
- **Body & action time (day-scale, 1 game day = 1 real day):** eating, drinking, sleeping, thirst and hunger, body temperature, short injuries and illnesses, fresh-food spoilage, fire burning, most crafting actions (compressed by the day ratio: a 15-minute real task at the default 48-minute day takes 30 real seconds).
- **Life-cycle & calendar time (year-scale, compressed by real_year / game_year):** plant growth and seasons, crop maturation, animal gestation, maturation and migration, tree growth, soil recovery, long processes such as tanning, retting flax, fermentation, seasoning timber, and slow healing (a broken bone's weeks).
Every process in data declares which scale its duration uses. Put the conversion in one place (`time.ron` + one Rust module) so the user can retune it.

### 4.3 Seasonal world
- **Phenology:** deciduous trees leaf out, flower, fruit, change color and drop leaves (leaf blocks become bare-branch state, with leaf litter accumulating on the ground and decaying); conifers stay green (except larch); grasses green up and cure to gold; wildflowers bloom in waves; fruits, nuts, seeds and tubers are available in their real seasons.
- **Snow & ice:** snow cover accumulates and melts by temperature (snow layers), lakes and rivers freeze and thaw (ice thickness grows over time and can be walked on only when thick enough), permafrost ground in the cold climates.
- **Animals:** migrations (caribou, wildebeest, birds, salmon runs), hibernation (bears, marmots), rut and breeding seasons, birthing seasons with young animals, winter coats (color change for arctic hare/fox, ptarmigan).
- **Weather:** the v1 weather system becomes seasonal: wet seasons, winter storms, summer thunderstorms, dry-season wildfire risk, spring floods from snowmelt raising rivers.
- **Rendering:** grass/foliage colormaps shift through the seasons, sun path and day length change, LOD terrain and distant forests change with the season too (bare deciduous forests read grey-brown from a distance in winter; snow-covered LOD tiles).

---

## 5. Planet, geology & terrain realism (extending v1 §6)

v1's planet model (sphere, plates, erosion, drainage, climate) stays. Replace its generic stone/deepslate/ores with real geology.

### 5.1 Geological provinces
From the tectonic history already simulated, assign each region a geological province that determines its rocks, structure and deposits:
- **Cratons/shields** (old interiors): granite, gneiss, schist, greenstone; banded iron formations; rare kimberlite pipes; glacially scoured in cold regions.
- **Fold mountain belts** (collisions): folded sedimentary sequences and metamorphic rocks (slate, schist, marble, quartzite), granite intrusions with associated vein deposits.
- **Volcanic arcs** (subduction): andesite, basalt, rhyolite, tuff, pumice, **obsidian** flows, volcanic cones and calderas (crater lakes), fumaroles with **sulfur**, hot springs, porphyry copper systems.
- **Hotspots and rifts:** basalt (columnar jointing in cliffs), flood basalt plateaus, rift lakes, native copper in basalt (rare).
- **Sedimentary basins & passive margins:** sandstone, shale, mudstone, **limestone**, **chalk** (with **flint nodules**), conglomerate, **coal seams**, evaporites (**rock salt**, gypsum) in arid basins.
- **Ocean floor:** basalt under sediment; coral reefs and carbonate platforms in warm shallows.
Rocks form **visible stratigraphy**: horizontal or folded layers exposed in cliffs, canyons, river cuts and sea cliffs, dipping under the surface realistically, so a player can "read" the rocks and follow a coal seam or a limestone bed.

### 5.2 Depth & the deep crust
The v1 "deepslate below Y −512" rule is replaced: under the local province sequence lies crystalline **basement** (granite/gneiss). Rock temperature rises with depth (geothermal gradient scaled with vertical scale), so deep mines are hot, which matters for the player's body (§9.4). Groundwater exists at the water table; mines below it flood unless drained (stretch goal until pumps exist in the tech graph).

### 5.3 Soils
Soils come from climate × parent rock × vegetation × drainage × slope, with realistic profiles (organic litter → topsoil → subsoil → weathered rock → bedrock), a few blocks deep in lowlands and absent on steep rock:
- chernozem (steppe; deep, black, fertile), brown forest soils (temperate), podzol (boreal; acidic), laterite/red tropical soils (weathered, nutrient-poor), **alluvium** (floodplains; very fertile, renewed by floods), loess (windblown silt), **peat** (bogs; fuel), **clays** (earthenware clay in river banks and lake beds, kaolin in weathered granite, fire clay under coal seams), sand and gravel (rivers, beaches, deserts, glacial outwash), desert pavement, volcanic ash soils (fertile), permafrost.
Each soil has fertility (simplified N/P/K + organic matter), drainage, pH and workability, used by plants (§6) and farming (§12).

### 5.4 Hydrology realism
- **Groundwater table** by climate and terrain; **springs** where it meets the surface (clean water; oases in deserts); wells can reach it.
- Rivers and lakes from v1, now with **seasonal levels** (spring floods, dry-season lows, some rivers dry up in arid seasons), freezing, and floodplains that flood.
- **Water movement:** oceans, rivers and lakes are hydrologically sustained reservoirs; water the player moves (containers, channels, dams) is **finite** and conserved, with levels equalizing and flowing downhill. Irrigation channels actually carry water from a source to fields (§12).
- **Water quality:** springs and fast streams are mostly safe, still and warm water is risky, seawater is salty; represented per water body and per block for player-moved water (§9.3).

### 5.5 Coasts & seas
Add **coral reefs** in warm, clear, shallow seas (fringing reefs, barrier reefs with lagoons, atolls on sunken hotspot islands), kelp forests in cool coastal waters, mudflats and salt marsh on sheltered low coasts, mangrove coasts in the tropics (mangrove trees, §6), rocky intertidal zones, sea ice in polar seas.

### 5.6 Terrain within the grid
Keep full-cube terrain, but add realism through natural surface detail: layer blocks for snow, leaf litter, sediment and ash; loose rocks, pebbles, cobbles and boulders as small models or partial blocks; exposed roots on banks; talus/scree with angle-of-repose slopes; rock outcrops that follow the stratigraphy; animal trails worn into vegetation where animals travel often; wallows, burrows and dens.

### 5.7 Fire
Wildfire is part of the ecosystem: ignition by lightning (and the player), spread by fuel load, moisture, wind and slope, dry-season risk by climate, smoke plumes visible from far away (rendered in LOD), burned ground and snags afterwards, and ecological succession on the burned area (§6.6).

---

## 6. Botany: real plants, real trees, living vegetation

### 6.1 Plant model
Every plant species in `flora/` defines: growth form (tree, shrub, herb, grass, vine, succulent, fern, moss, fungus, aquatic, alga); climate envelope (temperature, precipitation, seasonality tolerance); soil preferences; light tolerance (shade-tolerant understory vs pioneer); lifecycle (annual, biennial, perennial; phenology calendar); reproduction (seed production, dispersal mode: wind, animal, water, gravity); growth rate; maximum size; yields (wood, bark, fibre, fruit, nut, seed, root, leaf, resin, sap, medicine, toxin); edibility and nutrition per part and per season; toxicity; flammability; and texture/color parameters for procedural textures.

### 6.2 Trees: real species, real shapes
- Trees grow **procedurally** by species (space-colonization or L-system on the voxel grid, with species-specific parameters: apical dominance, branching angles, crown shape, trunk taper, buttresses, drooping, whorled branches for conifers), producing recognizable silhouettes: the broad spreading oak, the slender white birch, the narrow spire of spruce and fir, the umbrella of acacia, the tall straight pine with a high crown, the drooping willow, the column of cypress, the fan of palms, the giant sequoia.
- **Real heights and girths** in metres: saplings to old giants, e.g., mature oaks 20–30 m, spruces 30–50 m, rainforest emergents 50–70 m, giant sequoias/redwoods up to ~90 m (rare old-growth giants are landmarks).
- **Trunks and branches in the grid:** full log blocks for thick trunks; tapered sub-block branch pieces (e.g., 12/8/4/2-pixel-thick round branch models) for limbs and twigs; root flares and exposed roots. Thick branches are solid and climbable; thin ones are not.
- **Passable foliage:** leaves are **not solid**. Moving through foliage and bushes slows the player, makes noise, obscures vision and hides animals (and the player). Leaves render with cutout and wind sway; dense canopies cast realistic shade (sky-light reduction, not full blocking).
- **Tree lifecycle:** seed → seedling → sapling → mature → old growth → senescence → snag → fallen log → decay into soil. Trees grow in discrete stages near the player (rebuilding their blocks deterministically from the growth model) and advance by elapsed calendar time when a cube reloads.
- **Felling:** cutting a tree through with an axe (time depends on tool, trunk diameter and wood hardness) makes it **fall** as a physical event in the direction of the cut, crashing through foliage and damaging what it lands on. The fallen trunk must then be limbed, bucked into log sections (heavy: a 1 m section of a 40 cm oak weighs over 100 kg) and processed (split, hewn, sawn) or dragged/rolled (§10.4).

### 6.3 Wood with real properties
Each species' wood has: density, hardness, strength (bending/compression), elasticity (bow suitability), workability, splitting behavior, rot/insect resistance, fuel value and burn characteristics (charcoal quality, smoke, sparks), resin/pitch content, bark uses (tannin, fibre, roofing, birch-bark containers and tar), and appearance (bark texture, sapwood/heartwood colors, grain). These drive gameplay: yew, ash, hickory, osage-orange-like woods and elm make better bows; oak is strong and durable for frames; cedar and cypress resist rot; pine gives resin, pitch and kindling; birch gives bark and tar; willow and hazel make wattle and baskets; bamboo is strong, light and fast-growing; balsa-like woods for floats.

### 6.4 The plant kingdom beyond trees
Shrubs and bushes (passable, slowing), berry bushes, grasses (many heights: tall grass hides a crouching player and small animals), reeds, rushes, sedges, cattails and papyrus, ferns, mosses and lichens, vines and lianas (climbable in rainforest), cacti and succulents, aquatic plants (water lilies, pondweeds, seagrass, kelp), seaweeds, fungi (mushrooms and bracket fungi, including deadly look-alikes; tinder fungus), and crop ancestors. See **Appendix A** for the species list.

### 6.5 Edible, useful, medicinal and poisonous plants
Plants are food, fibre, medicine, poison, dye, fuel and tools. Toxic and edible look-alikes exist; the player must learn identification (journal knowledge, §12.3). Some foods need processing (acorns need leaching of tannins; some tubers need cooking; bitter wild plants can be toxic raw). Medicinal plants have modest realistic effects (willow bark for pain, antiseptic resins, soothing plants for burns). Keep effects plausible and understated.

### 6.6 Vegetation dynamics
- **Succession:** disturbed ground (fire, clearing, landslide, abandoned field) goes through realistic stages: pioneer herbs and grasses → shrubs → pioneer trees (birch, aspen, pine) → climax forest by climate. The player's clearings slowly regrow.
- **Competition & distribution:** seedlings establish only where light, soil, moisture and climate fit; shade-tolerant species take over under canopies; forest edges and clearings form naturally; grazing pressure from herbivores keeps grasslands open; beavers flood valleys and create wetlands.
- **Abstract vegetation state** per ecological cell (§7.4): biomass, canopy cover, species composition, fire fuel, forage availability. It drives animal carrying capacity and is what the LOD shows at a distance.

### 6.7 Rendering vegetation
Species-specific procedural textures (bark, leaves, needles, flowers, fruits), seasonal states, wind animation by species (stiff conifers, fluttering aspen), GPU-instanced grasses and small plants for density at low cost, and LOD canopy colors per species mix and season (§15).

---

## 7. Fauna & ecosystems

### 7.1 Principles
- Every animal is a **real species** (or a clearly labeled composite where a random planet needs an ecological role filled) with real size, speed, senses, diet, social structure, activity cycle, habitat and seasonal behavior.
- There are **no monsters.** Danger comes from real predators, dangerous herbivores, venomous animals, and the environment.
- Animals exist because the **ecosystem supports them** (§7.4), not because a spawn rule rolled.

### 7.2 Body plans, models & animation (making a large bestiary feasible)
- Define a small set of **body plans** with shared skeletons: quadruped (with variants: ungulate, carnivore, rodent, bear, elephant, giraffe, hippo), primate, bird (perching, ground, waterfowl, raptor, seabird, penguin), fish (fusiform, flat, eel), marine mammal (seal, cetacean), reptile (lizard, crocodilian, turtle), snake, amphibian, arthropod (crab, insect for ambient only).
- Models are blocky cuboid rigs in the game's style, but with **realistic proportions and real dimensions**: a moose is 2 m at the shoulder, an elephant 3+ m, a blue whale 25+ m.
- **Procedural animation** from gait data: walk/trot/canter/gallop gaits for quadrupeds, foot IK on uneven terrain, head look-at, breathing, tail and ear motion, swimming, flying and gliding, plus a small set of authored clips per body plan (eat, drink, attack, rear, lie down, sleep, groom). New species should need only data and a texture recipe.
- **Procedural textures** per species from a coat recipe (base colors, countershading, stripes, spots, rosettes, seasonal coat), generated by `hearth_texgen`.

### 7.3 Animal minds
- **Senses:** vision (field of view, acuity, night vision), hearing (noise from footsteps by surface, running, breaking branches, tools), **smell carried by the wind** (the v1 wind field): approaching from downwind matters. The player's scent, noise and visibility (crouching, moving through foliage, clothing color, light at night) are real stealth mechanics.
- **Needs & state:** hunger, thirst, fatigue, fear, reproductive state, age, health, injuries; memory of threats, food sites, water sites, territory, the den.
- **Behavior architecture:** utility AI selecting among behaviors (forage, graze, browse, hunt, stalk, ambush, flee, freeze, alarm-call, fight, defend young, drink, rest, sleep, migrate, patrol territory, mate, give birth, nurse, groom, play, scavenge, cache food, dig, build: nests, dens, beaver dams), with weights and parameters in data and behavior primitives in Rust. Pathfinding on the navigation grid from v1, extended for swimming, flying and climbing.
- **Social structures:** herds (with vigilance sharing, alarm calls and stampedes), packs (coordinated hunting with flanking and relays), prides, solitary territorial animals, flocks and schools (boids, GPU-friendly), pair bonds, mother-and-young groups, dominance hierarchies.
- **Activity cycles:** diurnal, nocturnal and crepuscular animals, so dawn and dusk are busy and night belongs to different animals.

### 7.4 Population & ecosystem simulation
- **Ecological cells:** the planet is divided into ecological cells (e.g., 256 m × 256 m, on the wrapped grid) holding, per species, a population density and age/sex structure, plus the vegetation state of §6.6 and water availability.
- **Dynamics:** in abstract time steps (e.g., once per game day per cell, spread across ticks), populations grow with food availability (carrying capacity from vegetation and prey), decline with predation, starvation, winter and hunting, disperse to neighboring cells, and migrate seasonally along real-world-like routes to water, grazing and breeding grounds. Predator-prey dynamics must be **stable over long runs** (no runaway explosions or total collapse without cause), achieved with realistic mechanisms (prey refuges, territorial limits on predators, density-dependent effects, alternative prey) rather than clamps alone.
- **Materialization:** near the player, cells spawn actual animals consistent with their density and structure (a herd of 23 caribou with calves in spring, a wolf pack of 6 holding a territory). When the player leaves, individuals fold back into the cell's numbers, including any that the player killed, wounded or tamed. Named/tamed/domesticated animals always persist as individuals.
- **Player impact is real:** over-hunting reduces local populations for seasons; clearing forest changes which species can live there; a camp with unsecured meat attracts scavengers and predators; fire changes habitats.
- **Biogeographic realms:** when the world is generated, group landmasses into faunal realms by continental isolation and climate (like Earth's Nearctic, Palearctic, Afrotropical, Indomalayan, Neotropical, Australasian). Each realm draws its fauna from matching real-world assemblages so that ecological roles are filled but continents feel different (e.g., one tropical continent has jaguars and tapirs; another has tigers and elephants). **Islands** get impoverished, endemic faunas: fewer mammals, more birds, sometimes flightless birds, rarely large predators. Hemisphere matters: polar bears only in the far north, penguins only in the southern oceans.

### 7.5 Predators (replacing zombies, skeletons and spiders)
Predators behave like real predators, which makes them more frightening, not less:
- Most predators avoid healthy adult humans most of the time. Attacks happen for real reasons: hunger in lean seasons, surprise at close range, defense of young, food or kills, cornering, the player being injured or small-appearing (crouching, alone, at night), habituation to human food, or territorial defense.
- **Attack styles by species:** stalking and ambush from cover (big cats), long pursuit and flanking by packs (wolves, wild dogs, hyenas), bluff charges and defensive mauling (bears, especially mothers with cubs; bears raiding food caches), ambush from water (crocodiles at drinking spots; sharks attracted by blood and splashing), venomous bites from snakes when stepped near or handled, and dangerous herbivores (moose, bison, hippos, elephants, buffalo, boar) that charge when threatened.
- **Counterplay is real:** fire deters most predators; standing tall, facing them and making noise deters many; running from a big cat triggers pursuit; bear behavior depends on the species; groups (later: companions, dogs) deter predators; meat storage (hung high, sealed in pots) matters; wounded predators may retreat; predators that were hurt learn to be wary of the player.
- Nights are dangerous because nocturnal predators are active and humans see poorly, not because monsters spawn in darkness.
- A **Predator Behavior** world setting (Authentic default / Wild (more aggressive) / Tranquil) scales aggression without changing mechanics.

### 7.6 Hunting, tracking, fishing, butchering
- **Tracking:** footprints in soft ground, mud and snow (decals that fade with time and weather), droppings, browse marks, blood trails from wounded animals, game trails, alarm calls revealing hidden predators.
- **Weapons and methods by technology** (§12): thrown stones, clubs, fire-hardened spears, stone-tipped spears, atlatl darts, bows, slings, snares and deadfall traps, pit traps, fishing by hand, spear, line and hook, nets, basket traps and fish weirs, harpoons for seals.
- **Realistic wounds on animals:** hit location and weapon matter; wounded animals flee and may need tracking; clean kills are rewarded.
- **Butchering** takes time and a cutting edge, yielding real quantities (meat by cut, fat, organs, hide, fur, bone, marrow, sinew, antler/horn, feathers, blood), scaled to the animal's real size. Quality depends on tools and skill. Carcasses attract scavengers (vultures circling are visible from far away) and spoil.

### 7.7 Domestication
Domesticated animals are not pre-made. The player domesticates **wild ancestors** through a realistic process: taming individuals (feeding, habituation, raising young from birth), keeping them (enclosures, food, water, shelter), and **selective breeding across generations** that shifts heritable traits (docility, size, yield, coat, milk, draft strength). Real pathways: wolf → dog, mouflon → sheep, bezoar ibex → goat, aurochs → cattle, wild boar → pig, red junglefowl → chicken, wild horse → horse, wild ass → donkey, guanaco → llama, wild camel → camel, wild water buffalo → water buffalo. Some animals can be tamed but never truly domesticated (zebras, big cats), as on Earth. This also brings back the v1 farm animals as outcomes rather than spawns.

### 7.8 Aquatic life
Freshwater fish by habitat (trout in cold streams, salmon with seasonal runs, pike, perch, carp, catfish), amphibians, beavers, otters, turtles; ocean life by zone: coastal fish, crabs and shellfish (gatherable at low tide), seals and sea lions hauled out on shores, seabird colonies, sea turtles, reef fish and reefs (§5.5), kelp forests with sea otters and urchins, schools of herring and sardines, tuna, dolphins, whales (migrating, audible underwater), sharks, squid, and a sparse, mysterious **deep sea** with bioluminescent creatures (real light sources in the abyssal darkness, rendered as emissive). Fish schools and plankton are instanced and cheap.

### 7.9 Ambient life
Birds (dawn chorus, flocks, migrations in V-formations, raptors circling, vultures over carcasses), insects (bees and wild hives with honey; fireflies on summer nights; butterflies; mosquitoes and biting flies in wetlands in warm seasons; termite mounds in savannas; locust swarms, rare), bats at dusk. Mostly GPU-instanced with simple behaviors, interacting where it matters (honey, mosquitoes as a nuisance and a mild health risk in the tropics, vultures revealing kills).

See **Appendix B** for the species list by ecosystem and priority tier.

---

## 8. Early hominins: *Australopithecus* (and the foundation for simulated humans)

### 8.1 What they are in the game
Sporadic small groups of *Australopithecus* live in suitable habitat around the world: tropical and subtropical **savanna–woodland mosaics, gallery forests along rivers, and lakeshores**, within reach of water, trees to sleep in and food. They are rare enough that meeting them is memorable. (Setting: *Hominin range* = All suitable habitat (default) / Single cradle region.) They're shown deliberately as a period liberty: the default era (§17.1) is a timeless wild Earth, and future eras will replace them with era-appropriate hominins and humans.

### 8.2 Behavior (grounded in what's known or reasonably inferred)
- Groups of roughly 5–25, bipedal walkers who also climb well; they forage for fruit, seeds, nuts, roots and tubers (with digging sticks), insects (termite fishing with twigs), eggs and small animals, and scavenge carcasses.
- **Tool use in the earliest known style:** using hammerstones and anvils to crack nuts and bones, striking stones against anvils to produce sharp flakes for cutting meat and plants, carrying good stones, and discarding tools at work sites. They do not make fire or complex tools.
- Sleep in **tree nests** at night; vigilant against predators; alarm calls; mob a threat together by shouting, brandishing sticks and throwing stones; flee to trees.
- Toward the player: wary and curious. They flee from fast approaches, may threaten when cornered or when young are near, and slowly **habituate** to a calm, patient player who keeps a respectful distance and doesn't hunt near them.
- They leave **traces:** tool scatters of flakes and hammerstones, cut-marked bones, abandoned nests, trails to water. Finding a scatter is a clue.

### 8.3 Learning from them
Watching hominins use a technique grants the player **observation insight** toward that knowledge (§12.3): hammer-and-anvil flaking, nut-cracking, termite fishing, digging-stick use, scavenging timing. Studying a tool scatter gives smaller insight. This is the first expression of a key future mechanic: **knowledge transfer between minds**.

### 8.4 The agent framework (build it general)
Implement hominins on a general **Agent** framework, not as special-case animals, because simulated humans (§17) will run on it:
- `Body` (the same physiology model as the player, §9, parameterized per species), `Mind` (needs, perception, memory, goals, utility/GOAP-style planner), `KnowledgeState` (which knowledge-graph nodes and skill levels this agent has; §12), `Inventory` (the same carrying model as the player, §10), `SocialGroup` (membership, roles, relationships, shared memory of territory and resources), `Culture` (shared behaviors and techniques, a placeholder that later carries traditions).
- Agents use the **same process system** as the player to make things: a hominin making a flake runs the same `processes/` data as a player knapping.
- Budget: hominin groups are few; full AI only near the player; group-level abstract simulation elsewhere (they're part of the ecological cell model as a population with needs and a range).

---

## 9. The player: body, identity & physiology

### 9.1 Character creation
A **Character** screen in the main menu (and at world creation), with a live, rotatable 3D preview under several lighting conditions:
- **Body:** male / female (body proportions and model), plus optional height and build sliders within the normal human range (cosmetic; they don't change stats).
- **Skin tone:** a continuous slider across the natural human range, with an undertone control and presets.
- **Hair:** styles (short crop, buzzed, shoulder-length, long straight, long wavy, curly, coily/afro, braids, locs, tied back, bald) and facial hair options (for any body), with natural hair colors (black, dark brown, brown, light brown, auburn, red, strawberry blonde, blonde, platinum, grey, white) plus a fine color picker.
- **Eyes:** brown, dark brown, hazel, amber, green, blue, grey.
- **Name** (optional).
- Saved as profiles; a profile can be picked when creating a world or respawning (§9.8).
- The character starts wearing only a **loincloth** (plain hide or plant fibre).
- Hair (and beard) grows slowly over calendar time and can be cut or tied back with the right tools (nice-to-have).

### 9.2 Body model
Replace the v1 MC-proportioned player with a **blocky but realistically proportioned** human rig (head, neck, chest, abdomen, pelvis, upper/lower arms, hands, upper/lower legs, feet), 1.6–1.9 m tall. Clothing is layered and visible (§10.3). First-person view shows arms and hands performing actions (knapping, carrying, climbing), and the player can see their own body looking down. Movement: realistic walk/run speeds (walk ~1.4 m/s, jog ~3 m/s, sprint ~6–7 m/s for short bursts), crouch, crawl, swim (stamina-limited), **climb** (ledges up to about head height with effort; trees with suitable branches; rock faces with holds, stamina-draining), realistic jumps (no 1.25-block leaps while carrying loads), and **fall injuries by impact speed** (sprains, fractures) rather than hearts.

### 9.3 Nutrition, hunger & thirst
- **Energy:** basal metabolism (from body size) plus activity costs (walking, running, carrying, chopping, swimming, shivering), measured in kcal on the body/action time scale (§4.2): roughly 2,000–3,500 kcal per game day for an active adult.
- **Macronutrients:** protein, fat and carbohydrate tracked in a simple model. A lean-meat-only diet eventually causes problems (real "rabbit starvation"); fat matters in cold climates. A slow "fresh foods" micronutrient reserve drops on long diets without fresh plants or organ meats and causes a scurvy-like decline (Authentic preset; slow and clearly signposted).
- **Hunger:** fullness (stomach capacity: you can't eat 5,000 kcal at once), digestion over hours, energy reserves (body fat as a stored buffer), and starvation over weeks of game time with worsening effects (weakness, cold intolerance, slower healing).
- **Hydration:** ~2–4 L per game day depending on heat and exertion, more when sweating; dehydration effects over a day, severe after ~2 days, fatal around 3 days. **Seawater** worsens dehydration. Snow must be melted (costs heat). Water sources differ in safety (§5.4); boiling and later filtering or clean containers reduce risk.
- **Food safety:** raw meat and fish risk parasites or food poisoning; spoiled food causes illness; some plants are toxic raw or at all; cooking, smoking, drying, salting and fermenting change safety and nutrition (§11.5).

### 9.4 Thermoregulation
Core body temperature from a simple heat-balance model: metabolic heat, clothing insulation (clo) and wind resistance, wetness (rain, swimming, sweat), air and water temperature, wind chill, sun, shelter and fire radiation, and ground contact when sleeping. Too cold: shivering (burns energy), numbness and clumsiness, frostbite at extremities, hypothermia. Too hot: sweating (thirst), heat exhaustion, heat stroke. Cold water immersion is dangerous within minutes. Deep mines are hot (§5.2).

### 9.5 Sleep & fatigue
- Fatigue accumulates with time awake and exertion; sleep deprivation degrades perception, accuracy and mood (screen and control effects, subtle).
- **Sleeping** is a real activity: lie down on the ground, bedding (grass, boughs, hides, furs), a hammock or a bed. Sleep quality depends on warmth, comfort, shelter, noise and safety.
- **While sleeping, time accelerates** (smoothly, e.g., up to 60–120×) instead of skipping. The world keeps simulating at the accelerated rate (fires burn down, weather changes, animals move) using the reduced fidelity layer where needed. Sleep is **interrupted** by cold, rain, pain, hunger, noise, or a nearby threat, and the player wakes with a short disoriented moment.
- Naps and resting (sitting) recover stamina and some fatigue.

### 9.6 Health, injuries & illness
- **Replace hearts** with a body model: blood volume, pain, and localized injuries (cuts, deep wounds, punctures, bites, bruises, sprains, fractures, burns, frostbite) on body regions, each with severity, bleeding rate, infection risk and healing time (short injuries on the day scale; fractures and deep wounds on the calendar scale).
- **Effects are physical:** a sprained ankle slows walking, a broken arm prevents two-handed actions, blood loss causes weakness and eventually unconsciousness, pain disturbs sleep and aim.
- **Treatment by technology:** pressure and elevation, cleaning with boiled water, bandages (plant fibre, bark, cloth), splints, resins and honey as antiseptics, medicinal plants, rest, warmth and food. Untreated wounds can become infected (fever, worsening).
- **Illnesses:** food poisoning, waterborne illness, infected wounds, envenomation (species-specific severity; most bites are survivable with rest), heat and cold illnesses, and a mild mosquito-borne fever risk in tropical wetlands. Keep illnesses medically plausible, understated and never gratuitous.

### 9.7 Stamina & carrying capacity
Short-term stamina for sprinting, swimming, climbing and heavy work, recovered by rest; long-term endurance limited by energy and hydration. Carrying load (§10) affects speed, stamina drain, balance on slopes and ice, swimming (heavy loads can drown you) and noise.

### 9.8 Death & respawn
Death is a big event, with configurable rules:
- **Legacy (default):** your body and belongings remain where you died. You continue as a **new person** (pick or create a character profile) who arrives in the same region, at your last campsite if one exists, carrying only a loincloth. Knowledge learned by the previous character is **partly kept** as journal notes (the "legend" of what they learned), so technique knowledge must be re-practiced but not rediscovered from scratch.
- **Permadeath:** the world ends with a summary of your life (days survived, places reached, technologies discovered).
- **Hardy:** respawn at your camp keeping all knowledge.

### 9.9 HUD & feedback
Minimal and diegetic by default: no numeric bars. The body communicates through sensations: stomach and thirst cues, shivering animation and breath fog in the cold, sweat and heat shimmer, vignette and heartbeat when badly hurt, desaturated vision when exhausted, muffled hearing when very weak. A **Body panel** (key B) shows a body diagram with injuries, temperature state, hunger/thirst/fatigue as descriptive states ("very thirsty", "chilled"), and treatments in progress. An optional **"Guided" HUD mode** shows compact bars for players who want them. There's no clock or compass until the player owns one; read the sun, stars, shadows and seasons. F3 still shows everything for development.

---

## 10. Inventory, carrying, clothing & containers

### 10.1 Principles
Every item has a real **mass** (kg), **volume** (L) and a **footprint** (grid cells) derived from its form and material. What you can carry depends on your hands, your clothing, your containers and your body. Nothing goes into an invisible pocket dimension.

### 10.2 Where things go
- **Hands:** a left hand and a right hand, each holding one item, or both holding one large item (a log section, a large stone, a carcass, a full water pot). Tools and weapons are used from the hands. Two-handed loads block most other actions and slow you.
- **Body:** a naked body with a loincloth has almost nowhere to carry things: a loincloth tie can hold one small item (a flake, a small pouch).
- **Clothing adds attachment points and capacity:** a belt (hangs a knife sheath, pouch, quiver, water skin, axe loop), garments with pockets or folds (later textile technology), shoulder straps, cloaks that can bundle items.
- **Containers** each have their own grid and maximum load: wrapped bundles (a hide or cloth tied around items), pouches, bags (hide, then textile), **baskets** (woven; carried in hand or as a back basket), a **tumpline** or pack frame for heavy loads, rucksacks (later), quivers, sheaths, water skins, gourds, pottery jars (heavy but sealable), birch-bark containers. Containers can hold containers.
- **Carrying and hauling large things:** dragging (slowly, with rope), rolling logs, travois and sledges (on snow and grass), rafts and canoes on water, and later **pack animals, carts and boats**. Dogs can carry small packs.
- Realistic human limits: comfortable load about 20–25% of body mass, heavy up to ~50% with major penalties, more only for short lifts.

### 10.3 Clothing
Clothing layers by body region (under, main, outer, footwear, hands, head, and back/belt gear), each garment with insulation (clo), wind and water resistance, breathability, weight, durability, attachment points and capacity, noise and visual color (stealth). Materials follow technology: bark cloth and woven grass, hides and furs (untreated hides rot and stiffen; tanned leather lasts; §12), sewn fur clothing (eyed needles), felt, plant-fibre textiles (nettle, flax/linen, hemp, cotton), wool (after sheep domestication), footwear (wraps, moccasins, shoes, boots with insulation), mittens, hats and hoods, rain capes (bark, oiled leather). Wet clothing loses insulation and weighs more; clothing can be dried by a fire. Armor is realistic and technology-bound: hide and layered leather, wooden or wicker shields, later bronze and iron (scale, lamellar, mail, helmets). No diamond armor.

### 10.4 Inventory UI
A grid-based inventory screen showing your body, your hands, and every worn container as panels; drag items between them, rotate items, stack only what really stacks (arrows in a quiver, berries in a basket measured by volume), split stacks, and see total carried mass and its effect. Holding a container in hand opens it. Put items **down** in the world on any surface: items are physical objects that sit where you place them (on the ground, on a shelf, in a pit). Stored food spoils; stored hides need care; items left outside get wet.

### 10.5 Quick access
The v1 hotbar is replaced by **quick slots** mapped to belt/attachment points and hands (keys 1–6 by default, plus a radial quick-select wheel), so drawing a knife from its sheath or an arrow from the quiver takes a moment, as it should.

### 10.6 Controls changes (all still rebindable per v1)
Primary action with right hand (LMB), secondary/left-hand action (RMB), interact/pick up (E), inventory (Tab or I), drop/put down (G), carry two-handed / drag (hold F), quick slots (1–6), radial quick-select (hold Q), crouch (C), prone (Z), climb (hold Space at a ledge), throw (hold and release), body panel (B), journal (J), map (M, map only shows what you've explored or drawn; §16.3). Keep all v1 settings screens and conventions.

---

## 11. Interacting with the world & making things

### 11.1 Gathering by hand
With nothing but hands you can: pick up loose stones, sticks, fallen branches, bark strips and leaf litter; pluck grass, reeds and fibrous plants; pick fruits, berries and nuts in season; dig loose soil, sand and mud slowly; collect clay from banks; gather eggs, shellfish and insects; drink from water sources; break brittle deadwood. You **cannot** punch trees down or break stone with bare hands.

### 11.2 Blocks, material and piles (realistic volumes)
A block is a cubic metre of real material and has real mass (a block of granite weighs about 2.7 tonnes; a block of soil 1.2–1.6 t). Removing terrain produces human-scale units: digging soil yields loose soil that falls into a **spoil pile** next to the hole unless carried away in baskets; quarrying rock yields stones and rubble of real sizes; felling yields a fallen tree to process. Loose materials (sand, gravel, soil, snow) have realistic **angle-of-repose** behavior and slump when piled too steeply. Placing terrain materials back builds piles, ramps and embankments at their real volumes (a basket load ≈ a fraction of a block), with the conversion ratios in data and exposed in `balance/` for playability tuning.

### 11.3 Process-based crafting
Crafting is performing **processes** on materials in the world with tools, at workstations, under conditions. A process in `processes/` declares: inputs (materials/forms with quantities or ranges), tools (with properties like "sharp edge", "hard hammer", "heat ≥ 900 °C"), workstation or environment conditions (fire, water, dry weather, a kiln), the action and its duration on the correct time scale (§4.2), the knowledge and skill involved (§12), outputs with quality rules, by-products and waste (flakes, sawdust, slag, ash), and failure modes.
- **Contextual actions:** looking at a target with items in hand offers the actions possible *with what you know* (hold a hammerstone and look at a flint nodule: "strike to test", "strike off a flake", "shape a core").
- **Hands-on minigames for skill-based crafts**, kept short and skippable once practiced: **knapping** (a sub-block stone grid where the player chooses strike points and angles; material quality decides how predictably flakes come off: flint and obsidian excellent, chert good, quartzite and basalt coarse), carving and whittling, hide scraping, weaving and basketry patterns, clay shaping, sewing, and smithing (heat window and hammer blows). With experience, "do it again" completes a known process automatically in its normal time.
- **Workstations are physical builds** made through construction (§14): hearths and fire pits, earth ovens, drying racks, smoking racks, tanning frames and pits, pit kilns, updraft kilns, charcoal clamps, copper smelting furnaces with bellows, crucibles and molds, bloomeries, forges with anvils, looms (warp-weighted, then horizontal), querns, and later water-powered mills. Their performance depends on how they're built (a kiln with better insulation reaches higher temperatures).

### 11.4 Heat & fire
- **Fire-making** is hard and realistic: natural fire (lightning-lit wildfire, carried as embers), then friction methods (hand drill, bow drill; success depends on wood species, dryness, tinder quality, humidity and skill), then percussion (flint and pyrite/marcasite, later flint and steel). Fire needs tinder, kindling and fuel; wet fuel smokes and fails; wind and rain matter; fires must be fed and can be banked overnight or carried as embers in a fungus or clay container.
- **A simple thermal model** for fires, ovens, kilns, furnaces and items: fuel energy, burn rate, airflow (bellows raise temperature), insulation and heat loss. Items have temperatures: hot stones can boil water in a hide container (stone boiling), clay fires at the right temperature for the right time, metals melt at real temperatures (copper ~1,085 °C, tin ~232 °C, bronze ~950 °C, iron via bloomery reduction ~1,200 °C without melting). Hot items glow with blackbody color (rendering tie-in), can't be held without tongs or insulation, cause burns, and cool over time.
- **Charcoal** is made in clamps or pits and burns hotter and cleaner than wood.

### 11.5 Food & cooking
Cooking methods follow technology: roasting on coals and spits, earth ovens, stone boiling, then pottery boiling, baking, grinding grain into flour, bread, porridge, rendering fat, brewing and fermenting. **Preservation:** drying (sun, wind, fire), smoking, salting (salt from evaporation pans or rock salt), freezing in winter, cold storage in pits or springs, fermentation, pemmican (dried meat, fat, berries), sealed pottery, and later granaries. Food spoils by type, temperature and moisture. Cooking changes nutrition and safety (§9.3).

### 11.6 Tools: quality, condition, maintenance
Tools have material, craftsmanship quality, sharpness, durability and condition. Edges dull with use and are resharpened (retouching stone, whetstones for metal); hafts crack; bindings loosen and must be rebound; bows lose strength if kept strung or get wet. Better materials and knowledge give better tools; the difference between a crude flake and a fine blade is felt in every action's speed.

---

## 12. Knowledge & the technology graph

### 12.1 Structure
Technology is a **graph of knowledge nodes** in `knowledge/`, each describing a technique or concept with: prerequisites (other knowledge, and the materials, tools, workstations or environments it requires), the processes it enables, **discovery routes** (§12.3), a skill track, an **era tag**, real historical context (a short journal-style description and approximate real date), and a `status` (`implemented` / `planned`). Planned nodes are data only; they're hidden in-game and exist so the graph can grow toward later eras without restructuring.

### 12.2 Eras and scope
The graph follows the real history of technology, from the first stone tools about 3.3 million years ago, through controlled fire, the Neolithic package of agriculture, pottery, woven cloth and eventually the wheel, irrigation, sailing, iron, and onward through windmills, the compass, mechanical clocks, printing, steam, electricity and computing. (Use the Britannica technology timeline the user supplied as the spine, expanding each milestone into the realistic chain of steps that led to it.)

**Implement now (playable): Eras 0–5. Author as `planned` data: Eras 6–8.** See **Appendix C** for the node list.
- **Era 0 — Lower Paleolithic:** hammerstone & anvil use, sharp flakes, choppers, digging and throwing sticks, termite probes, hand axes and cleavers, wooden spears (fire-hardened once fire is known), carrying bundles, **fire keeping** (captured natural fire), butchery, marrow extraction, stone boiling (late), simple windbreaks.
- **Era 1 — Middle Paleolithic:** fire-making by friction, prepared-core knapping (Levallois), scrapers, hafting with bindings and adhesives (birch tar, pine pitch with ochre), stone-tipped spears, hide scraping and wrapping (simple clothing), shelters, ochre pigments, basic cordage.
- **Era 2 — Upper Paleolithic:** blade technology, burins, bone and antler working (awls, **eyed needles**, harpoons, fishhooks), sewn fitted clothing, advanced cordage and **nets**, snares and traps, the **atlatl**, the **bow and arrow**, fat lamps, rafts, baskets, the **dog** (wolf domestication begins), grindstones for wild seeds, fish weirs, art (cave painting with ochre and charcoal as a creative mode).
- **Era 3 — Mesolithic & Neolithic:** ground and polished stone axes and adzes (much better for felling), dugout canoes, sledges and skis, **plant domestication** (selective seed saving; einkorn/emmer/barley/lentil/flax pathways, others per region), **animal domestication** (§7.7), **pottery** (hand-built, pit-fired, then kilns), spinning (spindle and whorl) and **weaving** (loom), linen and wool textiles, permanent houses (pit houses, wattle and daub, mudbrick, timber, thatch), querns and bread, fermentation, dairy (milk, cheese), granaries and storage pits, fences and pens, the **ard plough** with draft animals, **irrigation** (channels, dikes; ~6000 BCE), and the **wheel** (potter's wheel, then cart wheels and axles).
- **Era 4 — Chalcolithic & Bronze Age:** native copper cold-working and annealing, copper **smelting** from oxide/carbonate ores, crucibles, bellows, molds and casting (open molds, then two-part and lost-wax), arsenical copper, **tin bronze**, metal tools (saws, chisels, axes) transforming carpentry, fired brick, lime plaster, faience and early glass, **sailing** (~4000 BCE) and plank boats, wheeled carts, horse riding, dyes (plant dyes, ochres, azurite/malachite), record-keeping marks (a placeholder for writing in future eras).
- **Era 5 — Iron Age & Classical:** **bloomery iron** (~1200 BCE for widespread use), forging, carburizing to steel, quench and temper, iron tools and ploughshares, rotary querns, lime mortar and **concrete**, arches and vaults (structural bonus; §14), aqueducts, cisterns, the crane and pulley, the lathe, glassblowing, the **water wheel** (the first mechanical power network), advanced ships and sails.
- **Era 6 — Medieval (planned):** windmills (~950), the magnetic compass (~1044; lodestone/magnetite), mechanical clocks (~1250–1300), gunpowder (~850; saltpeter, sulfur, charcoal), heavy plough, horse collar, spinning wheel, blast furnace and cast iron, printing (~1455).
- **Era 7 — Industrial (planned):** the steam engine (~1765), railways (~1804), steamboats (~1807), photography, the mechanical reaper, the telegraph (~1844), the telephone and internal-combustion engine (~1876), electric light (~1879), the automobile (~1885).
- **Era 8 — Modern (planned):** radio, powered flight, rocketry, television, computers, nuclear power, the transistor, spaceflight, personal computers, the internet, smartphones, gene editing, artificial intelligence.
For the planned eras, design the **systems** they'll need in `docs/design/future-systems.md` (mechanical power networks, fluid/pressure systems, electrical networks, chemistry/industrial processes, vehicles, communications) so the architecture has room, but don't implement them now.

### 12.3 Discovery (how the player learns)
Knowledge is gained, never granted, through:
1. **Experimentation:** doing related things with relevant materials accumulates insight toward a node (striking different stones reveals which fracture cleanly; clay left by the fire hardens; meat hung in the smoke keeps longer; a green stone heated in a hot fire leaves beads of copper). Insight thresholds and triggers are data.
2. **Observation:** watching hominins (§8.3), natural phenomena (lightning-lit fires, beavers felling trees, birds weaving nests, spiders' webs, water flowing through a channel), and animals.
3. **Inference:** knowing two techniques can suggest a combination ("I can bind a flake to a stick" + "a spear" → idea: "tip the spear with a stone point").
4. **Found evidence:** tool scatters and remains.
When a node is discovered, the **journal** records it in the character's voice with a sketch, and the relevant actions appear. Partial insights appear as hints and hunches ("The clay near the hearth has gone hard and pale. Heat changes it."), so curious players get nudges without a recipe list. A world setting **Knowledge Mode** offers Discovery (default) / Guided (hints are more explicit) / Open (all knowledge available; sandbox and testing).

### 12.4 Skills
Practicing a technique improves its **skill** (speed, success rate, output quality, fewer wasted materials), with diminishing returns and slow decay if unused for a long time (Authentic preset). Skills live in the character's `KnowledgeState` and are what the Legacy death mode partly loses (§9.8).

### 12.5 Effort parity checklist
For each implemented node, the data and a design note must show: materials that are genuinely needed and plausibly available in the right geology/biome, the tools and workstation it really needs, realistic temperatures and times (via §4.2), the failure modes people actually faced, and how its output makes life measurably better. The `hearth content lint` report includes a "cost of each technology from scratch" rollup (total gathered materials, steps and game hours from nothing), which should increase steeply by era.

---

## 13. Materials & resources (replacing v1 ores)

### 13.1 Materials
`materials/` defines physical materials with real properties: density, hardness (Mohs where relevant), fracture behavior (conchoidal for flint/obsidian/chert), compressive/tensile/bending strength, thermal conductivity, melting/firing points, fuel value, workability, durability/rot resistance, and appearance. Everything (blocks, items, tools, buildings) takes its behavior from these.

### 13.2 Resources placed by geology
Place resources as real **deposit models** within the geological provinces of §5 (Appendix D has the full list):
- **Toolstone:** flint nodules in chalk and limestone; chert beds; obsidian at volcanic flows (excellent but brittle); quartzite and basalt cobbles in rivers and glacial deposits; fine-grained stones for grinding and polishing.
- **Copper:** green and blue stains of malachite and azurite at the oxidized tops of porphyry copper systems in volcanic arcs; chalcopyrite in veins and deeper sulfide zones; rare native copper in basalts and glacial drift.
- **Tin:** cassiterite, in veins near granite intrusions and concentrated as heavy grains in stream gravels (placers), often far from copper, which makes bronze a matter of finding both.
- **Iron:** bog iron in boreal wetlands (the easiest early source), hematite and goethite/limonite, magnetite (also lodestone, magnetic), banded iron formations in shields, laterite in the tropics.
- **Gold & silver:** native gold in quartz veins and as placer gold in rivers downstream (pannable); silver in lead ores (galena).
- **Lead & zinc:** galena and sphalerite in veins and carbonate-hosted deposits.
- **Fuel:** wood, charcoal, peat in bogs, lignite and bituminous coal seams in basins, animal fat and oil; bitumen seeps (stretch).
- **Salt, sulfur, saltpeter:** rock salt and salt pans in arid basins, sea salt by evaporation, sulfur at fumaroles, niter in caves and arid soils.
- **Clays, sands, lime:** earthenware clay, kaolin, fire clay; quartz sand for glass; limestone and chalk for lime; volcanic ash (pozzolana) for concrete.
- **Pigments & gems:** red and yellow ochre, manganese black, charcoal, azurite blue, malachite green; gemstones (quartz varieties, garnet, rare diamonds in kimberlite) are real minerals used for ornament, trade in future eras, and abrasives; they have **no armor or tool-tier role**.
- **Deposits are real bodies:** veins, seams, placers, disseminated ore bodies with low grades, nodules, crusts. Grade and size vary; most rock is just rock.

### 13.3 Prospecting & mining
- **Surface indicators:** rusty gossans over sulfide deposits, green stains on outcrops, distinctive float (ore pieces in streams and scree, which can be followed upstream), heavy dark grains and gold flecks when **panning** river gravels, indicator plants that tolerate metal-rich soils, springs with odd tastes, and outcropping seams in cliffs.
- The journal identifies rocks and minerals only as the player gains knowledge (at first: "a heavy, green-stained stone").
- **Mining** uses real tools (antler picks, stone mauls, fire-setting to crack rock, bronze and iron picks, wedges), with realistic rates by rock hardness. Tunnels in weak rock and soil need **timber supports** (§14). Mines are dark (lamps and torches are consumables), may flood below the water table, and get warmer with depth.
- **Ore processing:** hand sorting, crushing, grinding, washing and panning, roasting sulfide ores, then smelting (§11.4) and refining. Smelting produces slag and yields realistic, often disappointing, amounts of metal from low-grade ore.

---

## 14. Building & structural integrity

### 14.1 Construction pieces
Building uses realistic pieces inside the grid: posts, beams, rafters, joists, planks and floorboards, logs (for log walls), wattle panels, daub, thatch bundles, bark sheets, hide covers, poles (tents, lean-tos, teepee-like frames), snow blocks (snow shelters and domes), stones (dry-stone and mortared walls), mudbrick and adobe, rammed earth, fired brick, dressed stone, lime mortar, concrete, doors, shutters, ladders, stairs, fences and palisades. Many are sub-block models that connect to neighbors; several pieces combine into one wall/floor/roof block where appropriate. Construction takes real material quantities and time (§4.2), with **construction stages** visible (a frame before walls, walls before a roof).

### 14.2 Structural model (must be fast and deterministic)
- Every built piece has material properties: mass, **compressive capacity**, **span/cantilever capacity** (how far it can bridge or stick out unsupported), and **joint strength** by joinery (lashed, pegged, mortise-and-tenon, nailed later, mortared).
- **Load and support:** on any change, run an incremental stability solve on the affected connected structure (bounded size, e.g., up to ~8,000 pieces): find load paths to the ground or to anchored natural rock, accumulate each piece's load from above, and check capacity and spans. Use a Valheim-like support propagation as the fast first pass and a load check for heavy materials. Pieces that fail break and fall as physical debris, possibly cascading, with sound and dust.
- **Natural terrain** is anchored and stable unless modified. Excavation matters: large unsupported spans in weak rock and soil **collapse** unless shored with timber sets or pillars; strong rock (granite, basalt) spans far more.
- Roofs shed rain only if they're sloped and made of suitable material; flat bark or hide roofs leak and degrade. Thatch needs pitch. Mudbrick erodes in rain without plaster and eaves. Timber rots in ground contact unless rot-resistant or on a stone footing.
- **Shelter quality** is computed for the player (§9.4, §9.5) from enclosure, roof, insulation and fire: a good shelter really makes winter survivable.
- A **builder's view** (while holding a construction tool) shows load/stress coloring and whether a placement will stand, so structural rules are learnable.
- Arches, vaults and domes are supported as real structural forms in Era 5 (compression-only paths that span much further in stone).

---

## 15. Rendering, audio & feel (additions to v1)

- **Vegetation:** species textures and silhouettes, passable foliage with cutout and wind, bare deciduous trees in winter, seasonal colormaps, leaf litter, snow on branches (stretch), GPU-instanced grass and ground cover, LOD canopy color per species mix and season.
- **Animals:** instanced rendering for herds, flocks and schools; animation LOD (full procedural animation near, simplified far, impostors or dots very far so migrating herds and bird flocks are visible across a valley); footprints and tracks as fading decals.
- **Fire & heat:** better fire and smoke (heat shimmer, embers, smoke columns visible in LOD), incandescent hot metal and stones, kiln and furnace glow.
- **Light sources are consumables:** fires, fat lamps, resin and rush torches that burn down, later oil lamps and lanterns. Held light matters in v1's realistic darkness.
- **Deep sea bioluminescence** as emissive, bloomed points in the abyss.
- **Soundscape as information:** layered ambience by ecosystem, season, time of day and weather: dawn chorus, cicadas and crickets, frogs in wetlands, wind in conifers vs broadleaf canopies, surf, ice creaking on lakes, distant wolf howls and lion roars carrying at night, alarm calls when a predator moves, whale song underwater, and silence in deep caves. Animal sounds are positional, occluded, and part of animal behavior (alarm calls spread).
- **Player body audio & effects:** breathing with exertion, heartbeat when injured, shivering teeth, footstep sounds by surface and footwear.

---

## 16. World creation & menus

### 16.1 Create World flow
1. **Planet:** seed, Planet Size (Standard recommended; v1 table), vertical scale, land fraction, feature rarity, as in v1.
2. **Life & time:** day length, days per season, starting season, axial tilt, Realism preset (§3.4), Predator Behavior (§7.5), Knowledge Mode (§12.3), Hominin range (§8.1), Death rules (§9.8).
3. **Era:** an era selector showing only *Wild Earth* as available, with future eras visible but disabled and labeled "coming later" (§17.1).
4. **Choose your spawn on the globe:** after the planet is generated (progress bar), the player sees the rotatable **globe** (biome/climate-colored, with a toggle for relief, climate classes and a species-richness heat map). Clicking a point shows its latitude, climate, biome, typical season conditions, notable dangers and resources the character would plausibly know about (e.g., "temperate forest, cold winters, wolves and bears present, flint in the chalk cliffs"), with a difficulty hint. Spawning in the ocean snaps to the nearest suitable coast. There's a "Recommended starting regions" filter and a "Surprise me" option.
5. **Character:** choose or create a character profile (§9.1).

### 16.2 Main menu additions
Character profiles screen, a world list showing each world's globe thumbnail and your character's current location, and the v1 settings screens.

### 16.3 In-game map
The map shows only what the character has seen (terrain drawn as it's explored), and markers the player places. A **"Full map knowledge"** world option reveals everything. Later technology (drafting, surveying, the compass) improves the map's accuracy and features.

---

## 17. Designing for the future: eras & simulated humanity

The user plans, later, to let the player choose a historical **time period** for the world, with realistic simulation of human progress: settlements, towns, cities and structures that grow, and the advancement and spread of knowledge, driven by simulated human agents. Don't build that now, but make sure nothing you build now blocks it.

### 17.1 Eras as data
`eras/` defines each era: available flora and fauna (every species has `first_appearance` and `extinction` in years before present, so a Pleistocene era could bring mammoths, woolly rhinos, cave bears and sabre-toothed cats and remove recent domesticates), hominin/human species present and their knowledge baselines, climate offsets (a glacial-maximum era lowers sea level by ~120 m scaled and expands ice sheets, exposing land bridges), and which systems are active. **Only one era is enabled now: "Wild Earth"**: a timeless wild Earth with no civilization, present-day wild fauna *plus* the wild ancestors of domesticated species that have since gone extinct or become rare (aurochs, wild horses, wild dromedaries…) so domestication is possible, and sporadic *Australopithecus* by the user's request.

### 17.2 Hooks to build now
- The **Agent framework** (§8.4) with body, mind, knowledge, inventory, social group and culture, used by hominins today.
- **Knowledge transfer** between agents (observation, and later teaching and records), already used for hominin → player.
- A `HistorySimulator` trait and world-creation stage that runs before play (a no-op for Wild Earth), which in future will simulate thousands of years of human spread and settlement over the planet's resources, rivers, soils and climate.
- A **settlement/structure data model**: blueprints built from the same construction pieces and structural rules as the player (§14), so simulated towns obey physics too.
- **Ownership, territory and resource claims** fields on world data (unused now).
- Keep simulation layering (§3.7) general enough that human populations can live in the abstract layer, like animal populations, and materialize near the player.
Document these in `docs/design/future-humanity.md`.

---

## 18. Cohesion: the interaction matrix

Maintain `docs/design/interactions.md`, a matrix of how each system affects the others, and make sure every row is implemented. The core loops that must work together:
- **Season → plants → herbivores → predators → danger:** spring green-up and births, summer abundance, autumn fattening and nut mast, winter scarcity (predators bolder, herds migrate, bears hibernate).
- **Climate & weather → water & fire → vegetation → animals:** dry seasons shrink water holes (animals concentrate there, and so do crocodiles and lions), lightning starts fires, burns regrow as grassland, grazers follow.
- **Geology → soil → plants → animals → people:** fertile alluvium supports lush growth and big herds and later farms; chalk cliffs give flint; volcanic arcs give obsidian, sulfur and copper; bogs give peat and bog iron.
- **Player → ecosystem:** hunting pressure, clearing, fire, farming, domestication and waste (scavengers) change the local ecology in visible, lasting ways.
- **Body ↔ environment:** cold and wet drive clothing, shelter and fire technology; heat drives water carrying and shade; food variety drives foraging and trade-offs.
- **Knowledge ↔ materials ↔ geography:** progress requires traveling to where the resources are, which requires carrying, transport and preservation technology.

---

## 19. Milestones (v2)

Work in this order. Each milestone: implement, write/update its design doc, add data, write tests, run `scripts/check.sh` and `hearth content lint`, update `PROGRESS.md`, commit. Vertical slice first, then breadth, so the architecture is proven before the content explodes.

**V2-0 — Migration & content platform.** `MIGRATION.md`; remove dropped v1 content systems cleanly; content data system with schemas, cross-reference validation, lint, graph export, hot reload; units; balance layer and presets; save versioning and migration framework; `docs/design/` skeleton; `Open` knowledge mode for testing. *Accept:* lint passes on a seed dataset; v1 worlds are refused with a clear message; a save from V2-0 loads after a deliberate format change via a migration test.

**V2-1 — Time, calendar & seasons.** Day length and calendar settings, axial-tilt sun and day length by latitude, the two time scales, seasonal climate and weather, snow cover and ice, phenology state plumbing, seasonal colormaps and LOD, starting season. *Accept:* screenshot suite at 4 seasons × 3 latitudes; unit tests for solar position and day length vs known values; a year simulated headlessly shows correct snow, ice and wet/dry season timing by latitude.

**V2-2 — Geology, soils, hydrology & resources.** Provinces, stratigraphy, basement, geothermal gradient, soils, groundwater and springs, seasonal rivers, finite player-moved water, reefs and coasts, deposit models for every Appendix D resource, surface indicators, panning. *Accept:* `worldmap` gains geology, soil and deposit layers; statistical tests confirm deposits occur only in their provinces and at plausible frequencies; each Era 0–5 resource is reachable from any continent within a reasonable distance at Standard size, or the lint flags the gap.

**V2-3 — Player: character creator, body & physiology.** Character profiles and creator with preview, the new rig and animations, first-person body, movement/climbing/falls, metabolism and nutrition, hydration, thermoregulation, sleep with time acceleration, injuries and illness, death modes, diegetic HUD, Body panel, Guided HUD. *Accept:* headless physiology tests (e.g., a naked character in 5 °C rain becomes hypothermic within realistic game time; one with fur clothing and a fire does not; dehydration is fatal after ~3 game days with no water; recovery from a sprain vs a fracture takes the right time scale).

**V2-4 — Inventory, carrying & clothing.** Hands, body attachment points, containers with grids and limits, mass/volume/footprint, placing items in the world, dragging and rolling, encumbrance, quick slots and radial, clothing layers with insulation and capacity, visible clothing on the model. *Accept:* tests for capacity rules and encumbrance effects; UI round-trip tests; carrying a 100 kg log section is possible only by dragging and is slow.

**V2-5 — Interaction, process crafting & knowledge.** Gathering by hand, contextual actions, the process engine (inputs, tools, conditions, durations, quality, by-products, failures), the thermal model for fires and items, knapping and the other short minigames, skills, journal with discovery routes, Era 0–2 knowledge and processes, fire-making, cooking and preservation basics, tool condition. *Accept:* a scripted bot can progress from nothing to fire, a hafted stone spear, sewn hide clothing and dried meat using only discovery routes; `content graph` shows no unreachable Era 0–2 nodes; the effort rollup rises by era.

**V2-6 — Flora framework (temperate first).** Plant model, procedural tree growth for the temperate species set, wood properties, passable foliage, branches and climbing, felling and processing, understory, edible/medicinal/toxic plants, succession, vegetation cell state, wildfire, rendering and LOD. *Accept:* screenshot comparisons of each species' silhouette at 3 ages; a cleared area goes through succession over simulated years; wildfire spreads and burns out plausibly in a dry-season test.

**V2-7 — Fauna framework (temperate forest ecosystem first).** Body plans, procedural animation, species textures, senses with wind-carried scent, utility AI and behaviors, herds/packs, ecological cells and population dynamics, materialization, biogeographic realm assignment, predators and dangerous herbivores, tracking, hunting, butchering, the Tier 1 temperate set from Appendix B, ambient birds and insects. *Accept:* a 50-year headless ecosystem run on a temperate region stays within plausible bounds for every species; a scripted player hunting heavily depletes and later recovers a local deer population; predators attack only for realistic reasons (log each attack's cause in tests).

**V2-8 — Structural building & shelter.** Construction pieces, stages, stability solver, collapses, excavation supports, shelter quality, builder's view, Era 0–3 construction techniques. *Accept:* scripted structural tests (a stone span too long collapses; a timber-supported tunnel stands; a thatched roof keeps rain out and a flat bark roof leaks); solver stays under its time budget on large structures.

**V2-9 — Vertical slice review.** A complete temperate-forest experience: start in a loincloth in spring, survive a full year through learning, hunting, gathering, shelter, clothing and fire, with seasons, predators and weather. Run the scripted bot through a year, capture screenshots and logs, and write `docs/review/slice-1.md` with what works, what isn't fun or realistic, and fixes. Fix the top issues before expanding.

**V2-10 — Ecosystem expansion waves.** In order: boreal forest & tundra & polar; grassland/steppe & desert; savanna & tropical forest (with termites, vultures, water-hole dynamics); mountains & alpine; wetlands, rivers & lakes (beavers, salmon runs); oceans (coasts and intertidal, reefs, kelp forests, open ocean, deep sea with bioluminescence). Each wave adds its Appendix A/B species, biome-specific survival challenges, and a 50-year stability run. *Accept:* per-wave stability tests and screenshot suites.

**V2-11 — Australopithecus & the agent framework.** Agent framework as §8.4, hominin groups, behaviors, tool use through the process engine, tree nests, traces, habituation, observation learning. *Accept:* hominin groups persist for decades in suitable habitat in headless runs; a scripted observer gains insight toward knapping by watching them.

**V2-12 — Neolithic.** Era 3 in full: domestication of plants (seed selection across generations) and animals (§7.7), farming (soil, sowing seasons, weeds, pests, irrigation, fallow, manure), pottery and kilns, spinning and weaving, permanent houses, querns and bread, dairy, storage, boats and sledges, the wheel. *Accept:* a bot domesticates a grain over simulated generations and sees yields rise; a sheep lineage becomes docile and woolly over generations; pottery fired below temperature fails realistically.

**V2-13 — Metallurgy & mining.** Era 4–5 metals and the whole chain: prospecting, mining with supports, ore processing, charcoal, furnaces, bellows, crucibles, casting, alloying, bloomery, smithing, heat treatment, and the related tools and armor. *Accept:* realistic yields from ore; bronze requires both copper and tin sources; iron requires a bloomery and forging; tool quality differences measurable in action speeds.

**V2-14 — Late scope: Iron Age & Classical.** The remaining Era 5 nodes: lime mortar and concrete, arches and vaults, cranes and pulleys, lathe, glassblowing, water wheel and a minimal mechanical power network, advanced boats and sails, carts with draft animals. *Accept:* each node playable end-to-end; mechanical power network design doc ready for Era 6.

**V2-15 — World creation & menus.** The full §16 flow including era selector, globe spawn picker with region info, character selection, map with exploration memory. (Build a minimal version of the globe spawn picker early, during V2-2 or V2-3, since it speeds up testing; finish it here.) *Accept:* UI tests; spawning at chosen points across climates works.

**V2-16 — Long-run balance, performance & cohesion QA.** 100-year headless planet-scale runs (abstract layer) for stability; performance budgets (§21) verified; the interaction matrix fully checked; screenshot suite across every ecosystem, season and time of day; a scripted "survive two years in three different climates" run; `docs/review/v2-final.md`.

---

## 20. Resume protocol additions
Keep v1's resume protocol. Also keep current: `MIGRATION.md`, `docs/design/*`, `docs/review/*`, and a **Content Status** table in `PROGRESS.md` (implemented vs planned counts per domain: species, plants, knowledge nodes, processes). When restarting, run `hearth content lint` alongside `scripts/check.sh`.

---

## 21. Performance budgets (in addition to v1 targets)
Frame-rate targets from v1 still apply with the new content. Per-frame and per-tick budgets on the reference machine (8-core CPU, RTX 3060/RX 6600 class):
- Up to ~300 fully simulated animals and ~10 hominins near the player plus instanced ambient life (thousands of birds, fish and insects) within ~3 ms of simulation per tick across worker threads.
- Ecological cell updates amortized to under ~1 ms per tick; a planet-wide abstract year simulates headlessly in under a minute.
- Structural solves under ~2 ms for typical edits, never blocking the main thread (defer large ones and show a brief "settling" state).
- Vegetation growth and phenology updates amortized; seasonal colormap changes are GPU-side parameter updates, never remeshing the world.
- Physiology and thermal models are O(1) per entity per tick.
- Everything that can run off the main thread does.

---

## Appendix A — Flora (tier 1 = temperate vertical slice; tier 2 = expansion waves)

**Trees — temperate (tier 1):** English/pedunculate oak, white oak, silver birch, paper birch, Norway spruce, Scots pine, European beech, ash, sugar maple, field maple, hazel (shrub-tree), willow (white willow, osier willow), aspen, alder, wild cherry, wild apple/crab apple, yew, elm, hawthorn, elder, linden/basswood, sweet chestnut, walnut, hornbeam.
**Trees — tier 2:** boreal: white spruce, black spruce, balsam fir, larch/tamarack, jack pine; montane: Douglas fir, western red cedar, mountain hemlock, stone pine, juniper; Mediterranean: olive, cork oak, holm oak, Aleppo pine, cypress, carob, fig; arid: date palm, acacia, mesquite, Joshua-tree-like yucca, desert willow; savanna: umbrella acacia, baobab, marula; tropical: kapok (emergent), mahogany, teak, rubber tree, coconut palm, banana (herbaceous), mango, cacao, bamboo (giant and small), strangler fig, mangroves (red, black); temperate rainforest: giant sequoia, coast redwood, Sitka spruce, western hemlock; wetlands: bald cypress, black gum; subtropical: live oak, magnolia, eucalyptus (in an Australasian-like realm).
**Shrubs & bushes:** blackberry, raspberry, blueberry, lingonberry, cloudberry, gooseberry, currant, rose/rosehip, juniper, heather, gorse, sagebrush, creosote bush, saltbush, tea-like shrubs, dwarf willow and dwarf birch (tundra), rhododendron, manzanita, sea buckthorn.
**Herbs & grasses:** many grass species by climate (bunchgrasses, sod-forming prairie grasses, tall savanna grasses, tussock tundra sedges), reeds, cattails, papyrus, bulrush, rushes; nettle (fibre, food), flax (wild), wild hemp, milkweed (fibre), yarrow, plantain, dandelion, wild garlic/ramsons, wild onion, sorrel, clover, mint, sage, thyme, fennel, wild carrot, burdock, chicory; toxic: hemlock, water hemlock, deadly nightshade, foxglove, monkshood.
**Crop ancestors (domesticable):** einkorn and emmer wheat, wild barley, wild rye, wild oats, wild rice, teosinte (→ maize), wild millets, sorghum, lentil, pea, chickpea, bitter vetch, wild beans, wild squash and gourds, flax, cotton, potato (wild), manioc/cassava, yam, taro, wild grapes, olives, dates, figs.
**Succulents & cacti:** saguaro-like columnar cactus, prickly pear, barrel cactus, agave, aloe.
**Aquatic:** water lily, lotus, pondweed, duckweed, watercress, eelgrass/seagrass, giant kelp, sea lettuce, bladderwrack.
**Fungi, mosses, lichens:** chanterelle, porcini, morel, oyster mushroom, puffball, death cap (toxic look-alike), fly agaric, tinder/horse-hoof fungus (fire), birch polypore; sphagnum moss (bogs, wound dressing), reindeer lichen (caribou food), rock lichens.
**Corals:** reef-building corals and associated reef structure blocks (tier 2 oceans wave).

## Appendix B — Fauna by ecosystem (tier 1 = temperate forest slice; the rest in expansion waves)

**Temperate forest (tier 1):** red deer/white-tailed deer, roe deer, wild boar, European/eastern cottontail rabbit or brown hare, red squirrel/grey squirrel, red fox, badger, gray wolf (**predator**), black bear/brown bear (**predator**, omnivore), lynx/bobcat (**predator**), aurochs (dangerous herbivore; domesticable ancestor), wild turkey or woodland grouse, owls, woodpeckers, songbirds (ambient), crows/ravens, hedgehog, wild honey bees (ambient + honey), freshwater trout and perch, frogs, adders/rattlesnakes (**venomous**).
**Boreal & tundra & polar:** moose (dangerous herbivore), caribou/reindeer (migration), brown bear, gray wolf, lynx, wolverine, beaver (dam builder), snowshoe hare, ptarmigan, musk ox, arctic fox, arctic hare, snowy owl, lemmings, polar bear (**predator**, north only), ringed seal, walrus, emperor/king penguin-like penguins (south only), salmon (runs), arctic char, pike, mosquitoes (summer).
**Grassland & steppe:** bison (dangerous herbivore), wild horse (domesticable ancestor), pronghorn or saiga, prairie dogs/marmots/ground squirrels, coyote (**predator**), gray wolf, golden eagle, bustards, rattlesnake (**venomous**).
**Desert:** dromedary/Bactrian camel ancestors (domesticable), oryx, gazelle, jackrabbit, fennec fox, coyote, vultures, rattlesnake/horned viper (**venomous**), scorpions (**venomous**, small), desert tortoise, lizards.
**Mediterranean scrub:** wild goat/bezoar ibex and mouflon (domesticable ancestors), rabbit, wild boar, partridge, eagle, vipers.
**Savanna:** zebra, wildebeest (migration), gazelles, impala, giraffe, African elephant (dangerous herbivore), cape buffalo (dangerous herbivore), warthog, hippopotamus (very dangerous), lion (**predator**, prides), spotted hyena (**predator**/scavenger), cheetah (**predator**), leopard (**predator**), African wild dog (**predator**), Nile crocodile (**predator**, water holes), vultures, ostrich, termites (mounds), baboons.
**Tropical forest:** jaguar or tiger (**predator**, by realm), leopard, tapir, peccary or forest pig, forest elephant, gorilla/chimpanzee or orangutan-like great apes (by realm), monkeys, sloths, capybara, parrots, toucans/hornbills, python/anaconda (**predator**), venomous vipers and cobras (**venomous**), poison frogs, army ants (ambient), mosquitoes.
**Mountains & alpine:** mountain goat/ibex, chamois, bighorn sheep, marmot, snow leopard (**predator**), cougar/puma (**predator**), yak (domesticable ancestor), golden eagle, bearded vulture, pika.
**Wetlands, rivers & lakes:** beaver, otter, muskrat, moose, water buffalo (domesticable), herons, cranes, ducks and geese (migration; mallard → domestic duck, greylag goose → domestic goose), swans, frogs and toads, turtles, alligator/crocodile (**predator**), catfish, carp, pike, salmon, sturgeon.
**Coasts & oceans:** crabs, mussels, clams, oysters (gatherable), sea urchins, sea otter, harbor seal, sea lion, elephant seal, seabirds (gulls, gannets, puffins, pelicans, albatross), sea turtles, herring/sardine/anchovy schools, cod, mackerel, tuna, reef fish, octopus, squid, dolphins, orca (**predator**), humpback whale, blue whale, great white shark (**predator**), tiger shark (**predator**), reef sharks, rays, jellyfish (**venomous** stings), deep-sea anglerfish, lanternfish, giant squid (very rare), bioluminescent plankton.
**Islands:** endemic birds including flightless birds, giant tortoises on remote islands, seabird colonies, few or no large mammals and predators.
**Domesticable ancestors summary (§7.7):** wolf, mouflon, bezoar ibex, aurochs, wild boar, red junglefowl (tropical forest), wild horse, wild ass, wild camel, guanaco (mountain/grassland realm), wild water buffalo, yak, mallard, greylag goose, wildcat (self-domesticating around stores).

## Appendix C — Knowledge graph seed (Eras 0–5 implemented, 6–8 planned)
Author these as nodes with prerequisites; break each into the realistic sub-steps described in §12.2. Minimum node list:
- **Era 0:** stone_as_hammer, anvil_use, nut_cracking, sharp_flake, flake_cutting, digging_stick, throwing_stones, termite_probe, chopper, hand_axe, wooden_spear, carrying_bundle, butchery, marrow_extraction, fire_keeping, fire_hardening, cooking_roasting, windbreak_shelter.
- **Era 1:** fire_by_friction_hand_drill, fire_by_friction_bow_drill, prepared_core, scraper, hafting_binding, birch_tar, pitch_adhesive, stone_tipped_spear, hide_scraping, hide_wrap_clothing, cordage_basic, ochre_pigment, lean_to, stone_boiling.
- **Era 2:** blade_technology, burin, bone_antler_working, awl, eyed_needle, sewn_clothing, cordage_advanced, netting, snares_and_traps, atlatl, bow_and_arrow, fletching, fishhook, harpoon, fish_weir, fat_lamp, basketry, raft, wolf_taming, grinding_seeds, cave_painting, drying_and_smoking, pemmican, fur_bedding, snow_shelter.
- **Era 3:** ground_stone_axe, adze, dugout_canoe, sledge, skis, seed_selection, sowing_and_tending, harvesting_sickle, threshing_winnowing, animal_herding, selective_breeding, milking, cheese_making, pottery_handbuilt, pit_firing, kiln_updraft, spindle_spinning, loom_weaving, linen, wool, pit_house, wattle_and_daub, mudbrick, timber_frame, thatching, quern, bread, fermentation, storage_pits, granary, fencing, ard_plough, draft_animals, irrigation_channels, potters_wheel, wheel_and_axle, cart.
- **Era 4:** native_copper_working, annealing, copper_smelting, charcoal_clamp, bellows, crucible, open_mold_casting, two_part_mold, lost_wax, arsenical_copper, tin_bronze, metal_saw, metal_chisel, fired_brick, lime_burning, lime_plaster, faience, early_glass, sail, plank_boat, horse_riding, plant_dyes, mineral_pigments, record_marks.
- **Era 5:** bloomery_iron, forging, carburizing_steel, quench_and_temper, iron_tools, iron_ploughshare, rotary_quern, lime_mortar, concrete, arch, vault, dome, aqueduct, cistern, crane_pulley, lathe, glassblowing, water_wheel, gearing_basic, advanced_sailing, tanning_bark (if not earlier), smithing_advanced.
- **Era 6 (planned):** windmill, magnetic_compass, mechanical_clock, gunpowder, heavy_plough, horse_collar, spinning_wheel, blast_furnace, cast_iron, printing_press.
- **Era 7 (planned):** steam_engine, railway, steamboat, photography, mechanical_reaper, telegraph, telephone, internal_combustion_engine, electric_light, automobile.
- **Era 8 (planned):** radio, powered_flight, rocketry, television, electronic_computer, nuclear_power, transistor, spaceflight, personal_computer, internet, smartphone, gene_editing, artificial_intelligence.

## Appendix D — Rocks, minerals & materials seed
**Igneous:** granite, diorite, gabbro, basalt (incl. columnar), andesite, rhyolite, obsidian, pumice, tuff, scoria. **Sedimentary:** sandstone, red sandstone, shale, mudstone, siltstone, limestone, chalk (with flint nodules), dolomite, conglomerate, breccia, chert, coal (lignite, bituminous, anthracite), rock salt, gypsum, travertine. **Metamorphic:** slate, schist, gneiss, marble, quartzite, soapstone (carving, heat-resistant molds). **Ores & minerals:** native copper, malachite, azurite, cuprite, chalcopyrite, cassiterite, hematite, magnetite (lodestone), goethite/limonite, bog iron, pyrite and marcasite (fire-starting sparks), galena, sphalerite, native gold, native silver, cinnabar (pigment; toxic), sulfur, niter, rock salt, bauxite (planned eras), kimberlite with rare diamond (ornament/abrasive only). **Earths:** clays (earthenware, kaolin, fire clay), sand (quartz, volcanic black sand, calcareous white sand), gravel, loess, peat, humus/topsoil types (§5.3), volcanic ash/pozzolana, red and yellow ochre, manganese oxide. **Organic materials:** each wood species (§6.3), bark types, plant fibres (nettle, flax, hemp, milkweed, cotton, grass, reed, bast of linden and willow), resins and pitch, birch tar, beeswax, honey, animal materials (hide, fur, leather, rawhide, sinew, gut, bone, antler, horn, ivory, feathers, fat/tallow, bladders, shells), salt, charcoal, ash, lime.

---

Begin with §0.2, then V2-0. Keep the world cohesive at every step: before adding a thing, ask what causes it, what it affects, and how the player will learn about it.
