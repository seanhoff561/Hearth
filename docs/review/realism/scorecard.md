# The realism scorecard (T §3.6)

How the generated Earth stands against the real one, scale by scale: each measure now against
its target, and the last realism suite's verdict. Updated at every audit (the measures by
`bench realism`, the suite by `tools/shots/realism.shots`; RGA-1 for the evidence). Seed 7.

**Last update:** G1b, 2026-10-14 (the Landscapes, Hillslopes and Site rows; the Regions row and
a Lakes row G1a's, 2026-10-09; Light and Images R1a's; the Trees row S5 (c)'s; the rest T2's,
RGA-1). **Top open gaps:** grass and water drawn as sprites and blocks, rivers whose banks stand
below their water, the trees past the render distance as boxes; the grid's lakes three times the
Earth's share; indirect light; the desert's form (fans rough, ranges low).

| Scale / system | Measure | Now | Target | Suite verdict |
|---|---|---|---|---|
| Planet | Land share; mean land height; mean ocean depth | 29.0 %; 791 m; 3,640 m | 29.2 %; 797–840 m; 3,682 m | The globe reads as the Earth's |
| Planet | Surface above 2 km; below 6 km | 1.6 %; 0.14 % | ~3.5 %; ~1 % | — |
| Regions | Channels on the grid's eight directions | none: water spread by slope, courses curved through their nodes (G1a) | none | (G1a review: `docs/review/g1a/`) |
| Landscapes, all land | Windows' median slope; median relief; land over 5° | 2.5°; 170 m; 31 % (G1b; G1a 2.3°; 183 m; 30 %; T2 1.5°; 78 m; 16 %) | 2.8°; 163 m; 33 % (± 20 %) | Hills and valleys from 400 m (G1a) |
| Landscapes, by height | Median relief of windows at 0–200 · 200–500 · 500–1,000 m · 1–2 · over 2 km | 81 · 86 · 183 · 505 · 1,161 m (G1b; G1a 80 · 73 · 188 · 482 · 1,167) | 73 · 80 · 231 · 458 · 1,170 m (± 25 %) | — |
| Landscapes, by kind | Slope p50 at 30 m: hills · plains · desert · boreal · mountains | 17.3 · 3.4 · 5.5 · 2.5 · 29.1° (G1b; G1a's 20.0 · 2.9 · 4.9 · 2.4 · 28.5 counted the blocks' aliased noise); the hills' 1 km windows 21.2–21.6° (lidar 21.6–22.2) | 21.8 · 3.3 · 4.2 · 2.1 · 24.6° (± 25 %) | Hills of ridges and valleys from a metre up (G1b); the desert's fans rough and its ranges low |
| Hillslopes | Mean \|Δh\| at 4 m, hills · plains · desert · boreal | 1.49–1.56 · 0.21–0.26 · 0.35–0.57 · 0.18–0.21 m (G1b; G1a 1.03–1.27 · 0.19–0.20 · 0.27–0.30 · 0.15–0.16) | 1.34–1.38 · 0.19–0.51 · 0.44–0.49 · 0.21–0.22 m (× 1.5) | Gullies and washes down the slopes (G1b); no fans, scree or terraces as forms of their own |
| Hillslopes | Curvature at 4 m, p95, hills | 0.119–0.130 /m (G1b; G1a 0.105–0.121) | 0.112–0.119 /m (× 1.5) | — |
| Lakes | The grid's lakes' share of the land | 6.3 % | ~2 % | Flat windows; shores stepped along the grid's cells (W1) |
| Site | Closed hollows over 0.25 m at 1 m | 0.0–0.7 % off rivers outside the mountains (G1b; G1a 0.8–12 %); glacial mountains 3.3–5.5 %; a river's trough 1.3 % of one hills window (W1) | under 1 % outside glacial and karst land | Bare patches as flat coloured blobs |
| Ground | What lies on it | one green sward, pixel-art sprites | litter, pebbles, moss, roots by biome | A lawn in every biome, tundra and boreal included |
| Water | Cube edges; rivers in channels that hold them | blocks in 1 m steps; chords | none; all | (stream shots) |
| Trees | How drawn; how placed | meshes within the render distance, their skeletons' wood and leaf cards (S5 (c), D300); the distant terrain's crowns beyond; each site alone | meshes (S5), instances and impostors far (L1); spaced by competition, clumped | Round stems, real crowns near (`docs/review/s5/world/`); boxes past the render distance |
| Light | Cast shadows | the sun's: near to 320 m (soft as its disc to 3.7 cm), far to 12 km (R1a) | the sun's, near and far | Trees, grass and people cast at a low sun; gentle land too smooth to (G1a) |
| Distance | Seam ratio across the edge of full detail (median); outer band's brightness | forests 4.6, open land 3.7; −46 to +89 % | ≤ 1.3; ± 5 % | The forest's edge, the lighter LOD band, a disc of sprites |
| Weather | Wet hours, London; the year against its normal | 6.0 %; +5 % (D296) | 6–8 %; ± 10 % | — |
| Weather | Cloud types; fog | one layer; none | types by situation; valley and coastal fog | One cloud layer |
| Images | Spectral α underfoot · at eye height · from the hill; colourfulness at eye height | 2.3 · 1.3 · 1.0; 54 (R1a; T2 1.9 · 1.2 · 1.0; 60) | 1.8–2.4; 15–60 | Shade breaks the pixels underfoot; cube edges remain |
| Biomes | Landforms present (dunes, mesas, scree, surf) | none (no dunes; no Mesa land) | each where it belongs | A dune sea without dunes |
