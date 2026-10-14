# G1b — walking scale (review)

The ground a player walks on, a metre at a time, beside one-metre lidar of the same kind of land
(USGS 3DEP; `docs/review/realism/references.md`), and G1a's ground at the same places (D301).

- `walking_1m_window1.jpg`, `walking_1m_window2.jpg`: each kind's typical and upper-quartile
  window of a kilometre, lit from the north-west; lidar, G1a and G1b side by side, closed hollows
  deeper than 0.25 m in red. G1a's and G1b's are the same place: the G1b run picked them and the
  G1a build (commit 89cb06b) measured them by `REALISM_WINDOWS`.
- `terrain_earth_g1a_same_windows.md`: G1a's measures at those windows. G1b's are
  `docs/review/realism/measures/terrain_earth.md`.

## Against lidar
Each generated window's measures as a share of its lidar window's (1.00 the same; the target is
× 1.5 either way): mean |Δh| at lags of 1–64 m, the curvature's 95th percentile at 4 m, and the
share of the ground in closed hollows deeper than 0.25 m (the target under 1 % outside glacial and
karst land; in brackets, those with no river in them).

| Land | Window | G1a: \|Δh\| at 1 · 4 · 16 · 64 m | 4 m curvature | Hollows | G1b: \|Δh\| at 1 · 4 · 16 · 64 m | 4 m curvature | Hollows (off rivers) |
|---|---|---|---:|---:|---|---:|---:|
| hills | 1 | 0.83 · 0.83 · 0.81 · 0.85 | 0.94 | 6.7 % | 1.12 · 1.13 · 1.12 · 1.01 | 1.09 | 0.1 % (0.1 %) |
| hills | 2 | 0.86 · 0.86 · 0.81 · 0.80 | 1.07 | 12.2 % | 1.11 · 1.11 · 1.10 · 0.97 | 1.06 | 1.4 % (0.1 %) |
| plains | 1 | 0.95 · 0.99 · 1.07 · 1.27 | 0.56 | 3.1 % | 1.24 · 1.11 · 1.10 · 1.15 | 0.90 | 0.3 % (0.3 %) |
| plains | 2 | 0.38 · 0.39 · 0.39 · 0.42 | 0.34 | 3.3 % | 0.56 · 0.51 · 0.49 · 0.48 | 0.53 | 0.1 % (0.1 %) |
| desert | 1 | 0.64 · 0.67 · 0.71 · 0.78 | 0.40 | 2.3 % | 0.92 · 0.79 · 0.76 · 0.74 | 0.68 | 0.7 % (0.7 %) |
| desert | 2 | 0.53 · 0.56 · 0.58 · 0.66 | 0.33 | 1.8 % | 1.34 · 1.16 · 1.12 · 1.28 | 0.87 | 0.3 % (0.3 %) |
| boreal | 1 | 0.59 · 0.72 · 0.80 · 0.87 | 0.51 | 3.7 % | 0.82 · 0.85 · 0.87 · 0.89 | 0.93 | 0.0 % (0.0 %) |
| boreal | 2 | 0.61 · 0.73 · 0.79 · 0.92 | 0.50 | 0.8 % | 0.95 · 0.97 · 0.97 · 0.99 | 0.96 | 0.0 % (0.0 %) |
| mountains | 1 | 1.62 · 1.82 · 2.24 · 2.85 | 2.00 | 6.9 % | 1.13 · 0.91 · 0.76 · 0.66 | 0.86 | 5.5 % (5.5 %) |
| mountains | 2 | 1.91 · 1.95 · 1.93 · 1.83 | 2.12 | 6.6 % | 1.21 · 0.97 · 0.79 · 0.66 | 1.32 | 3.3 % (3.3 %) |

- **Hills** (Oregon Coast Range): ridges and valleys from a metre up, gullies down the slopes; at
  30 m the windows' slope is 21.6° and 21.2°, the lidar's 21.6° and 22.2°. G1a's were bumps.
- **Plains** (south-central Iowa): the first window as rough as its lidar. The second lidar window is a valley's
  steep sides cut into the plain (7.5° at 30 m); the generator's plains are between the two
  lidar windows (|Δh| at 4 m 0.21–0.26 m against 0.19 and 0.51 m), not yet cut so.
- **Desert** (Mojave, Nevada): washes down the fans, stony knobs on the slopes. At 30 m its second window
  reads 8.4° against the lidar's 4.6°: the generator's relief is spread evenly where the real
  land has smooth fans and steep range fronts (G1a's open item).
- **Boreal** (north-east Minnesota): gentle, drained, a little rougher than lidar at 30 m (2.5° against 2.1°
  over the site).
- **Mountains** (the Teton Range, glacial: its hollows allowed): rough at a metre, 0.66 of the lidar's relief at
  64 m; grainy where the lidar has cliffs and smooth talus.
- The red along the river in the hills' second window is a carved river's trough standing below
  its water's level (W1); the flat grey patches are lakes, their shores stepped (W1).

## What changed the hollows
The walking level's cells drained, but the blocks read between them. Four causes, each measured
by taking it away (the hills' two windows, hollows over 0.25 m):

| | Window 1 | Window 2 |
|---|---:|---:|
| All four fixed | 0.03 % | 0.10 % |
| Channels stepping across corners | 0.27 % | 1.58 % |
| Drained to diagonal neighbours (D8) | 0.20 % | 0.25 % |
| Catmull-Rom between cells | 0.07 % | 0.15 % |
| Soil's mounds on the channels' beds | — | 0.93 % |

## Commands
`cargo run -p bench --profile dev-opt -- realism terrain --out bench-out/realism` (with the lidar
and SRTM fetched by `scripts/fetch-realism-refs.sh`); to measure another build at the same
places, `REALISM_WINDOWS='hills:x,z,x,z;plains:…'` from the `windows at` lines it prints.
