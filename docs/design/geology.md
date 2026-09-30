# Geology, soils, hydrology and resources

*Status: partial (V2-2 in progress). Implemented: rock types as generated blocks, geological
provinces, stratigraphy, basement, intrusions, geothermal gradient (`hearth_worldgen::geology`,
`hearth_content::generate::natural_blocks`, `hearth_texgen::material`). Planned in V2-2: soils,
groundwater and springs, seasonal rivers, finite water, coasts, deposits and indicators,
surface detail, water rendering. Decisions: D38–D40.*

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
2. **conditions** filter the provinces of that setting (arid < 450 mm, humid > 750 mm, warm and
   cold sea-level temperature, coastal/inland, old/young orogen) — so red beds and evaporites
   form in dry basins, coal measures and chalk in wet ones, carbonate platforms on warm
   margins;
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

**Plutons**: on a 1,600-block grid, a third of the cells hold a pluton of the host province's
intrusive rock (granite, diorite, gabbro), 120–500 blocks across, a dome whose roof may rise
above the surface (granite landscapes) or stay buried.

**Geothermal gradient**: rock temperature = mean surface temperature + 25 °C per real km
(`Geology::rock_temperature_c`), so deep mines are hot (used by physiology, V2-3).

## Tools
`bench worldmap` renders province and surface-rock maps of the whole planet and, with
`--geo-area x,z,size`, a block-scale outcrop map and an east–west cross-section of an area.

## Interactions
Rock → soils (parent material, V2-2), deposits and indicators (V2-2), hardness and material
of every mined block (V2-5 tools and mining), heat at depth (V2-3), building stone (V2-8).

## Known simplifications
- One sequence per province; no unconformities, no faults inside a province (province borders
  act as faults), no overturned folds or thrust stacks.
- Plutons are domes without contact metamorphism.
- Bedding inside a formation is only in the texture.
- Region choice uses today's climate as a stand-in for the climate the rocks formed in.
