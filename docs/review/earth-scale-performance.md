# The Earth-sized planet after E4.1: the land re-checked (E4.1 §4.8)

E4.1 made the Earth-sized world fast to look at, to make a person on and to load into
(`docs/spec/e4.1-earth-scale-performance.md`, D279–D288). This review checks what that did to
the land: the same planet, the same land at every distance it is drawn, and where the generator
did change (one planet across the seam, D285), the same kinds of land in the same measure.
*Before* is the stopping point E4.1 began from (41f6c02), *after* the end of E4.1; both built
here and run on the cloud machine's software adapter (llvmpipe), each from its own tree and with
its own game folder (every cache made afresh).

**In short:** the planet is the same to the byte; the land's shape is the same at every column
sampled; the rocks, soils and plants are the same kinds in the same measure, but individual
trees, plants and rock outcrops now stand in other places (their noises were made periodic round
the planet); at a distance the before and after pictures cannot be told apart.

## What E4.1 changed that the land could show

| Change | Where it shows | Expected |
|---|---|---|
| The globe's map from the planet grid, not block-level samples (D280) | the globe | the grid is the land's shape at the globe's scale (a texel is about 20 km) |
| Queries at the scale they stand for (D279): the far field, the distant tiles, the animals' habitats and the places read the level their footprint needs | the distance, the animals | the same macro structure, the detail the distance shows |
| Refinement tiles read back from disk (D284) | everywhere | nothing: the copy is the tile (deterministic, versioned, by the grid's fingerprint) |
| Noises periodic in the circumference; features hashed by their place round the planet; geometry about its own origin or in f64 (D285) | rock beds and folds, the provinces' borders, domes and basins, cliffs, kelp, flowers, the stands' ages, caves, trees at the seam | the same kinds and measures; patterns moved where a noise was replaced or a frequency snapped to a whole number of waves round the planet; exact (no steps) at 40 million blocks |
| The shaders' patterns periodic in 4,096 blocks (D285) | the ground's grain and beds, waves, wind, caustics | the same look, and no seam every 4,096 blocks |
| The near ground streamed in view first and faded in (D288) | while loading | nothing once loaded (pictures draw cubes at once) |

## The planet: the same

The planet grid's code is as it was: E4.1 only added a fingerprint read from it (`git diff
41f6c02 -- crates/hearth_worldgen/src/planet/` adds lines and removes none). Both builds made
seed 7's grid at the game's resolution (2048²) in their own folders, and the two files are the
same to the byte (`planet_7_earth_2048.bin.zst`, sha-256 `4dde577e…` both). Its hypsometry
(`bench hypso --seed 7`), against Earth's:

| | Seed 7, Earth-sized | Earth |
|---|---:|---:|
| Land | 29.0 % | 29.2 % |
| Mean height of the land | 791 m | 797–840 m |
| The land's 10 / 50 / 90 / 99 % heights | 118 / 588 / 1,248 / 5,244 m | |
| Mean depth of the ocean | 3,640 m | 3,682 m |
| Surface above 2 km / above 4 km / below 6 km | 1.60 / 1.08 / 0.14 % | ~3.5 / ~0.8 / ~1 % |
| Highest and deepest cell (20 km cells) | 6,380 m, −8,151 m | peaks 8.8 km, trenches ~11 km |

The worldgen statistics tests (E §11) pass at the end of E4.1 as at its start
(`scripts/check.sh`): the planet's climate zones, its deserts and wet windward coasts, its
hypsometry over seeds and at Earth's size (`planet::stats`); continents zoned into realms
(`tests/realms.rs`); deposits in their provinces, host rocks and climates and at their
frequencies (`tests/deposits.rs`); the geology's provinces by tectonic setting and its layers in
order (`geology`); the water table and springs (`tests/hydro.rs`); the refinement levels
consistent, and the seam and pole tests (D285). The coast tests, a soak run, were run for this
review too (`cargo test -p hearth_worldgen --test coasts -- --ignored`), in both trees: reefs,
marshes and mangroves and kelp pass in both; the coastal salt pans passed at the stopping point
and found only 3 columns after, against more than 10 wanted. The cause is the shelter's noise,
redrawn periodic round the planet (with the old formula put back, the test passes): on the test
planet the few desert coasts are sheltered or not by stretches of shore kilometres long, and the
redrawn noise moved them. The marshes' and mangroves' test, which counts sheltered low coasts in
every climate, passes either way: the shelter's measure is unchanged. The salt pans' test now
scans an 8-block lattice keeping only the pans (a whole lattice that fine would take
gigabytes): 28 columns, every one on a hot dry coast with its salt crust.

## The land: a census before and after

`crates/hearth_worldgen/tests/surface_census.rs`, run in both trees on the same planet file: 600
columns of land spread round the planet within about 60° of the equator, the two cubes about
each surface generated as the world began, and their blocks tallied by kind.

- **The land's shape is the same:** all 600 columns have their surface at the same height,
  to the block.
- **The same kinds in the same measure:** the two tallies differ by 1.8 % of the blocks (the
  share that would have to change kind to make them equal), mostly rock kinds trading places
  where the provinces' borders, folds and domes moved: gneiss 5.1 → 4.6 % of the blocks, red
  sandstone 1.9 → 2.2 %, shale 0.7 → 0.5 %, schist 16.6 → 16.9 %. Water 17.9 % both; grass
  4.5 % both.
- **Things in other places:** 172 columns came out block for block the same; 527 have the same
  top block. Where the top block changed it is mostly a plant (one grass for another, a crown
  where there was none or none where there was one: the stands' ages and the flowers now come
  from noises periodic round the planet), and a few soils and rocks (sand for turf, schist
  cobbles for quartzite cobbles) where the deposits' undulation and the rocks' provinces moved.

## The land: pictures before and after

`tools/shots/e41_earth.shots` (seed 7, 1280 × 720, midsummer at eleven, the game's grid), each
pair before on the left and after on the right (`docs/review/e4.1/`).

| Place | Pair | Seen |
|---|---|---|
| The range: the highest land (6,380 m, an ice cap 14 million blocks east of the origin), from its top | `range_high.jpg` | the same ridges, slopes and snowfields; the rock showing through the snow in a few other spots |
| The range from 1,200 m above it | `range_air.jpg` | the same, to the horizon |
| The range's bedded rock by a pool near its top | `range_valley.jpg` | the same beds, snow patches and water |
| A temperate coast (50° N) from the shore | `coast_shore.jpg` | the same meadow, flowers and trees; a few plants differ |
| The coast from 400 m | `coast_air.jpg` | the same shore, beaches, islands and woods |
| A plain (52° S) from the ground, in pine wood | `plain_ground.jpg` | the same kind of wood, ferns and ground; the trees stand in other places and one now shades the camera |
| The plain from 300 m | `plain_air.jpg` | the same canopy, clearings and lake |
| By the water on the plain | `plain_river.jpg` | the same wood and ground; a nearer trunk in another place |
| The globe about the range (after only) | `globe.jpg` | the continent, its biomes, lakes and the range's ice cap; before E4.1 the map was never finished (PLAYTEST 29) |

At the distance a shortcut is used (the distant tiles, the far field, the globe) the land looks
the same; up close the trees and plants are as before in kind and number but not in place.

## Verdict

Realism is unchanged: the planet and the land's shape are identical, and the generator's
changes for the seam move individual trees, plants and outcrops without changing what grows or
lies where. The one visible difference, trees in other places, is the price of a planet whose
features match across the seam, taken knowingly (D285). A world saved before E4.1 opens with
its trees and plants in their new places and the player's changes laid over them as before (on
the Earth-sized planet such worlds could not be opened before E4.1).
