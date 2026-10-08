# Terrain at Earth's scale: the refinement levels

*Amendment E §5.2 (E4). Status: implemented for the land's shape, its water and its rivers; the
far-field layer and the grid's recalibration are E4's other parts. Code:
`hearth_worldgen::relief` (the levels), `region::Terrain` (the blocks on them), `tools/bench`
(`bench relief`, `bench region`).*

## Purpose
The planet grid's cells are some 20 km across on Earth and a block is a metre. Between them the
land needs real shape at every scale: ranges, valleys and their tributaries, ridges and cirques,
coasts with bays and headlands, lakes, plains flat over kilometres, rivers that run all the way
down. Each scale is made from the one above, so they agree, and every value is the same whichever
part of the world is made first.

## Levels
- Each **level** is eight times finer than its parent: 2,446 m, 306 m and 38 m cells on Earth
  (as many as stay coarser than 24 m), on a grid of kilometre cells. A small test planet, whose grid is already fine, has none
  and keeps the grid's own sampler.
- A level is made a **tile** at a time: 64 × 64 cells and a margin of 16 all round, kept in an LRU
  cache per level (512 tiles). A cell's value is its tiles' blend: the tile whose core holds it,
  faded into those whose margins reach it (weight 1 − smoothstep(0, 16, distance past their
  core)). Surfaces and lake depths blend; drainage is the owning tile's.
- Making a tile (`Relief::build`), from the parent's cells about it:
  1. **Surface**: the parent's, bicubic, with relief added at the level's own wavelengths (from
     the parent's cell down to two of its own; the finest level's down to four, the blocks adding
     their own below), each octave's amplitude 160 m × (λ / 1 km)^0.7 at full relief, times how
     much relief the place takes: mountains where the rock is uplifted, hills on high land, plains
     nearly flat, abyssal hills on the deep floor, the shelf smooth.
  2. **Sea**: the parent's sea where this level's surface stays under it, flooded over the low
     ground joined to it (bays, inlets) and up rivers whose beds lie at its level (estuaries).
  3. **Lakes kept**: a parent's lake keeps its extent. About its cells' outline the ground is
     drawn to a slope through its surface (down within, up without) with the level's relief on
     it, so the shore wanders about the outline; the ground below the surface within it holds
     the same water in every tile.
  4. **Rivers kept**: the parent's rivers (from 3, 1 and 0.15 m³/s at the three levels) as
     channels along a fractal meander between the cells' jittered nodes. A bed lies below the
     parent's surface by 0.35 of the level's relief reach (keeping half its height above the sea)
     but never below the water it flows on to: the sea's level, or the lake it runs into (each
     cell knows the first water down its course). Where reaches cross, a cell drains along the one
     that ends lowest, beds are cut so each falls to the next, and no ring is left.
  5. **Drainage and wear**, eight rounds: hollows filled by Priority-Flood+ε (Barnes et al. 2014)
     or cut through where shallow enough (breaching, after Lindsay 2016: 400, 100 and 25 m at the
     three levels; never below the sea's level), the water routed down the steepest way, and the
     ground worn by implicit stream power (Braun and Willett 2013), f = 0.08 · erodibility ·
     (√(Q / Q_cell) − 1) per cell (ground draining only itself is left to slump; land wears to the
     sea's level at most). Slopes past 40° slump. A hollow's outlet wears down with all its water
     through it, so a lake drains as its outlet is cut.
  6. **Lakes**: a hollow joined to a parent's lake is that lake's; one of the level's own is a
     lake if it fits in a tile's margin (so every tile that sees it agrees) and is deeper than 3,
     1.5 or 0.8 m somewhere, or lies below the sea's level (the water table at it). A larger one
     of its own fills as a basin's flat floor.
- **Runoff** for the discharge: the grid's rain on the cell's real area, the share that runs off
  growing with rain and falling with warmth (12–75 %).

## The blocks
- `Terrain::nearby` reads, once for a rectangle of columns, the finest level's cells over it and
  its rivers (`Nearby`); `sample_with` takes each column from them: the bicubic surface, the
  blocks' own relief below the cells (wavelengths of 96 m and less, as rough as the place),
  the sea over low ground beside the level's sea, lakes to their surface, and rivers carved at
  their width, w = 4 Q^0.5 m and depth 0.27 Q^0.39 m (Leopold and Maddock's hydraulic geometry),
  with banks and a floodplain. Wide rivers come from the level above, whose margin holds their
  banks. Single samples share their square's neighbourhood (64 blocks, kept for the columns
  about it).
- **Far tiles** read the coarsest level whose cells are at most four of their columns wide
  (`nearby_scaled`, `level_for`): the finest within a few kilometres, the 306 m level out to some
  fifty, the first beyond. Searches over the whole planet (`find_biome`) read the grid's own.

## Parameters
`relief.rs`: `RATIO` (8), `TILE` (64), `MARGIN` (16), `MIN_CELL` (24 m), `TILES_KEPT` (512),
`VALLEY` (0.35), the amplitude law, `channel_threshold`, `lake_depth`, `breach_depth`, the wear
constant (0.08) and the slump angle. `region/mod.rs`: `RIVER_MARGIN` (6 cells), `DETAIL_M`
(96 m), `NEAR_SQUARE` (64), the hydraulic geometry.

## What it gives (seed 7, `bench relief`)
- Each level's mean surface within a few percent of its parent's over the same ground (3,321 m
  against 3,418 m in the mountains at the first level; equal on coasts and plains below).
- No river cell running uphill to the next at any level (0 of 32, 243 and 340 at the three sites'
  first level).
- Lakes 1–4 % of the land about the sites (Earth's lakes cover some 3.7 % of its unglaciated land:
  Verpoorter et al. 2014).
- A tile in some 23 ms; the far view of a world (2,175 LOD tiles) in about 11 s, against 9 s
  before.

Review images: `docs/review/e4/relief_levels.jpg` (the levels about three sites),
`mountains_before.jpg` and `mountains_after.jpg` (a spawn in the mountains, 600 blocks up).

## Tests
`relief::tests`: Earth has levels down to tens of metres and a test planet none; a cell has one
value whichever tile is made first; rivers fall all the way down through every level; each level
keeps its parent's surface on the whole. `region::tests`: a refined planet reads the same however
it is sampled (alone, in a batch, across the seam); the blocks' rivers run down and far tiles read
coarse cells.

## Known simplifications
- The grid's heights are Earth's on the whole (`planet.md`), short in the middle heights; its
  relief field (uplift) drives how rough the levels are.
- Small streams of a level's own drainage may shift a little at a tile's edge (the tiles agree on
  the surface and on every river kept from above).
- A lake kept from the parent stands at the parent's surface even where the finer ground would
  hold it lower; a river beside a lake and below its surface stays a river.
- No glaciers shape the ground (cirques and U-shaped valleys), and no sediment builds deltas.

## Sources
- Barnes R., Lehman C., Mulla D. 2014, "Priority-Flood: an optimal depression-filling and
  watershed-labeling algorithm for digital elevation models", *Computers & Geosciences* 62: 117–127.
- Lindsay J. B. 2016, "Efficient hybrid breaching-filling sink removal methods for flow path
  enforcement in digital elevation models", *Hydrological Processes* 30: 846–857.
- Braun J., Willett S. D. 2013, "A very efficient O(n), implicit and parallel method to solve the
  stream power equation governing fluvial incision and landscape evolution", *Geomorphology*
  180–181: 170–179.
- Leopold L. B., Maddock T. 1953, *The hydraulic geometry of stream channels and some
  physiographic implications*, USGS Professional Paper 252.
- Verpoorter C., Kutser T., Seekell D. A., Tranvik L. J. 2014, "A global inventory of lakes based
  on high-resolution satellite imagery", *Geophysical Research Letters* 41: 6396–6402.
