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
- **Regional sampler**: block-resolution height (bicubic grid + detail noise), meandering rivers
  with floodplains, lakes, coasts, 36 biomes from climate + altitude + local conditions.
- **Cube generation**: pure and order-independent (priority lattice, 27-neighbour seeding):
  rock by depth, caves, rock-variety veins, trees and plants.

## Interactions
Feeds geology provinces (V2-2 derives them from the tectonic history), soils (climate ×
parent rock), ecosystems (biomes → ecological cells), seasons (V2-1 adds seasonal amplitude).

## Known simplifications (to be replaced)
Rock palette, caves' stone and the tree/plant set are v1 placeholders until V2-2 (geology) and
V2-6 (flora). Ore bands were removed in V2-0.
