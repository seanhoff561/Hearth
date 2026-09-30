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

## Distant trees (D56) — commit 1e04b39 + the distant-trees change, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS | p99 ms | GPU ms | CPU ms | Draws | Triangles | VRAM MiB | Upload KiB/frame (max) | Allocs/frame (max) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 873.1 | 728.2 | 1.35 | 1.12 | 0.80 | 2165 | 1.56 M | 236 | 40.0 (212) | 393.6 (467) |
| peak_lod512 | 918.4 | 566.9 | 1.38 | 1.00 | 0.98 | 894 | 1.67 M | 253 | 53.7 (58) | 376.6 (388) |
| peak_lod1024 | 913.5 | 567.4 | 1.41 | 0.98 | 0.99 | 894 | 1.67 M | 253 | 53.7 (58) | 376.5 (388) |
| coast_sunset | 1014.5 | 589.2 | 1.56 | 0.71 | 0.98 | 1153 | 0.52 M | 166 | 53.2 (188) | 393.8 (448) |
| underwater | 1033.8 | 577.6 | 1.34 | 0.62 | 0.96 | 1005 | 0.36 M | 141 | 73.7 (319) | 400.9 (474) |
| cave_torches | 1227.8 | 615.7 | 1.34 | 0.79 | 0.56 | 489 | 1.33 M | 235 | 8.8 (75) | 358.3 (391) |
| thunderstorm | 781.0 | 469.4 | 1.90 | 1.24 | 0.97 | 2165 | 1.56 M | 236 | 40.2 (212) | 398.6 (472) |

| GPU ms per pass | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| sky tables | 0.042 | 0.042 | 0.042 | 0.043 | 0.042 | 0.043 | 0.044 |
| cull 0 | 0.017 | 0.016 | 0.015 | 0.016 | 0.014 | 0.017 | 0.019 |
| terrain 0 | 0.379 | 0.075 | 0.075 | 0.141 | 0.168 | 0.150 | 0.410 |
| hi-z | 0.054 | 0.054 | 0.053 | 0.055 | 0.049 | 0.056 | 0.059 |
| cull 1 | 0.009 | 0.010 | 0.010 | 0.010 | 0.009 | 0.009 | 0.010 |
| terrain 1 | 0.009 | 0.009 | 0.009 | 0.009 | 0.009 | 0.010 | 0.010 |
| lod terrain | 0.421 | 0.611 | 0.600 | 0.154 | 0.082 | 0.437 | 0.459 |
| sky, translucent, rain | 0.137 | 0.135 | 0.132 | 0.226 | 0.204 | 0.013 | 0.176 |
| metering | 0.019 | 0.019 | 0.019 | 0.024 | 0.017 | 0.020 | 0.022 |
| tonemap | 0.035 | 0.028 | 0.027 | 0.028 | 0.025 | 0.034 | 0.031 |

| CPU ms per system | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.361 | 0.365 | 0.364 | 0.339 | 0.359 | 0.225 | 0.380 |
| lod selection | 0.001 | 0.000 | 0.000 | 0.001 | 0.001 | 0.001 | 0.002 |
| rain cover map | 0.001 | 0.000 | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 |
| prepare: terrain | 0.164 | 0.321 | 0.321 | 0.288 | 0.292 | 0.019 | 0.208 |
| prepare: lod | 0.025 | 0.030 | 0.031 | 0.030 | 0.020 | 0.030 | 0.033 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.002 |
| encode | 0.034 | 0.034 | 0.036 | 0.044 | 0.038 | 0.039 | 0.048 |
| submit | 0.216 | 0.231 | 0.242 | 0.277 | 0.252 | 0.240 | 0.298 |
| wait for gpu | 0.339 | 0.105 | 0.098 | 0.004 | 0.004 | 0.256 | 0.305 |

| Scene | Visible cubes | LOD tiles drawn | Setup s | SSIM vs golden | What |
|---|---:|---:|---:|---:|---|
| lowland_forest | 1048 | 384 | 4.6 | – | flight over broadleaf forest and a river at 46° N, summer noon |
| peak_lod512 | 2350 | 454 | 3.0 | – | a full turn on a volcano's summit (420), LOD distance 512 |
| peak_lod1024 | 2350 | 454 | 3.0 | – | the same turn, LOD distance 1024 |
| coast_sunset | 1601 | 393 | 5.2 | – | along a rocky shore at 24° N looking out to sea at sunset |
| underwater | 1650 | 289 | 4.2 | – | six blocks under the sea off the same shore, afternoon |
| cave_torches | 10 | 449 | 3.3 | – | through the longest cave under a hill, lit by torches, at night |
| thunderstorm | 1048 | 384 | 4.4 | – | the forest flight under a heavy thunderstorm (16 mm/h, overcast) |

