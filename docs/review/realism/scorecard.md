# The realism scorecard (T §3.6)

How the generated Earth stands against the real one, scale by scale: each measure now against
its target, and the last realism suite's verdict. Updated at every audit (the measures by
`bench realism`, the suite by `tools/shots/realism.shots`; RGA-1 for the evidence). Seed 7.

**Last update:** R1a, 2026-10-09 (the Light and Images rows; the rest T2's, RGA-1). **Top open
gaps:** the relief from 30 m to 30 km (half the Earth's over all land, a sixth in hill country);
trees, grass and water drawn as blocks and sprites; indirect light.

| Scale / system | Measure | Now | Target | Suite verdict |
|---|---|---|---|---|
| Planet | Land share; mean land height; mean ocean depth | 29.0 %; 791 m; 3,640 m | 29.2 %; 797–840 m; 3,682 m | The globe reads as the Earth's |
| Planet | Surface above 2 km; below 6 km | 1.6 %; 0.14 % | ~3.5 %; ~1 % | — |
| Regions | Channels on the grid's eight directions | combs, spokes, chords | none | Rivers zigzag |
| Landscapes, all land | Windows' median slope; median relief; land over 5° | 1.5°; 78 m; 16 % | 2.8°; 163 m; 33 % (± 20 %) | Flat wherever the land is not a mountain range |
| Landscapes, by height | Median relief of windows at 0–200 · 200–500 · 500–1,000 m · 1–2 · over 2 km | 33 · 50 · 89 · 299 · 1,515 m | 73 · 80 · 231 · 458 · 1,170 m (± 25 %) | Lowlands flat, high mountains too steep |
| Landscapes, by kind | Slope p50 at 30 m: hills · plains · desert · boreal · mountains | 1.8 · 2.3 · 2.4 · 1.1 · 29.5° | 21.8 · 3.3 · 4.2 · 2.1 · 24.6° (± 25 %) | Hill country without hills; mountains without valleys |
| Hillslopes | Mean \|Δh\| at 4 m, hills · plains · desert · boreal | 0.24 · 0.15 · 0.14 · 0.08 m | 1.38 · 0.19–0.51 · 0.46 · 0.21 m (× 1.5) | No gullies, fans, scree or terraces |
| Hillslopes | Curvature at 4 m, p95, hills | 0.015 /m | 0.12 /m (× 1.5) | — |
| Site | Closed hollows over 0.25 m at 1 m | 1–6 % | under 1 % outside glacial and karst land | Bare patches as flat coloured blobs |
| Ground | What lies on it | one green sward, pixel-art sprites | litter, pebbles, moss, roots by biome | A lawn in every biome, tundra and boreal included |
| Water | Cube edges; rivers in channels that hold them | blocks in 1 m steps; chords | none; all | (stream shots) |
| Trees | How drawn; how placed | voxel templates; each site alone | meshes (S5); spaced by competition, clumped | Blocky crowns, branch bars |
| Light | Cast shadows | the sun's: near to 320 m (soft as its disc to 3.7 cm), far to 12 km (R1a) | the sun's, near and far | Trees, grass and people cast at a low sun; gentle land too smooth to (G1a) |
| Distance | Seam ratio across the edge of full detail (median); outer band's brightness | forests 4.6, open land 3.7; −46 to +89 % | ≤ 1.3; ± 5 % | The forest's edge, the lighter LOD band, a disc of sprites |
| Weather | Wet hours, London; the year against its normal | 6.0 %; +5 % (D296) | 6–8 %; ± 10 % | — |
| Weather | Cloud types; fog | one layer; none | types by situation; valley and coastal fog | One cloud layer |
| Images | Spectral α underfoot · at eye height · from the hill; colourfulness at eye height | 2.3 · 1.3 · 1.0; 54 (R1a; T2 1.9 · 1.2 · 1.0; 60) | 1.8–2.4; 15–60 | Shade breaks the pixels underfoot; cube edges remain |
| Biomes | Landforms present (dunes, mesas, scree, surf) | none (no dunes; no Mesa land) | each where it belongs | A dune sea without dunes |
