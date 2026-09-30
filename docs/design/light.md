# Light

*Status: implemented (v1). Code: `hearth_world::lighting`.*

## Model
Sky light (15 straight down through transparent blocks, then BFS spreading with attenuation)
and block light (BFS from emitters), both across cube borders, with incremental add/remove
updates and column heightmaps seeded from the generator's estimate so cubes can be lit before
the cubes above them load.

## v2 changes (planned)
- Foliage becomes partial shade: leaf blocks reduce sky light by a species-dependent amount
  instead of blocking it (V2-6).
- Consumable light sources: fires, fat lamps and torches that burn down (V2-5).
- Glowing hot items and embers emit light by temperature (V2-5).
