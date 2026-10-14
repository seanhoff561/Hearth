# Terrain at Earth's scale: the refinement levels

*Amendment E §5.2 (E4), shaped by Amendment T's G1a (D299, relief from 30 m to 30 km) and G1b
(D301, walking scale). Code: `hearth_worldgen::relief` (the levels), `region::Terrain` (the
blocks on them), `tools/bench` (`bench relief`, `bench realism`).*

## Purpose
The planet grid's cells are some 20 km across on Earth and a block is a metre. Between them the
land needs real shape at every scale: ranges, valleys and their tributaries, hillslopes and the
gullies cut into them, coasts, lakes, plains flat over kilometres, rivers that run all the way
down, and the ground underfoot. Each scale is made from the one above, so they agree, and every
value is the same whichever part of the world is made first.

## Levels
- Each **level** is eight times finer than its parent: 2,446 m, 306 m and 38 m cells on Earth,
  and the **walking level**'s 4.8 m (no finer than 4 m), on a grid of kilometre cells. A small
  test planet, whose grid is already fine, has none and keeps the grid's own sampler.
- A level is made a **tile** at a time: 64 × 64 cells and a margin of 16 all round, kept in an LRU
  cache per level (its share of the terrain tiles' memory budget) and on disk
  (`relief_store`, format 4). A cell's value is its tiles' blend: the tile whose core holds it,
  faded into those whose margins reach it (weight 1 − smoothstep(0, 16, distance past their
  core)). Surfaces and lake depths blend; drainage is the owning tile's.
- **How much relief a place takes** (`relief/roughness.rs`, G1a): an amplitude at its valleys'
  spacing from its height above base level (the graded profile its rivers could cut to), its
  rain, dry land, ice, province and mountains' uplift, with a spectrum ∝ λ^0.95 below the spacing
  and λ^0.2 beyond; the spacing as far as hillslopes of the place's gradient take to rise
  through it. Below 76 m soil-mantled ground's relief falls away faster (creep rounds it: Perron
  et al. 2008); bare rock, young glacial ground and dry land keep theirs.
- Making a tile (`Relief::build`), from the parent's cells about it:
  1. **Surface**: the parent's, bicubic, with the level's octaves added (from the parent's cell
     down to two of its own; the walking level's parent to four, the walking level from twice
     its parent's cell to four of its own). Each octave is laid across the slope beneath it
     where that slope falls further over its wavelength than its relief reaches
     (`relief/gullies.rs`: a lattice of gullies running downhill, scaled to the isotropic
     noise's spread), so ridges and valleys run down the slopes instead of standing as bumps.
     Dry land's basins bury the 300 m and 40 m levels' relief (what the ranges shed); the
     walking level's is the fill's own surface, washes, bars and swales (Bull 1977).
  2. **Sea**: the parent's sea where this level's surface stays under it, flooded over the low
     ground joined to it (bays, inlets) and up rivers whose beds lie at its level (estuaries).
  3. **Lakes kept**: a parent's lake keeps its extent; about its outline the ground is drawn to a
     slope through its surface with the level's relief on it, so the shore wanders.
  4. **Rivers kept**: the parent's rivers (from 3, 1, 0.15 and 0.0005 m³/s at the four levels:
     at the walking level the brooks and first-order channels) along a curve through the parent's
     nodes (`relief/courses.rs`), wandering by slope and discharge. A bed lies below the parent's
     surface by 0.6 of the relief the level adds at the parent's scale, at or above every bed
     downstream, never below the water it flows on to. The walking level's channels step only
     along its rows and columns: one stepping across a corner between two banks would rise
     between its cells as the blocks read them.
  5. **Drainage and wear**, eight rounds: hollows filled by Priority-Flood+ε (Barnes et al. 2014)
     or cut through where shallow enough (Lindsay 2016), the water spread to every lower
     neighbour by slope and contour width (Quinn et al. 1991), the ground worn by implicit stream
     power (Braun and Willett 2013), hillslopes creeping (0.05, 0.12 and 0.25 of the nine-point
     Laplacian a round below the first level) and slumping past 40°. The walking level drains
     along its rows and columns (the blocks read between its cells), and its erosion reckons
     with the storms' runoff on dry land's bare ground (0.3 of the rain at 150 mm and less: Yair
     and Lavee 1985), where real dry lands drain densest; the discharge it keeps is the mean's.
  6. **Lakes**: a hollow joined to a parent's lake is that lake's; one of the level's own is a
     lake if it fits in a tile's margin and is deep enough (3, 1.5, 0.8 and 2 m), else it fills.
- **The walking level's second pass** (`relief/walking.rs`): two tiles may lay the same hollow's
  way out differently in their overlap, and their blend there can dam it, so the level's blended
  surface is drained again over tiles offset half a tile from the first's.
- **Runoff** for the discharge: the grid's rain on the cell's real area, the share that runs off
  growing with rain and falling with warmth (12–75 %).

## The blocks
- `Terrain::nearby` reads, once for a rectangle of columns, the finest level's cells over it and
  its rivers; `sample_with` takes each column from them: the cells' surface read between them
  (bicubic, its slopes held so that it never leaves the range of the cells about it: plain
  Catmull-Rom dips below a valley's floor two cells wide by an eighth of its sides' rise), the
  sea and lakes, rivers carved at their width (w = 4 Q^0.5 m, depth 0.27 Q^0.39 m: Leopold and
  Maddock 1953) with banks, and **the ground's own roughness** below the cells
  (`Terrain::micro_relief`): soil's mounds and pits (0.65 m) smooth on the flattest ground; on
  dry land's slopes stony knobs that ring no hollow (to 4 m by outcrop and slope); bare rock
  above the trees and on cliffs (6 m); none on a channel's bed.
- **Only the blocks about the players make the walking level's tiles**; a world's first ones are
  made side by side about the player (`prefetch_walking`, 160 blocks). Every other read takes
  its stand-in (`Relief::walking_proxy`): its octaves over its parent's surface, without its
  drainage, those finer than the columns' spacing faded out as a DEM's samples average them.
  - **Far tiles** read the coarsest level whose cells are at most four of their columns wide
    (`nearby_scaled`, `level_for`), the stand-in on the 38 m level's cells.
  - **Placement** (deposits, springs, caves, a place's card: `sample_regional`) reads the 38 m
    level at the blocks' scale with the stand-in and the micro relief, within some metres of the
    blocks' own (`REGIONAL_SLACK_M`, 24 m; a fifth of a metre at the median); what it places on
    the blocks' surface is looked for within that slack.
  - Searches over the whole planet read the grid's own.

## Parameters
`relief/mod.rs`: `RATIO` (8), `TILE` (64), `MARGIN` (16), `MIN_CELL` (4 m), `WALKING_CELL`
(10 m), `LATTICE` (8), `VALLEY` (0.6), `SMOOTH_M` (76 m), `STORM_SHARE` (0.3),
`channel_threshold`, `lake_depth`, `breach_depth`, `creep_rate`, `K_POWER` (0.08), the slump
angle. `relief/roughness.rs`: the amplitude, gradient and spectrum terms (D299, D301).
`region/mod.rs`: `RIVER_MARGIN` (6 cells), `MICRO_M` (9.6 m), `SOIL_M`, `STONY_M`, `ROCK_M`,
`NEAR_SQUARE` (64), `REGIONAL_SLACK_M`, the hydraulic geometry.

## What it gives (seed 7)
- At walking scale (`bench realism terrain`, one-metre windows beside USGS lidar of each kind;
  `docs/review/g1b/`): |Δh| at 1–64 m and the 4 m curvature within × 1.5 of the kind's lidar in
  hills, plains (between its two lidar windows), the desert and boreal land; mountains to their
  64 m relief (0.66). Closed hollows over 0.25 m 0.0–0.7 % of the ground off rivers outside the
  mountains (glacial land: 3.3–5.5 %), against G1a's 0.8–12 %.
- From 30 m to 30 km (`bench realism global|terrain`): the Earth's median slope, relief and
  steep land within their bounds; slope at 30 m by kind within ± 25 % but the desert's (5.5°
  against 4.2°).
- The far read stands in for the walking level: at every window its slope at 30 m within half a
  degree of the blocks' averaged to 30 m.
- Tiles: the walking level's some 50 ms with its parents' share; a new world makes 16 of them.

## Tests
`relief::tests`: Earth has levels down to metres and a test planet none; a cell has one value
whichever tile is made first; rivers fall all the way down through every level; each level keeps
its parent's surface on the whole; a river bends through its nodes; the walking level drains
(its cells and as the blocks read them); its channels step along rows and columns; reads between
cells stay between them; a far read stands in for the walking level. `region::tests` and
`tests/fine_tiles.rs`: a refined planet reads the same however it is sampled; coarse callers make
no walking tile.

## Known simplifications
- Small streams of a level's own drainage may shift a little at a tile's edge (the tiles agree on
  the surface and on every river kept from above).
- A lake kept from the parent stands at the parent's surface even where the finer ground would
  hold it lower; a river beside a lake and below its surface stays a river; a carved river's
  banks may stand below its water (W1).
- The desert's ranges are lower and its fans rougher than a basin-and-range's; mountains lack
  some of their kilometre-scale relief and are grainy at a metre.
- No glaciers shape the ground (cirques and U-shaped valleys), no sediment builds deltas, no frost,
  wind or solution shapes the walking scale yet.

## Sources
- Barnes R., Lehman C., Mulla D. 2014, "Priority-Flood: an optimal depression-filling and
  watershed-labeling algorithm for digital elevation models", *Computers & Geosciences* 62: 117–127.
- Lindsay J. B. 2016, "Efficient hybrid breaching-filling sink removal methods for flow path
  enforcement in digital elevation models", *Hydrological Processes* 30: 846–857.
- Braun J., Willett S. D. 2013, "A very efficient O(n), implicit and parallel method to solve the
  stream power equation governing fluvial incision and landscape evolution", *Geomorphology*
  180–181: 170–179.
- Quinn P., Beven K., Chevallier P., Planchon O. 1991, "The prediction of hillslope flow paths for
  distributed hydrological modelling using digital terrain models", *Hydrological Processes* 5:
  59–79.
- Perron J. T., Kirchner J. W., Dietrich W. E. 2008, "Spectral signatures of characteristic
  spatial scales and nonfractal structure in landscapes", *JGR* 113: F04003.
- Bull W. B. 1977, "The alluvial-fan environment", *Progress in Physical Geography* 1: 222–270.
- Yair A., Lavee H. 1985, "Runoff generation in arid and semi-arid zones", in *Hydrological
  Forecasting*, Wiley: 183–220.
- Fritsch F. N., Carlson R. E. 1980, "Monotone piecewise cubic interpolation", *SIAM Journal on
  Numerical Analysis* 17: 238–246.
- Leopold L. B., Maddock T. 1953, *The hydraulic geometry of stream channels and some
  physiographic implications*, USGS Professional Paper 252.
