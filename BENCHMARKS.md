# Benchmarks

Results of `hearth bench` (newest last). Frame times come from an offscreen frame loop with two frames in flight and no presentation; GPU times from timestamps written between passes. See `docs/perf-audit.md`.

## Baseline before the audit — commit 2404584, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS | p99 ms | GPU ms | CPU ms | Draws | Triangles | VRAM MiB | Upload KiB/frame (max) | Allocs/frame (max) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 966.0 | 650.2 | 1.49 | 1.01 | 0.85 | 2165 | 1.00 M | 178 | 40.0 (212) | 393.6 (467) |
| peak_lod512 | 932.2 | 510.9 | 1.61 | 0.93 | 1.02 | 893 | 1.41 M | 226 | 53.7 (58) | 376.5 (390) |
| peak_lod1024 | 871.5 | 101.1 | 1.61 | 0.97 | 1.12 | 893 | 1.41 M | 226 | 53.7 (58) | 376.5 (390) |
| coast_sunset | 1105.6 | 744.9 | 1.26 | 0.72 | 0.90 | 1153 | 0.51 M | 163 | 53.2 (188) | 393.8 (448) |
| underwater | 1043.5 | 529.7 | 1.41 | 0.63 | 0.95 | 1005 | 0.36 M | 140 | 73.7 (319) | 400.9 (474) |
| cave_torches | 1286.3 | 604.4 | 1.32 | 0.75 | 0.57 | 489 | 1.22 M | 223 | 8.8 (75) | 358.3 (391) |
| thunderstorm | 929.9 | 617.5 | 1.57 | 1.05 | 0.89 | 2165 | 1.00 M | 178 | 40.2 (212) | 398.6 (472) |

| GPU ms per pass | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| sky tables | 0.045 | 0.043 | 0.045 | 0.044 | 0.043 | 0.044 | 0.046 |
| cull 0 | 0.018 | 0.016 | 0.016 | 0.016 | 0.015 | 0.017 | 0.019 |
| terrain 0 | 0.410 | 0.076 | 0.078 | 0.145 | 0.173 | 0.146 | 0.415 |
| hi-z | 0.058 | 0.055 | 0.058 | 0.056 | 0.050 | 0.056 | 0.059 |
| cull 1 | 0.009 | 0.010 | 0.011 | 0.011 | 0.009 | 0.009 | 0.009 |
| terrain 1 | 0.010 | 0.009 | 0.010 | 0.009 | 0.009 | 0.009 | 0.009 |
| lod terrain | 0.251 | 0.532 | 0.546 | 0.151 | 0.086 | 0.401 | 0.259 |
| sky, translucent, rain | 0.145 | 0.136 | 0.148 | 0.238 | 0.200 | 0.015 | 0.178 |
| metering | 0.021 | 0.022 | 0.025 | 0.021 | 0.018 | 0.020 | 0.021 |
| tonemap | 0.042 | 0.027 | 0.030 | 0.031 | 0.025 | 0.034 | 0.033 |

| CPU ms per system | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.367 | 0.358 | 0.361 | 0.328 | 0.356 | 0.228 | 0.366 |
| lod selection | 0.001 | 0.000 | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 |
| rain cover map | 0.001 | 0.000 | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 |
| prepare: terrain | 0.178 | 0.335 | 0.339 | 0.264 | 0.292 | 0.020 | 0.186 |
| prepare: lod | 0.027 | 0.031 | 0.031 | 0.025 | 0.019 | 0.031 | 0.036 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.002 |
| encode | 0.037 | 0.040 | 0.041 | 0.036 | 0.037 | 0.041 | 0.041 |
| submit | 0.238 | 0.259 | 0.345 | 0.244 | 0.246 | 0.251 | 0.261 |
| wait for gpu | 0.182 | 0.047 | 0.028 | 0.004 | 0.004 | 0.202 | 0.178 |

| Scene | Visible cubes | LOD tiles drawn | Setup s | SSIM vs golden | What |
|---|---:|---:|---:|---:|---|
| lowland_forest | 1048 | 384 | 4.4 | – | flight over broadleaf forest and a river at 46° N, summer noon |
| peak_lod512 | 2350 | 453 | 2.8 | – | a full turn on a volcano's summit (420), LOD distance 512 |
| peak_lod1024 | 2350 | 453 | 2.7 | – | the same turn, LOD distance 1024 |
| coast_sunset | 1601 | 393 | 4.7 | – | along a rocky shore at 24° N looking out to sea at sunset |
| underwater | 1650 | 289 | 3.9 | – | six blocks under the sea off the same shore, afternoon |
| cave_torches | 10 | 449 | 3.0 | – | through the longest cave under a hill, lit by torches, at night |
| thunderstorm | 1048 | 384 | 4.1 | – | the forest flight under a heavy thunderstorm (16 mm/h, overcast) |

