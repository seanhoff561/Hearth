# Planet generation

*Status: implemented (v1 M2). Code: `crates/hearth_worldgen`. Details: `ARCHITECTURE.md` §5,
`DECISIONS.md` D8–D17.*

## Purpose
A whole Earth-like planet whose geography, drainage and climate arise from physical causes,
wrapping east–west on a Mercator grid with poles.

## Model
- **Planet grid** (N×N Mercator cells with sphere metrics): warped weighted spherical-Voronoi
  plates with Euler poles; coasts from active/passive margins; elevation from collisions,
  subduction arcs, rifts, hotspots, old ranges and ocean-floor subsidence; Priority-Flood
  depression filling and implicit stream-power erosion; lakes vs endorheic basins by water
  balance.
- **Climate**: currents per ocean basin, upwind continentality, zonal moisture advection with
  orographic lift and rain shadows, dry-season belts, Köppen classes, lapse-rate temperature.
- **Refinement levels** (an Earth-sized planet; E4): 2.4 km, 306 m and 38 m cells between the
  grid and the blocks, each made from the one above with its own relief, rivers, lakes and coasts
  ([terrain.md](terrain.md)).
- **Regional sampler**: block-resolution height (the finest level's surface and the blocks' own
  relief; on a small test planet, bicubic grid + detail noise), meandering rivers with
  floodplains, lakes, coasts, 36 biomes from climate + altitude + local conditions.
- **Cube generation**: pure and order-independent (priority lattice, 27-neighbour seeding):
  rock by depth, caves, rock-variety veins, trees and plants.

## At Earth's size (E4)
The elevation model's heights are Earth's real ones on an Earth-sized planet and blend back to
the small planets' tuning as the planet shrinks (`Scale::earth`, so the test planets keep their
shape): interiors some 500 m up, the greatest ranges some 6.5 km over their 20 km cells with a
4.6 km plateau behind, ordinary and Andean ranges 3–5 km, old ranges 1.6–2.1 km, trenches 7–11 km.
`bench hypso` measures a grid against Earth:

| | Earth | seeds 7, 8, 9 (2048²) |
|---|---|---|
| Land | 29.2 % (Kossinna 1931) | 29.0, 29.1, 29.2 % |
| Mean land height | 797–840 m (Eakins and Sharman 2012; Kossinna) | 791, 599, 637 m |
| Mean ocean depth | 3,682 m (NOAA) | 3,640, 3,562, 3,544 m |
| Highest 20 km cell | ~6 km (the Himalaya, Tibet) | 6.4, 6.0, 5.8 km |
| Deepest cell | ~10.9 km (the Mariana) | 8.2, 7.7, 7.2 km |
| Surface above 2 km | some 3–4 % | 1.6, 0.4, 0.8 % |

Short of Earth: the middle heights (1–3 km), as ranges are few and narrow at the grid's scale
and its erosion flattens them, and the deepest trenches at 2048² (512² reaches 10.4 km). Test:
`planet::stats::an_earth_sized_planet_has_earths_hypsometry` (three seeds at 512²).

## Interactions
Feeds geology provinces (V2-2 derives them from the tectonic history), soils (climate ×
parent rock), ecosystems (biomes → ecological cells), seasons (V2-1 adds seasonal amplitude).

## Known simplifications (to be replaced)
Rock palette, caves' stone and the tree/plant set are v1 placeholders until V2-2 (geology) and
V2-6 (flora). Ore bands were removed in V2-0.
