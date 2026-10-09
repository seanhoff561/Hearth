# The realism references (T §3.2)

What the realism analysis compares the generated Earth against. **No reference data or picture
is kept in the repository or shipped:** `scripts/fetch-realism-refs.sh` fetches them into
`bench-out/realism/refs/` (git-ignored). Committed are only statistics derived from them,
pictures of the generator's own output, and shaded relief of the public-domain models of the five
places in the United States (USGS 3DEP and SRTM) beside the generator's, credited where shown.

## Elevation

| Set | What | Licence | Used for |
|---|---|---|---|
| USGS 3D Elevation Program, 1 m lidar DEMs (`prd-tnm.s3.amazonaws.com/StagedProducts/Elevation/1m/Projects/…`) | Five 10 km tiles: `OR_SouthCoast_2019_A19` x41y481 (Oregon Coast Range, forested hills), `IA_SouthCentral_2020_D20` x45y454 (south-central Iowa, rolling plains), `MT_GlacierNP_2016` x29y538 (Glacier National Park, glaciated mountains), `NV_ClarkCounty_2018_C19` x74y407 (Mojave Desert, Nevada), `MN_LakeCounty_2018_C20` x61y530 (north-east Minnesota, boreal shield and lakes) | Public domain (a work of the U.S. Government) | Walking scale: `bench realism terrain` (1 km windows at 1 m and 4 m) |
| AWS Terrain Tiles (Mapzen/Tilezen `elevation-tiles-prod`), "skadi" one-arc-second HGT | `N43W124`, `N41W094`, `N48W114`, `N36W115`, `N47W092` (the same five places), `N46E007` (the Bernese Alps) | Built from public-domain sets (SRTM, NASA; 3DEP, USGS; ETOPO1, NOAA) and national models with their own attribution terms, listed at <https://github.com/tilezen/joerd/blob/master/docs/attribution.md> | The view from a hill: `bench realism terrain` (30 km windows at 30 m) |
| AWS Terrain Tiles, "terrarium" PNGs | All 64 tiles at zoom 3 (the land mask); 200 tiles at zoom 12 at random over the land between 60° S and 60° N (pixels 38 m × cos latitude) | As above | The planet's land as a whole: `bench realism global` |

## Photographs

Openly licensed photographs from Wikimedia Commons, matched to the realism suite's biomes and
scales (`tools/shots/realism.shots`): chosen by `bench realism photo-pick` from Commons'
search, only CC0, public-domain, CC BY and CC BY-SA files at least 1200 pixels wide; each file's
page, author and licence in `bench-out/realism/refs/photos/photos.tsv`. They are fetched on the
owner's machine (`scripts/fetch-realism-refs.sh --photos`; the build machine's network does
not reach Commons) and laid beside the renders in `bench-out/realism/sheet.html`. Used for
comparison only, never copied into the game, its guide or its marketing.

## Published statistics

| Source | What it gives the analysis |
|---|---|
| Leopold, L. B. and Maddock, T. (1953). *The hydraulic geometry of stream channels.* USGS Professional Paper 252 | Channel width and depth from discharge (already the generator's) |
| Hack, J. T. (1957). *Studies of longitudinal stream profiles in Virginia and Maryland.* USGS Professional Paper 294-B; Flint, J. J. (1974). *Water Resources Research* 10, 969–973 | Channel concavity θ of S ∝ A^−θ: 0.4–0.7 in most rivers |
| Montgomery, D. R. and Dietrich, W. E. (1988, 1992). *Nature* 336, 232–234; *Science* 255, 826–830 | Hillslopes turn into channels at 10³–10⁵ m² of drainage area, the slope–area relation's roll-over |
| Roering, J. J., Kirchner, J. W. and Dietrich, W. E. (1999). *Water Resources Research* 35, 853–870 | Soil-mantled hillslopes: convex crests, planar flanks near a critical slope of some 40° |
| Perron, J. T., Kirchner, J. W. and Dietrich, W. E. (2008, 2009). *JGR* 113, F04003; *Nature* 460, 502–505 | Landscapes' spectra fall as a power law and roll over at a characteristic ridge–valley spacing (some 100–400 m in soil-mantled hills); evenly spaced valleys |
| Strahler, A. N. (1952). *GSA Bulletin* 63, 1117–1142 | The hypsometric integral of a drainage basin |
| van der Schaaf, A. and van Hateren, J. H. (1996). *Vision Research* 36, 2759–2770; Torralba, A. and Oliva, A. (2003). *Network: Computation in Neural Systems* 14, 391–412 | Natural scenes' power spectra fall as 1/f^α with α near 2 (about 1.8–2.4) |
| Hasler, D. and Süsstrunk, S. (2003). *Proc. SPIE* 5007, 87–95 | The colourfulness measure; natural photographs mostly 15–60 |
