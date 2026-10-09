# RGA-1 — the first realism gap analysis

*T2 of Amendment T (§3), 2026-10-09. The generated Earth of seed 7, the game's planet (`main`
after T1), measured and looked at against the real one at every scale. Evidence first: the
numbers come from `bench realism` (its commands below) and the pictures from the realism suite
(`tools/shots/realism.shots`) and the terrain comparisons in `terrain/`. Severity is how much a
player would notice: 5 at once, by anyone; 1 only an expert, looking for it.*

## 0. In short

The planet is right, and so are the sky and its light levels; what lies between 30 m and 30 km,
what grows on it, and the shadows the sun should cast across it are not yet the Earth's.

- **The planet** (severity 1): land 29.0 % (Earth 29.2), mean land 791 m (797–840), the ocean
  3,640 m deep (3,682). High plateaus and the deepest trenches are short.
- **The land between 30 m and 30 km** (severity 5): half the Earth's relief. Over 200 random
  windows of land, the generated median slope is 1.5° (Earth 2.8°), the median relief 78 m (163
  m), the land steeper than 5° 16 % (33 %). In humid hill country the gap is widest: at 30 m
  the Oregon Coast Range's median slope is 21.8°, the generator's forested hills 1.8°, and their
  relief over 30 km 762 m against 122 m. High mountains have the Earth's slopes but none of its
  form: no valleys, troughs, ridgelines.
- **Walking scale** (severity 5): below the finest level (38 m) the ground is the level's smooth
  surface and isotropic noise — two to six times smoother than real ground at every lag from 1 to
  64 m (six in hill country), with hollows that hold water where real ground drains, and no
  gullies, channel heads, fans, scree, terraces or pit-and-mound.
- **Drainage drawn on the grid** (severity 4): valleys and rivers run in the eight directions of
  the cells they were cut on (combs of parallel gullies, eight-spoked volcanoes, rivers as
  zigzags of straight chords).
- **No cast shadows** (severity 5): nothing shadows anything; a low sun lights the world as noon
  does.
- **Trees, grass and flowers as voxels and pixel sprites** (severity 5), placed without regard
  to one another or to the ground's small forms.
- **Water as blocks** (severity 5), in 1 m steps, without flow.
- **A seam where full detail ends** (severity 5): the step across it a median four times the
  steps beside it (forests 3.0–8.4), the band beyond 46 % darker to 89 % brighter.
- **Weather** (severity 3): it rained in about a third of all hours, lightly — wet days were
  taken for wet hours. **Fixed in passing** (D296): London now rains in 6.0 % of its hours
  (records 6–8 %).

The plan these lead to is §5; the ranked table §4.

## 1. How it was measured

| Command | What | Output |
|---|---|---|
| `bench realism terrain` | Five kinds of land, each beside a real one of the kind: a kilometre at 1 m (and 4 m) from USGS 3DEP lidar, thirty kilometres at 30 m from SRTM; slopes, curvature, roughness by lag, closed hollows, drainage density, the slope–area relation, the largest channel's sinuosity, hypsometry; shaded relief side by side | `measures/terrain_earth.md`; `terrain/*.jpg` |
| `bench realism global` | 200 windows of some ten kilometres at random over the land between 60° S and 60° N, on the Earth (AWS terrain tiles) and on the generated planet (by area as on a sphere), at the same pixel size | `measures/global_earth.md`; `terrain/global_generated.jpg` |
| `bench realism levels` | Each refinement level's surface alone, and the whole, at each kind's place | `measures/levels_earth.md` |
| `bench realism terrain --planet standard` | The fine-detail regression: the same seed on the Standard test planet | `measures/terrain_standard.md` |
| `bench realism weather` | The weather model's year, hour by hour, at five real places' climate normals, beside their records | `measures/weather.md` |
| `bench realism images DIR` | Pictures' luminance, colourfulness, spectral slope and repetition | `measures/images.md` |
| `hearth --screenshot-list tools/shots/realism.shots` | The realism suite: 16 biomes and a stream × 4 scales (underfoot, eye height, a 40 m rise, a 400 m hill) × 2 lights; each shot's near–LOD seam measured | `suite/*.jpg`; `measures/seams.tsv` |

The real ground in the paired pictures (left in each) is the U.S. Geological Survey's 3D
Elevation Program lidar and SRTM (NASA, USGS), both public domain, read from the USGS and the AWS
Terrain Tiles. The references and their licences are in `references.md`;
`scripts/fetch-realism-refs.sh` fetches them. **Limits:** the photographs could not be fetched here (Commons is closed to this
machine), so the suite's matched photographs are the owner's to fetch (`--photos`), and the
suite was judged here against the measured statistics, published descriptions and the lidar;
the renders are the software adapter's (the same shaders as the PC, without its frame rates);
one seed (7).

## 2. Findings by scale

Each: what was seen, how Earth builds it against how the generator does, severity.

### 2.1 Planet and continents — severity 1
- `bench hypso`: land 29.0 % (29.2), mean land height 791 m (797–840), land at 10/50/90/99 %
  118/588/1,248/5,244 m, mean ocean depth 3,640 m (3,682). Above 2 km 1.6 % of the surface
  (Earth ~3.5 %): the high plateaus (Tibet, the Altiplano) are short. Below 6 km 0.14 % (~1 %),
  the deepest point 8.2 km (11): the hadal trenches are shallow. The globe (T1.4) reads well.
- *Earth*: plates over billions of years, isostasy, erosion to base level, the climate's belts.
  *Generator*: a plate simulation on the 20 km grid, uplift fields, a Köppen climate, stream-power
  erosion and drainage on the grid. Close in process and in result.

### 2.2 Regions, 10–1,000 km — severity 3
- Rivers are random midpoint-displacement polylines at each refinement level, snapped to the
  level's eight-neighbour cells; no meanders, oxbows, braids or deltas as landforms
  (`relief.rs`, `region/rivers.rs`). At the blocks a river is a run of straight chords between
  jittered cell nodes, ±13 m (`terrain/hills_1m.jpg`: the zigzag).
- Drainage cut on the grids shows their directions: combs of short parallel valleys in the
  random windows (`terrain/global_generated.jpg`), a volcano's gullies as straight spokes in the
  eight directions (`terrain/volcano_30m.jpg`), coast and lake edges in steps.
- The largest channel's sinuosity in the 30 km windows: real 1.40–1.77; generated 1.13–3.95 (the
  route across filled flats is not a fair measure there; the pictures are the evidence).
- *Earth*: rivers incise to base level and migrate; meanders by bank erosion and point-bar
  growth; braids where gradient and sediment are high; deltas where sediment meets the sea.
  *Generator*: stream power on D8 grids, each level's rivers redrawn as noise polylines.

### 2.3 Landscapes, 1–10 km (the view from a hill) — severity 5

Thirty kilometres at 30 m (`bench realism terrain`; pictures `terrain/*_30m.jpg`, real left):

| Land | Slope p50 / p90 (°), real | generated | Steeper than 30°, real · gen | Relief (m), real · gen | Closed hollows >1 m, real · gen |
|---|---|---|---|---|---|
| Forested hills (Oregon Coast Range) | 21.8 / 35.7 | 1.8 / 4.9 | 24 % · 0 % | 762 · 122 | 0.3 % · 1.8 % |
| Rolling plains (south-central Iowa) | 3.3 / 7.1 | 2.3 / 5.2 | 0 · 0 | 93 · 228 | 0.7 % · 1.1 % |
| Basin and range (Mojave, Nevada) | 4.2 / 21.8 | 2.4 / 4.4 | 4.1 % · 0 % | 1,036 · 250 | 0.2 % · 5.3 % |
| Boreal shield (north-east Minnesota) | 2.1 / 6.5 | 1.1 / 2.6 | 0 · 0 | 191 · 101 | 2.4 % · 1.1 % |
| Glaciated mountains (Glacier National Park) | 24.6 / 44.6 | 29.5 / 44.3 | 38 % · 49 % | 2,118 · 3,666 | 0.5 % · 6.7 % |

Over the whole land (`bench realism global`, 200 windows each):

| | Window median slope p10 · p50 · p90 (°) | Relief p10 · p50 · p90 (m) | Land steeper than 5° · 10° · 20° · 30° |
|---|---|---|---|
| Earth | 1.2 · 2.8 · 15.5 | 31 · 163 · 949 | 33 % · 19 % · 8 % · 2.5 % |
| Generated | 0.4 · 1.5 · 7.3 | 21 · 78 · 415 | 16 % · 9 % · 6 % · 3.2 % |

The same windows by their mean height (median relief, median slope, how many):

| | 0–200 m | 200–500 m | 500–1,000 m | 1–2 km | above 2 km |
|---|---|---|---|---|---|
| Earth | 73 m, 2.3° (44) | 80 m, 2.3° (65) | 231 m, 3.8° (41) | 458 m, 5.0° (36) | 1,170 m, 14.9° (14) |
| Generated | 33 m, 0.7° (30) | 50 m, 0.9° (42) | 89 m, 1.7° (95) | 299 m, 4.3° (17) | 1,515 m, 26.4° (16) |

Below a kilometre the generated land has under half the Earth's relief and a third of its
slope; between one and two kilometres it comes near; above two it is steeper than the Earth's.
Real relief grows steadily with height above base level; the generator's comes only with uplift.

- Hill country is nearly flat: the Coast Range's ridge-and-valley texture, valleys a few hundred
  metres apart with slopes at 30–40°, is absent. Deserts have no ranges, fans or washes; the
  boreal shield none of its knobs, streamlined drift or chains of lakes. High mountains have the
  slopes but not the form: an evenly crumpled surface, no troughs, cirques, arêtes or valley
  floors (`terrain/mountains_30m.jpg`).
- The relief is missing already at the coarse levels: over 30 km at the hills' place the grid
  gives 62 m, the 2.4 km level 114 m, the 306 m and 38 m levels 120 m
  (`measures/levels_earth.md`).
- *Earth*: the relief of land is set by how far rivers have cut below it and by how hillslopes
  answer (diffusion, landslides), at a valley spacing of a few hundred metres in soil-mantled
  hills (Perron et al. 2009); glaciers carve troughs; deserts are tectonic ranges shedding fans.
  *Generator*: each level adds relief as `amplitude(λ) = 160 (λ / 1 km)^0.7` m times a
  roughness of 0.012 on lowlands, 0.11 on high land and 1 in mountains (`relief.rs`), set by
  uplift alone; stream power cuts weak D8 valleys; no hillslope process, no glacial or aeolian
  forms.

### 2.4 Hillslopes and valleys, 10 m–1 km — severity 5

A kilometre at 1 m (pictures `terrain/*_1m.jpg`, lidar left; numbers at 1 m unless said):

| Land | Slope p50 / p90 (°), real | generated | Mean \|Δh\| at 1 · 4 · 16 · 64 m, real | generated | Curvature at 4 m p5 / p95 (1/m), real · gen |
|---|---|---|---|---|---|
| Forested hills | 29.8 / 41.1 | 4.6 / 10.5 | 0.35 · 1.38 · 5.2 · 16.4 | 0.06 · 0.24 · 0.90 · 2.78 | −0.090/+0.119 · −0.014/+0.015 |
| Rolling plains | 1.2–10.8 / 12.9–19.4 | 2.7–3.3 / 5.3–6.3 | 0.05–0.13 · 0.19–0.51 · 0.6–1.8 · 1.8–5.5 | 0.04 · 0.14–0.16 · 0.5–0.6 · 1.8–2.2 | −0.03…−0.04/+0.03…+0.06 · −0.011/+0.012 |
| Basin and range | 5.5–5.9 / 26.7–27.4 | 3.1–3.2 / 5.6–6.2 | 0.12–0.13 · 0.44–0.49 · 1.5–1.7 · 4.5–4.7 | 0.04 · 0.14–0.15 · 0.53–0.57 · 1.8–2.0 | −0.07…−0.09/+0.08…+0.09 · −0.013/+0.013 |
| Boreal shield | 3.2–3.8 / 12.0–13.4 | 1.5–1.6 / 2.8–3.6 | 0.07 · 0.21–0.22 · 0.7 · 2.1–2.2 | 0.02 · 0.07–0.08 · 0.25–0.30 · 0.8–0.9 | −0.03…−0.035/+0.03 · −0.008/+0.009 |

- The real ground is two to six times rougher at every lag (six in the hills), and its
  curvature (hilltops convex, hollows and channels concave) three to eight times stronger. Real
  hillslopes carry gullies, channel heads, terraces and fans; the generated ones carry none
  (`terrain/plains_1m.jpg`: Iowa's dendritic gullies beside smooth noise).
- Below the finest level the blocks add almost nothing: at the hills' place, 2 m apart, the 38 m
  level alone has a median slope of 1.8° and the whole 2.1°, the same |Δh| at every lag from 2 m
  up; the noise below (`detail`, 96 m, ±0.4 m on lowland; `rough`, 24 m, ±0.35–1.55 m) only
  roughens the curvature.
- Where the levels' drainage shows at walking scale it runs in straight segments with 45° bends
  (`terrain/volcano_1m.jpg`).
- The slope–area relation's concavity is within the Earth's range where channels exist (θ 0.51
  generated, 0.42 Coast Range at 30 m; Hack, Flint: 0.4–0.7).
- *Earth*: creep and rain splash round the crests; overland flow concentrates into rills and
  gullies where it exceeds a threshold (channel heads at 10³–10⁵ m², Montgomery and Dietrich);
  talus rests at 33–38°; colluvium gathers at the foot. *Generator*: a smooth surface and
  isotropic noise.

### 2.5 The site, 1–10 m — severity 4
- Closed hollows deeper than 0.25 m cover 1–6 % of the generated ground at 1 m (the noise's
  hollows) against 0–0.4 % of the real (0.5–1.4 % in the glaciated mountains): puddles would
  stand where real ground drains (and W1's water would find them).
- Stones: cobbles by chance per column; boulders one ellipsoid of 1.2–2.4 m in 8 % of 12-block
  cells; logs a straight 3–5 block log along x or z in 12 % of forest cells
  (`cubegen/features.rs`). No size distribution, burial, sorting, scree or pit-and-mound; no
  trails or wallows.
- The suite's eye-height shots: §2.13.

### 2.6 The ground, cm–1 m — severity 4
- The suite's underfoot shots: §2.13. No litter, twigs, cones, pebbles, moss or roots; the sward
  a smooth noise of one green with bare blotches.

### 2.7 Water — severity 5
- Natural water is static blocks (`water`, levels 0–8), its surface rounded to whole blocks, so a
  river falls in 1 m steps and meets the smooth ground at cube edges; drawn by the old block
  mesher's `fluid()`. Waves follow the wind, not the current; no flow maps, riffles, pools, bars,
  waterfalls or deltas; lakes never have beaches (`region/biome.rs`).
- Rivers cross the land in straight chords with banks of one height (1.2 blocks), not in the
  valleys they would have cut (`terrain/hills_1m.jpg`); the S5 stopping point's finite-water
  test needed twelve times as many banks to find one that holds its channel (T §0.2).
- *Earth*: the channel is shaped by the flow it carries (Leopold and Maddock; already the
  generator's width and depth), its surface falls smoothly, its bed sorts its stones.
  *Generator*: hydraulic geometry right in width and depth, everything else blocks.

### 2.8 Life in the land — severity 4
- Trees: one hashed point per 5 × 5 blocks, kept by density and a crown-area chance, each site
  alone: no spacing (real forests are regular at the crown's scale and clumped at tens of
  metres), no response to the ground's small forms. Drawn as voxel templates; the S5 meshes
  wait for S5 (c).
- Grass: crossed sprites on a third of the columns; flowers in 24-block squares, understory in
  axis-aligned squares.
- Animals: a group's place redrawn at random in its range each step; no trails, routes or
  wallows; the only flocks are scavengers.

### 2.9 Weather and sky — severity 3
- `bench realism weather` (before → after the fix in passing, D296):

| Place | Wet hours | Spells, median · p90 | Rate when wet | The year (normal) | Records |
|---|---|---|---|---|---|
| London | 29.4 % → 6.0 % | 5 · 14 → 3 · 7 h | 0.3 → 1.2 mm/h | 766 → 628 mm (600) | 6–8 % of hours |
| Portland, Oregon | 38.2 % → 7.7 % | 5 · 18 → 3 · 8 h | 0.4 → 1.6 mm/h | 1,460 → 1,056 mm (1,100) | ~10 % |
| Des Moines, Iowa | 28.0 % → 5.9 % | 3 · 13 → 2 · 7 h | 0.5 → 1.7 mm/h | 1,211 → 894 mm (900) | 5–7 % |
| Las Vegas | 3.0 % → 0.9 % | 1 · 5 → 2 · 4 h | 0.3 → 1.0 mm/h | 90 → 80 mm (110) | under 1 % |
| Manaus | 33.0 % → 7.7 % | 2 · 12 → 3 · 7 h | 1.3 → 3.6 mm/h | 3,791 → 2,425 mm (2,300) | showers most wet-season afternoons |

- Clouds: one textured layer at one base, the same cover over the whole sky; no cumulus fields,
  decks, cirrus or towers, no cloud shadows; no fog of any kind; wind blind to the terrain; snow
  without drifts. The sky itself (Hillaire 2020, calibrated to 100 klx) and the moon and night
  levels are right; the stars are random, without the Milky Way.
- *Earth*: fronts and rain bands within systems, showers by convection, fog by radiation and
  advection, wind steered by the land. *Generator*: one drifting noise field and its threshold.

### 2.10 Light and rendering — severity 5
- **No cast shadows**: no shadow maps; direct light reaches every face open to the sky
  (`rendering.md`'s known simplifications). Trees do not shade the ground, hills do not shade
  their lee, a low sun lights the world as noon does (the suite's low-sun shots).
- No indirect light: ambient is the sky's irradiance times the voxel sky light and a face's
  direction. Occlusion baked on the smooth ground only (five rays). Leaves without translucency
  in the world.
- Right already: the sun and sky's levels (Q §2.1), materials against measured ranges, exposure,
  aerial perspective.

### 2.11 Distance — severity 5
- The seam: §2.13's measure (a median four times the steps beside it). The LOD shades each
  material's mean albedo with a simpler light path; forests far off are crown boxes, then a
  canopy surface; small plants end at the full-detail square; distant water is baked colour
  without refraction. Haze is physical and the same near and far.

### 2.12 Motion, animals and sound — severity 3
- Gaits and speeds audited (P5). Herds have no cohesion model; no flocks crossing the sky.
- Sound: wind, rain and animals' calls; the only ambience is crickets, the same in every biome;
  no water, surf, thunder or dawn chorus.

### 2.13 The realism suite

132 shots, 16 biomes and a stream (`suite/<biome>.jpg`: rows underfoot, at eye height, from the
40 m rise and from the 400 m hill; late morning left, the sun 15–20° up at 7 h right). What gives
them away, in the order a player would see it:

1. **No shadows.** At 7 h every tree, tussock and hill should cast a shadow longer than itself;
   nothing does. A low sun shows only in its tint and in the sunward faces' brightness (the
   boreal, broadleaf and tropical rises: crowns bright on one side, their lee lit as at noon).
2. **Blocks and pixels.** Trees are voxel templates: square trunks (the tropical forest's fills
   the frame as a stepped brown pillar), crowns of leaf cubes, branch bars at right angles (the
   boreal birch's white L); every small plant is a pair of crossed 16-pixel sprites a metre
   high, so a meadow at eye height is a wall of large pixels; small stones are cubes (white in the
   tundra, dark by the hundred on alpine rock).
3. **A lawn in every biome.** One smooth sward colour with soft blotches, saturated green under
   the temperate and tropical rainforests, the boreal forest, the savanna and the tundra alike;
   no litter, needles, moss, lichen, roots, twigs, cones, tussocks or stones (a real boreal
   floor is feathermoss and blueberry, a rainforest's litter among buttresses, the tundra muted
   olive and rust).
4. **Flat, and patched.** From the rise and the hill most land is a plain (§2.3); its materials
   are hard-edged blobs repeated to the horizon (the steppe's grass and grey gravel in a
   camouflage pattern, the tundra's and the deserts' coloured spots), a noise's threshold.
   Alpine rock is a smooth rounded surface striped by its strata like a contour map.
5. **The edges of detail.** From the rise the full-detail square is a sharp-edged disc of
   sprites in plainer ground (desert, dune sea, savanna, steppe); from the hill the near forest
   is a dark carpet ending at a line, the LOD forest lighter beyond. Measured (`seams.tsv`, the
   61 shots that see the edge): the step across it is a median 4.6 times the steps beside it in
   forests (3.0–8.4) and 3.7 on open land (0.2–22: below 1 only on bare rock and tundra, whose
   near ground has no sprites); the band beyond 46 % darker to 89 % brighter.
6. **Missing landforms and water.** The dune sea has no dunes (a flat dark plain with sprites);
   the hot desert no wadis, pavement or inselbergs; seed 7's Earth has no Mesa land at all (the
   suite's mesa shots found none and became alpine rock); the stream's place is a still pool in a
   ring of bare soil; the beach has no surf.

Measured (`bench realism images`, `measures/images.md`): the spectral exponent α is 1.9
underfoot (1.5–2.5) but 1.2 at eye height (0.9–1.7) and 1.0 from the rise and the hill
(0.4–1.6) — natural scenes 1.8–2.4: too much fine contrast (sprites, cube edges) and too little
broad light and shade (the missing shadows); colourfulness a median 60 at eye height (31–85) and
51 overall — natural photographs mostly 15–60, so at or past the most colourful. The repetition
score is low (median 0.02; at most 0.15, the cubes on alpine rock).

**Right already:** the sky's colour and brightness, the haze over distance, exposure at both
lights, the low sun's tint, the materials' albedos within their measured ranges.

## 3. The fine-detail regression

The same seed on the Standard test planet (65,536 blocks around, its grid 32 blocks a cell, a
block standing for 4 m of relief) and on the Earth, at 1 m, beside the lidar
(`measures/terrain_standard.md`; `terrain/hills_1m_standard.jpg`):

| Land | Slope p50 (°): Standard · Earth · real | \|Δh\| at 1 m: Standard · Earth · real | Steeper than 30°: Standard · Earth · real |
|---|---|---|---|
| Forested hills | 14.6–18.7 · 3.5–4.6 · 29.7–29.8 | 0.21–0.25 · 0.06 · 0.34–0.35 | 13–20 % · 0.1–0.4 % · 49 % |
| Rolling plains | 15.4–23.8 · 2.7–3.3 · 1.2–10.8 | 0.21–0.32 · 0.04 · 0.05–0.13 | 12–34 % · 0 · 1 % |
| Basin and range | 23.3–50.9 · 3.1–3.2 · 5.5–5.9 | 0.35–0.78 · 0.04 · 0.12–0.13 | 36–79 % · 0 · 7 % |
| Boreal shield | 6.2–15.3 · 1.5–1.6 · 3.2–3.8 | 0.11–0.20 · 0.02 · 0.07 | 5–10 % · 0 · 0.3–0.4 % |

- **What was lost:** on the Standard planet the grid's own erosion and drainage cut valleys a few
  tens of blocks apart, so walking-scale ground had organised relief. On the Earth the same
  processes act at the levels' 2.4 km, 306 m and 38 m cells with the Earth's relief per
  wavelength and a lowland roughness of 0.012; below 38 m only noise of some 0.4 m. The ground
  became two to six times smoother than real ground.
- **What the Standard planet had wrong:** relief exaggerated four times over a 611-fold
  compression made every kind of land steep — plains at 15–24°, deserts at 23–51° — and the
  grid's cells showed as straight seams and facets. It is not the target; the lidar is.
- **What each level gives** (`levels_earth.md`, the hills' place): over 30 km, relief 62 m (grid),
  114 m (2.4 km level), 120 m (306 m), 121 m (38 m), 122 m (the blocks); slope p50 at 30 m 0.1°,
  0.4°, 0.9°, 1.8°, 1.8°. Every level adds a little and none enough; the noise below adds
  nothing measurable above 8 m.

## 4. The ranked gaps

Perceived realism gained (severity × how much of the world it touches, 1–5) over cost (weeks of
work, and the frame, the CPU and memory it takes), with an approach and a measurable target; the
rows in the order of that ratio.

| # | Gap | Gain | Cost | Approach | Target |
|---|---|---|---|---|---|
| 1 | No cast shadows | 5 | low: ~1 week; ≤ 1.5 ms GPU | Cascaded shadow maps for the near world and things; the distant terrain's horizon shadows from its height fields; contact shadows | The low-sun suite shots shadowed; ≤ 1.5 ms at 1080p on the RTX 4060 |
| 2 | Relief between 30 m and 30 km | 5 | medium: ~2 weeks; tile time | Relief set by landscape type and height above base level, not uplift alone: the levels' amplitudes and erosion calibrated to the global and matched statistics; hillslope diffusion at the 306 m and 38 m levels | Global: median slope 2.8° ± 20 %, relief p50 163 m ± 20 %, land > 5° 33 % ± 5; relief by height band within 25 % of the Earth's (73 · 80 · 231 · 458 · 1,170 m); matched kinds' 30 m slope p50 ± 25 % |
| 3 | Walking-scale landforms (below 38 m) | 5 | high: ~4 weeks; tile time | G1: a ~5 m and a ~1 m level with diffusion, channel heads and gullies, colluvium and fans, scree, tree-throw; detail from lidar statistics by landscape type (hybrid, T §4.1 D) | \|Δh\| at 1–64 m and 4 m curvature within × 1.5 of the matched lidar; closed hollows > 0.25 m under 1 % outside glacial and karst land |
| 4 | Drainage on the grid's directions | 4 | medium (with 2, 3, 7) | Multiple-flow routing for erosion; rivers as smooth curves with meanders by slope and discharge | Channel directions without eight peaks; lowland sinuosity 1.3–2.5 |
| 5 | Trees as voxels, placed alone | 5 | medium: S5 (c), (d) | The S5 meshes in the world; placement by competition (inhibition within a crown, clumps at tens of metres) and by the ground's forms | The suite's eye-height shots; pair correlation as measured forests |
| 6 | The near–LOD seam | 5 | medium: L1 | The LOD shaded with the near ground's materials and light path; trees continuous across; the seam metric in the perf gate | Seam ratio ≤ 1.3, bands within 5 % in brightness and 3 in colour |
| 7 | Water as blocks | 5 | high: W1 | Water surfaces from the hydrology, channels carved to hold them, a shallow-water simulation near players, flow maps | No cube edges; every river in a channel that holds it |
| 8 | Grass, flowers and the ground's surface | 4 | medium: P7G, G1 | Grass handed to the sward and instanced blades; litter, pebbles, moss by biome | The suite's underfoot shots |
| 9 | Stones, logs, litter | 4 | medium: P7 (e) with G1 | Clast sizes heavy-tailed, shapes by rock, burial, sorting along rivers, scree grading | Size distributions as measured |
| 10 | Clouds and fog | 3 | medium: R1 | Cloud types by situation, cover varying over the sky, cloud shadows, valley and coastal fog | The suite's skies; cover's U-shape |
| 11 | Indirect light | 3 | high: R1 | Irradiance probes fed by the voxel light grid with a bounce's colour | Interiors and shade no longer flat |
| 12 | Animals' paths and flocks | 2 | low–medium: S6 | Daily routes, trails worn into the ground, flocks crossing the sky | — |
| 13 | Soundscapes | 3 | low | Water, surf, dawn chorus and insects by biome | — |
| 14 | Plateaus and trenches | 1 | low | Uplift fields for plateaus; trench depth | Above 2 km ~3.5 % |
| — | Weather's wet hours | 3 | done | Fixed in passing (D296) | London 6.0 % of hours |

## 5. The plan this leads to

See `PLAN.md` and D297: **R1a (shadows)** first, being cheap and touching every view and every
later milestone's review; then **G1** in two parts — **G1a**, the relief between 30 m and 30 km
(gaps 2 and 4), and **G1b with S5/P7 (c)–(f)**, walking scale with the trees, shrubs, stones and
logs (gaps 3, 5, 9) — then W1, L1, P7G, S6 with H1, R1b (clouds, fog, indirect light) and the
rest as before.
