# Light

*Status: implemented (v1). Code: `hearth_world::lighting`.*

## Model
Sky light (15 straight down through transparent blocks, then BFS spreading with attenuation)
and block light (BFS from emitters), both across cube borders, with incremental add/remove
updates and column heightmaps seeded from the generator's estimate so cubes can be lit before
the cubes above them load.

## Since v1, and still to come
- Fires, lamps and torches burn down (V2-5, `fire-and-food.md`).
- Still to come: foliage as partial shade (leaf blocks dimming sky light by species, S5) and
  light in real units (Amendment Q §2.1, with S2's lighting).
