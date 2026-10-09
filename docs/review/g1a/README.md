# G1a review pictures: relief from 30 m to 30 km (D299)

Seed 7, the Earth. Measured by `bench realism global` and `bench realism terrain --wide`
(`../realism/measures/`); rendered on the cloud machine's software device.

- `kinds_30m.jpg`: 30 km of each kind at 30 m, shaded. Each row shows real lidar (USGS 3DEP)
  on the left, T2's generated land in the middle and G1a's on the right. Rows: hill country,
  a plain, a desert, boreal forest, mountains. The sites are matched by biome, height and rain,
  so a row's middle and right columns may be different places.
  - Hill country has the Earth's slopes now (20.0° against 21.8°; T2 1.8°). Its relief is bumps,
    not ridges and valleys running together as the lidar's are. That form is G1b's channel heads.
  - The desert has flat basin floors between ranges (4.9° against 4.2°). Its ranges are lower
    and fewer than the basin-and-range land beside it (p90 15° against 22°).
  - The flat, straight-edged patches in the plain, the desert and the mountains are water on the
    planet grid's lakes. Their shores follow the grid's cells, and the grid has three times the
    Earth's share of lakes (W1).
- `global_before_after.jpg`: the planet's sample windows, shaded; T2 on the left, G1a on the
  right. Hill and mountain windows have valleys now, where most were bare before.
- `river_map.jpg`: a river of 42 m³/s on a plain, in plan. Before (left), its course ran in
  straight stretches with right-angled corners at its parent's cells. After (right), it curves
  through them at every level and wanders by slope and discharge.
- `land_from_400m.jpg`: the game from 400 m at 16 h; before on the left, after on the right.
  Rows: hill country, a plain, a desert, boreal forest, mountains.
  - The hill country (fourth row) now shows hills, valleys and far ridges where before it was flat.
  - The plain and the desert stay gentle, as they should.
  - The mountain shot's site, found by biome, now lies on a lake. The grid's lakes are W1's.

Measured (T2 → G1a; the Earth's in brackets, each within its bound):

| Measure | T2 | G1a | Earth's |
|---|---|---|---|
| Windows' median slope | 1.5° | 2.3° | 2.8° (± 20 %) |
| Windows' median relief | 78 m | 183 m | 163 m (± 20 %) |
| Land over 5° | 16 % | 30 % | 33 % (± 5) |
| Relief at 0–200 · 200–500 · 500–1,000 m · 1–2 · over 2 km | 33 · 50 · 89 · 299 · 1,515 m | 80 · 73 · 188 · 482 · 1,167 m | 73 · 80 · 231 · 458 · 1,170 m (± 25 %) |
| Slope at 30 m: hills · plains · desert · boreal · mountains | 1.8 · 2.3 · 2.4 · 1.1 · 29.5° | 20.0 · 2.9 · 4.9 · 2.4 · 28.5° | 21.8 · 3.3 · 4.2 · 2.1 · 24.6° (± 25 %) |
