# Geology, soils, hydrology and resources

*Status: partial (V2-2 in progress). Implemented: rock types, soils and sediments as
generated blocks, geological provinces, stratigraphy, basement, intrusions, geothermal
gradient, soils, loose stones and scree, deposits of every Appendix D resource with their
surface indicators, panning, the per-continent resource coverage, groundwater, springs, water
quality, coasts (reefs, mangroves, salt marsh, mudflats, kelp, rocky shores), sea ice and
seasonal rivers (`hearth_worldgen::{geology, soil, deposits, coverage, hydro}`,
`hearth_content::generate::natural_blocks`, `hearth_texgen::material`,
`hearth_env::{climate, rivers}`, `hearth::season_cover`). Planned in V2-2: finite
player-moved water, water rendering. Decisions: D38–D51, D55.*

## Purpose
Real rocks, soils, water and resources placed by physical causes, so a player can read the
land: follow a limestone bed along a valley side, find the granite core of a mountain range,
know that red cliffs mean a dry basin and that flint lies in chalk (v2 §5, §13).

## Rocks as data (D38)
Every rock type in `geology/rocks.ron` (33: igneous, sedimentary, metamorphic, coals, salt and
gypsum) becomes a block of the same name with no code: the block loader asks the content for
its natural blocks (`natural_blocks`), gives each the `rock` template, a hardness from the
material's compressive strength (chalk ≈ 0.6, limestone 1.5, granite 2.7, basalt 3.7) and the
material's map colour, and records the material on the block. The texture is drawn from the
material's appearance (colour, second colour, pattern: crystalline, speckled, layered, banded,
veined, glassy, porous, clumpy…). A pack's JSON may still override any generated block.

## Provinces (D39)
`geology/provinces.ron` holds 14 provinces with their tectonic setting, stratigraphic
sequence, basement, intrusions, structure (folded or not, dips) and selection conditions.
Every planet grid cell (at half resolution) gets one:
1. its **setting** comes from the planet's tectonic history: deep ocean → ocean floor, volcanic
   arcs, other volcanic terrain → hotspot, shields → craton, young and old orogens → fold
   belts, rifts, and the remaining basins → passive margin near coasts or sedimentary basin
   inland;
2. **conditions** narrow the provinces of that setting: structural ones (coastal/inland,
   old/young orogen) decide which may occur; climate ones (arid < 450 mm, humid > 750 mm,
   warm and cold sea-level temperature) are preferences — a province keeps 15% of its weight
   where they do not hold (D45) — so red beds and evaporites favour dry basins, coal
   measures and chalk wet ones, carbonate platforms warm margins, without being confined to
   them;
3. a **weighted hash of a warped region** (1/64 of the circumference) picks one, so provinces
   form coherent regions. Columns take the province of the nearest jittered cell site after a
   warp, so borders are irregular.

## Stratigraphy
At each column the sequence's top lies at the **regional surface** (the planet's elevation,
quarter resolution, blurred) plus 8 blocks, plus the **exhumation**: 0.6 × the uplift (metres ×
vertical scale) — the thickness erosion has stripped from uplifted land. Mountain cores
therefore expose deep layers and the basement; basins keep their youngest rocks on top.
Layers take their thickness (real metres × vertical scale, like relief) from a smooth noise
within their range and pinch out where a presence noise says they are absent. **Folded**
provinces bend the layers across an axis (wavelength 300–900 blocks, amplitude from the dip,
the phase bent by noise); others warp into irregular domes and basins (wavelengths of
kilometres). Depth below the top selects the layer; below the sequence lies the basement.

**Plutons**: on a 1,600-block grid, a share of the sites set by the host province (a third by
default, 80% in shields, 60% in old orogens) hold a pluton of its intrusive rock (granite,
diorite, gabbro), 120–500 blocks across, with a flat roof and steep walls. The roof lies at a
depth the province sets (D46): deeply eroded shields and old orogens expose their granites
(granite landscapes, kaolin and tin), young belts keep them buried.

**Geothermal gradient**: rock temperature = mean surface temperature + 25 °C per real km
(`Geology::rock_temperature_c`), so deep mines are hot (used by physiology, V2-3).

## Soils (D41)
`geology/soils.ron` holds 16 soils (chernozem, brown forest soil, podzol, alluvial, red
tropical, laterite, loess, peat bog, gley, andosol, rendzina, terra rossa, desert, tundra with
permafrost, mountain lithosol, and a young-soil fallback), each with horizons (material and
thickness) and a formation. For every land column the generator scores the soils whose listed
conditions all hold: climate (exact Köppen code 3, group 2), parent rock (the geology's rock
under the soil, 3.5 — soils made by their rock win where it is), landform (floodplain, wetland:
4) or vegetation (2, −2 if the soil lists vegetation and none fits), drainage (1) and maximum
slope, with a small hash so soils that fit equally well mix in patches. **Drainage** comes
from slope, the parent rock's porosity, closeness to a river, climate and wet hollows on flat
ground. Horizons are real metres (a block is a metre), thinner than half a block are part of
the turf, thinned by slope (full on flats, gone on steep ground, where treeless slopes are
scree and wooded ones keep a stony soil); turf, podzol turf or moss replaces the top block
where the biome is vegetated. Beaches take black sand from volcanic rock, shell sand from
warm carbonate coasts and quartz sand elsewhere; river and lake beds are gravel, sand, clay
and mud; salt flats a salt crust over mud; glaciers snow and ice.

Soils and sediments are blocks generated from their materials (every Soil, Clay and
Sediment material, plus materials of thick horizons such as peat); loose ones (tagged
`falls`) fall. They replace v1's dirt, coarse dirt, rooted dirt, mud, clay, sand, red sand
and gravel.

## Surface detail
Every true rock has a generated `<rock>_cobbles` block (a few stones on the ground, replaceable,
so snow buries them). The feature pass scatters the local bedrock's cobbles by surface and
setting: bare rock, scree and gravel (10 %), deserts and high ground (×2.5), stony shores (×3),
more on slopes, a few anywhere. Boulders are the local bedrock (or mossy cobblestone in wet
climates). Rock outcrops follow the stratigraphy.

## Deposits (D42, D43)
`geology/deposits.ron` holds 52 deposit models covering every Appendix D resource: toolstones
(flint nodule bands in chalk and limestone, chert beds, obsidian flows, quartzite and basalt
river cobbles, quartz veins), fire stones (pyrite nodules and veins, marcasite in chalk),
pigments (red and yellow ochre, manganese black), clays (river banks, weathered mudrock,
fire clay under coal and lignite, kaolin from rotted granite and in beds), salt (pans, rock
salt lenses, salt springs) and gypsum (beds, desert crusts), copper (oxidised caps, cuprite,
native copper in basalt, porphyry, sulfide veins), tin (veins near granite, stream tin
downstream of it), iron (bog iron, hematite, magnetite, limonite, laterite), gold (quartz
veins, placers), lead–silver, native silver, zinc, sulfur (fumaroles, in gypsum), cinnabar
(hot springs, veins), saltpetre, travertine, pozzolana, kimberlite, garnet, bauxite and coals.

**Placement.** Each 256-block cell draws a Poisson number of candidates per model (frequency ×
area); a candidate becomes a body where its province, conditions and host rock hold: place
conditions (river, lake, wetland) are searched for inside the cell, climate conditions read
the column, "volcano" means volcanic terrain or a volcano within 1.2 km, "granite" a granite
pluton under or near the place (500 blocks for placers, which wash down from it). Buried
bodies try six depths in their range (real metres × vertical scale) for a host rock. The
cell's bodies are cached and every cube draws the ones that reach it.

**Shapes.** Veins are steep plates (random strike, dip 55–90°) of their thickness; seams and
evaporites flat lenses with a gentle undulation; nodules three bands of scattered nodules;
disseminated bodies ellipsoids where 45% of the host takes the ore; pipes vertical cylinders
to the surface; crusts, flows and bog ores follow the ground (crusts break into blotches
toward their edges, bog ore lies a block under the peat); placers lie along river beds as
gravel with heavy grains, or as cobbles on dry bars for rock resources. Bodies replace only
their host rocks (or any rock and soil when none is listed).

**Indicators.** Shallow bodies (top within 16 blocks of the ground) show at the surface:
stains and gossans (the indicator mineral's ore block on about a third of the ground over
the body), float (loose pieces scattered around within 1.5× the body's size, at least 24
blocks) and their own crusts and outcrops. Each body records the lowest and highest ground
within its reach, so indicators on slopes are drawn in every cube they fall in.

**Blocks.** Every deposit brings its blocks, generated from the content (D43): `<mineral>_ore`
(rock matrix flecked with the mineral, glinting where it is metallic), `<mineral>_crust`,
`<mineral>_placer`, `<mineral>_float`, `<rock>_cobbles`, and the earths and rocks
themselves.

## Panning
`Deposits::pan(x, z)` is what a pan of river gravel (8 kg) washed at a bed or bank holds: the
heavy grains of the placers it lies in, grade × mass, richest in the middle of a placer reach
(a pan of placer gold holds tens of milligrams; stream tin tens of grams), plus trace indicator
grains shed by bodies upstream within 300 blocks (garnets below a kimberlite). The panning
action and its items arrive with the process engine (V2-5) and ore processing (V2-13).

## Resource coverage (D44)
`hearth_worldgen::coverage` takes a census of every body on the planet (≈0.1–0.5 s at
Standard), keeps the ones workable in their resource's era (≤ 4 blocks deep in Eras 0–2, 12 in
Era 3, 40 later), labels continents (landmasses ≥ 50 km² at Standard) and computes, per
continent and resource, the distance within which 90% of the land lies from a workable body.
Reach limits are 5 km (Eras 0–2), 8 km (Era 3) and 12 km (Eras 4–5) at Standard; a gap is
covered when a substitute (same metal, same formula, shared `_ore` or process tag) is within
reach. Across seeds 1–6 at Standard (11 continents), every Era 0–2 need (toolstone, fire
stone, ochre, clay) is within reach of every continent; the open gaps are regional
resources: kimberlite (8 continents), bituminous coal (6), fire clay (3), travertine and
volcanic ash (3 each), sphalerite, sulfur and kaolin (2 each), and single cases of lignite,
cinnabar, gypsum and salt. `hearth content lint --coverage [--seed N]…` flags them.

## Groundwater (D48)
`hearth_worldgen::hydro`: the **water table** is a subdued copy of the land. From the drainage
base (the lowest ground or water surface within 256 blocks, from the planet grid) it rises
under the land smoothed over ~100 blocks by a share of the relief set by the surface rock's
**permeability** (data on every rock: tight crystalline rock 0.9, shales and slates 0.8, fair
0.7, sandstones and lavas 0.5, karst 0.12) and by the climate (wetness from precipitation less
evaporation: wet climates hold it high, dry ones sink it up to 24 blocks below the valley
floors). It meets rivers, lakes and the sea at their surfaces and stays below the ground. In
humid lowlands it lies a few blocks down (median ≈ 3), under hills deeper, in deserts tens of
blocks. **Every cave void below it is water**: worm caves and ravines flood up to the table at
their start, caverns up to the table or their own lake, so dry caves are found in hills,
karst uplands and dry country, and lowland caves are sumps.

## Springs and water quality
Springs rise where the water table would come out of the ground on a slope (spring lines at
the foot of hills and valley sides: about 1.4 per km² of humid land, 0.5 semi-humid, 0.2
semi-arid), as rare oases in desert hollows, and over the deposits that show as springs (salt
springs over salt beds, hot springs by volcanoes, sulfurous fumaroles, travertine springs).
Each has a pool and a brook that follows the steepest way down until it meets other water,
finds a hollow or soaks away (longer in wet climates). `Hydrology::quality(x, y, z)` gives the
natural water at a block: sea (35 g/L, brackish off river mouths), fresh or salt lakes (closed
basins keep the salt of evaporation), streams (small cold ones safe, broad warm lowland rivers
risky and muddy), springs (safe; salty, sulfurous, hot or mineral by kind) and the groundwater
a well reaches (safe, brackish in deserts).

## Seasonal rivers (D55)
Rivers rise and fall through the year with the water of their basins (`hearth_env::rivers`).
Each river cell of the planet grid runs the year-scale water balance of its own climate
(`SeasonalCover`: rain and snow, melt from a landscape of slopes 4 °C warmer to 4 °C colder,
evaporation, a storm share of rain that runs off anyway, and drainage through a fast store
(15 days) and the ground (120 days; the ground's share falls with the groundwater's wetness, so
streams of dry lands run dry)). A reach's regime is the mix of everything upstream, weighted by
the water each part adds (its gain in grid discharge times its runoff ratio) and delayed by the
flood wave's travel time (1.5 m/s over the real distances the planet stands for). The mix is
done in the frequency domain, where a delay is a phase shift, over all 36 harmonics of the
73-step year, and each reach keeps its year as a series (smoothed over one step so shifted
floods do not ring): 22,000 reaches on a Standard planet in 0.07 s. So snowmelt rivers flood
in spring (up to 6–8× their mean in cold dry country) and nearly stop in frozen winters,
savanna rivers run high in the wet season and fall to a fifth of the mean in the dry, oceanic
rivers are high in winter and low in late summer (about 4:1), and a great river crossing a
desert floods with the rains of its distant highlands, weeks later.

On the terrain (`season_cover.rs`) a river column's level follows its reach's flow: depth
grows as flow^0.6 (Manning); floods over the banks (above twice the mean flow or so) rise half
as fast and stand at most half a block plus a quarter of the channel's depth over them, and
where they would stand above the land around the river they thin toward the edge of the
floodplain to meet it, so there are no walls of water. Low water bares bars and banks; small
rivers (under 8 blocks) stop below 0.15× their mean and any river below 0.02×. The sea holds
river mouths up. Drowned plants (and water plants left dry) are remembered and come back, and
everything returns to the mean level when the cover is refreshed.

## Finite water (D61)
`hearth_world::water::WaterSim`. **Natural water** — the sea, lakes, rivers, springs, flooded
caves (`water[level=0]`, and plants standing in it) — is a sustained reservoir that the
hydrology holds at its level, and is never simulated. **Finite water** is what the player moves:
each block of it holds 1–1000 litres with its quality (salinity, germ risk, temperature), kept
by the simulation, and shows its volume in eighths (`water[level=1..8]`). Every tick (about ten
a second) it falls; spreads by evening out with each lower neighbour in turn (half of each
difference, the order alternating so it spreads alike every way, films under 5 mm staying
put); and natural water feeds the open blocks beside it at its height or under it (a channel
dug from a river fills to the river's level with the river's water; a pit dug under a lake bed
floods) and takes in water that comes down onto it (a channel drains into the sea). Evening out
stops at steps of a litre, which add up along a channel, and cannot pass through full water,
so once the water is at rest each connected body takes one level: its water fills its blocks,
and the open blocks right over them, from the bottom up, each level shared evenly — connected
wells reach the same height, a pond is flat — or, touching natural water, stands at its
surface. Only blocks that changed and their neighbours are looked at, so water at rest costs
nothing; volumes are integer litres, so water is conserved exactly (the budget counts what the
reservoirs fed and drained, the air took, the player poured and took, and what a block put in
its place spilled). Over days (`weather`): neighbouring water mixes (halfway in half a day);
**groundwater** seeps into holes below the water table at the rate the rock allows (tight rock
10 L a day per block of hole, fair 300, sandstone and gravel 1500, karst 4000 — what hand-dug
wells yield), so a well fills to the table with groundwater; water under the open sky
**evaporates** by warmth and the air's drying power (`water_env::open_water_evaporation`: about
10 mm a day in hot, dry, windy weather, 1–2 mm in mild humid weather, under 1 mm in cool damp
weather), keeping its salt — past saturation
(360 g/L), and all of it once the water is gone, the salt is left as a **salt crust** (a thin
`salt_crust` block holding its kilograms; water poured back dissolves it); water takes the air's
temperature (a full block in about a day); and water still for half a day grows germs toward a
level that rises with warmth. `hearth::water_env::WorldWater` gives the simulation the world:
natural water's quality from `Hydrology::quality`, the water table, seepage by rock, and the
weather's evaporation.

## Coasts (D49, D50)
A slow noise and every river mouth mark **sheltered** coasts (bays, estuaries, lagoons; about
40 % of them). Low sheltered coasts (under 1.5 blocks, slope under 5 %) are **mangrove** in the
tropics (air above 20 °C, sea above 22 °C) — dense trees on arching prop roots standing in up to
two blocks of water — and **salt marsh** elsewhere (cordgrass on mud); their shallows are
**mudflats**. On hot desert coasts (and hot steppe drier than 350 mm) the low sheltered flats
are **coastal salt pans** (sabkhas, as on the Persian Gulf, in Baja California and at Shark
Bay): the sea floods them now and then and evaporates, leaving a crust of rock salt over mud —
a source of salt for the coasts of dry lands, and the place to make more by flooding pans
(finite water evaporates to a salt crust). **Coral reefs** grow in warm (above 21 °C), clear (not near river mouths or in
muddy bays), shallow sea: fringing reefs 50–160 blocks out from the shore, barrier reefs
300–600 blocks out with a lagoon behind, and atolls ringing the drowned hotspot volcanoes; the
reef raises the floor to 1–3.5 blocks below the surface (coral heads and grooves), living
coral over reef limestone, with fans and branching corals on top. **Kelp** forests hold to
rocky and gravel floors in cool water (5–20 °C), thick in patches; **wrack** grows on the rocks
of cool shores; **tide pools** fill hollows in the rock of stony shores. **Sea ice** (in the
seasonal cover, `hearth_env::climate`): sea water freezes at −1.8 °C and the sea's heat keeps
it open until the air is below −4 °C, so the same growth law as lake ice from that threshold
gives perennial pack ice in the high Arctic, seasonal ice in subarctic bays and open water on
coasts whose winters are only just below freezing.

## Tools
`bench worldmap` renders province, surface-rock, soil and deposit maps of the whole planet
(the soil map in the colours of soil maps: black chernozem, red tropical soils, blue alluvium,
purple peat; sediments, bare rock and ice set apart), the land share of every province and
soil, the bodies per model and the coverage table, and, with `--geo-area x,z,size`, a
block-scale outcrop map and an east–west cross-section of an area. `bench deposits [--model id]
[--near x,z] [--max-depth n] [--coverage] [--springs] [--rivers min_width] [--find
biome|coral]` lists a world's
bodies, springs or columns of a biome nearest a point (with their depth, size, grade, biome and
climate) for inspection and screenshots (`tools/shots/v22_deposits.shots`,
`tools/shots/v22_coasts.shots`).

## Acceptance (V2-2)
- `bench worldmap` gains geology (province and surface-rock), soil and deposit layers
  (`bench-out/worldmap/equirect_{province,rock,soil,deposits}.png`). On seed 1 at 512² the
  soils cover the land much as Earth's orders do: brown forest soils 20 %, desert soils 18 %,
  tundra soils 14 %, red tropical soils 11 %, podzols 8 %, loess and alluvium 4 % and 3 %,
  chernozem 3 %, bare rock, ice and loose sediments 12 % between them (the grid's cells
  smooth away the steep ground where mountain lithosols lie).
- Deposits occur only in their provinces, host rocks and climates, at the frequencies the
  content gives (`crates/hearth_worldgen/tests/deposits.rs`).
- Every Era 0–2 need is within reach of every continent on seeds 1–6 at Standard size
  (tested), and `hearth content lint --coverage` flags each continent's gaps for eras 0–5:
  as notes where a resource of the same use stands in, as warnings where none does (rock
  salt and gypsum on some continents; sulfur, sphalerite, travertine, coal, fire clay,
  kaolin, volcanic ash and kimberlite are regional, as on Earth).
- The rest of the milestone's list — groundwater and springs, seasonal rivers, finite water,
  reefs and coasts, surface indicators, panning — is covered by the tests named in the
  sections above and the shot lists `tools/shots/v22_*.shots`. The minimal globe spawn picker
  (§16, for testing; finished in V2-15) opens with the world-map key (M) in the preview and
  as `--screenshot "globe=1"`.

## Interactions
Rock → soils (parent material, V2-2), deposits and indicators (V2-2), hardness and material
of every mined block (V2-5 tools and mining), heat at depth (V2-3), building stone (V2-8).
Deposits → prospecting, panning and mining (V2-5, V2-13), the knowledge of eras 0–5 (V2-5,
V2-12–V2-14), trade distances between continents; springs and water taste over salt and
sulfur bodies (V2-2d).

## Known simplifications
- One sequence per province; no unconformities, no faults inside a province (province borders
  act as faults), no overturned folds or thrust stacks.
- Plutons are domes without contact metamorphism.
- Bedding inside a formation is only in the texture.
- Region choice uses today's climate as a (soft) stand-in for the climate the rocks formed
  in.
- Deposit bodies are independent of one another: no zoning (oxide cap over sulfide ore over
  primary vein) inside one system, no ore shoots, and placers do not trace back along the
  river network to their source body.
- Frequencies are per km² of suitable ground at game scale (D42), not Earth's densities.
- The water table is a static field: no seasonal rise and fall, no perched tables, no
  confined aquifers or artesian pressure; brooks are natural water (static channels).
- Finite water levels out only through connected water at rest (no flow pressure while it
  moves), does not soak into the ground, freeze or wash soil away, and leaves waterlogging of
  slabs and fences for later; the world loop (V2-3) must tell it about blocks the seasonal
  cover changes next to finite water.
- No tides yet: the "intertidal" is a band a block or two either side of sea level.
- Sheltered coasts come from noise and river mouths, not from the coastline's shape.
- The Köppen classes of the planet are coarse (e.g. one "Dfb" for all humid continental), so
  soils listing finer codes match by group.
- Soil horizons are whole blocks; litter layers, texture classes and nutrient dynamics wait
  for flora and farming (V2-6, V2-12).
