# Geology, soils, hydrology and resources

*Status: planned (V2-2); seed data in V2-0 (`data/hearth/geology`, `data/hearth/materials`).*

## Purpose
Real rocks, soils, water and resources placed by physical causes so players can read the land
(v2 §5, §13).

## Data (V2-0 seed)
Rock types, minerals, nine provinces with stratigraphic sequences and basements, seven deposit
models (toolstones, pyrite, ochre, clay), four soils. Materials carry density, hardness,
fracture, knapping quality, strengths, thermal and fuel properties.

## Planned model
- Provinces assigned from the tectonic history (craton, fold belt, arc, hotspot, rift, basins,
  passive margin, ocean floor); layers with thickness and dip, outcropping in cliffs.
- Basement below the sequence; geothermal gradient by depth.
- Soils from climate × parent rock × vegetation × drainage × slope with horizons.
- Groundwater table, springs, seasonal rivers, finite player-moved water.
- Deposit bodies placed by the vein/nodule mechanism already in `cubegen::veins`, with surface
  indicators and panning.
- Blocks generated from materials × block forms (replacing the v1 block pack).
