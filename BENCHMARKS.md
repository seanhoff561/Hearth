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

## 1. Sky light cache — commit f2378b4, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS | p99 ms | GPU ms | CPU ms | Draws | Triangles | VRAM MiB | Upload KiB/frame (max) | Allocs/frame (max) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 857.8 | 718.2 | 1.38 | 1.14 | 0.47 | 2165 | 1.56 M | 236 | 40.0 (212) | 391.6 (465) |
| peak_lod512 | 939.9 | 514.5 | 1.54 | 1.04 | 0.68 | 894 | 1.67 M | 253 | 53.7 (58) | 374.6 (386) |
| peak_lod1024 | 963.0 | 576.3 | 1.40 | 1.01 | 0.64 | 894 | 1.67 M | 253 | 53.7 (58) | 374.6 (386) |
| coast_sunset | 1451.7 | 832.7 | 1.06 | 0.66 | 0.58 | 1153 | 0.52 M | 166 | 53.2 (188) | 391.9 (446) |
| underwater | 1492.2 | 676.4 | 1.08 | 0.64 | 0.59 | 1005 | 0.36 M | 141 | 73.7 (319) | 398.9 (473) |
| cave_torches | 1272.7 | 634.0 | 1.20 | 0.76 | 0.34 | 489 | 1.33 M | 235 | 8.8 (75) | 356.3 (389) |
| thunderstorm | 839.2 | 688.3 | 1.41 | 1.17 | 0.50 | 2165 | 1.56 M | 236 | 40.2 (212) | 396.6 (470) |

| GPU ms per pass | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| sky tables | 0.043 | 0.046 | 0.043 | 0.042 | 0.045 | 0.042 | 0.043 |
| cull 0 | 0.017 | 0.016 | 0.016 | 0.015 | 0.015 | 0.016 | 0.018 |
| terrain 0 | 0.384 | 0.075 | 0.075 | 0.137 | 0.183 | 0.143 | 0.387 |
| hi-z | 0.055 | 0.056 | 0.055 | 0.054 | 0.052 | 0.054 | 0.055 |
| cull 1 | 0.009 | 0.011 | 0.011 | 0.010 | 0.009 | 0.009 | 0.009 |
| terrain 1 | 0.009 | 0.010 | 0.009 | 0.009 | 0.009 | 0.009 | 0.009 |
| lod terrain | 0.431 | 0.634 | 0.619 | 0.140 | 0.084 | 0.424 | 0.430 |
| sky, translucent, rain | 0.139 | 0.141 | 0.136 | 0.209 | 0.196 | 0.013 | 0.166 |
| metering | 0.020 | 0.020 | 0.020 | 0.019 | 0.018 | 0.019 | 0.020 |
| tonemap | 0.036 | 0.029 | 0.029 | 0.027 | 0.026 | 0.033 | 0.030 |

| CPU ms per system | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.005 | 0.007 | 0.006 | 0.009 | 0.007 | 0.008 | 0.005 |
| lod selection | 0.001 | 0.000 | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 |
| rain cover map | 0.001 | 0.000 | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 |
| prepare: terrain | 0.175 | 0.349 | 0.334 | 0.277 | 0.295 | 0.019 | 0.183 |
| prepare: lod | 0.026 | 0.032 | 0.030 | 0.025 | 0.019 | 0.032 | 0.026 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.002 |
| encode | 0.036 | 0.040 | 0.037 | 0.036 | 0.035 | 0.041 | 0.039 |
| submit | 0.230 | 0.248 | 0.228 | 0.232 | 0.231 | 0.237 | 0.241 |
| wait for gpu | 0.688 | 0.384 | 0.399 | 0.104 | 0.079 | 0.444 | 0.689 |

| Allocations per frame | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.000 | 0.000 | 0.000 | 0.015 | 0.060 | 0.030 | 0.000 |
| lod selection | 0.440 | 0.000 | 0.000 | 0.332 | 0.210 | 0.335 | 0.440 |
| rain cover map | 0.080 | 0.000 | 0.000 | 0.053 | 0.040 | 0.053 | 0.080 |
| prepare: terrain | 64.992 | 60.327 | 60.327 | 64.575 | 71.030 | 40.605 | 69.850 |
| prepare: lod | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| encode | 91.692 | 92.000 | 92.000 | 91.792 | 91.630 | 91.753 | 91.840 |
| submit | 231.397 | 219.283 | 219.278 | 232.458 | 233.485 | 220.583 | 231.392 |
| wait for gpu | 1.003 | 0.958 | 0.965 | 0.678 | 0.510 | 0.970 | 1.000 |

| Scene | Visible cubes | LOD tiles drawn | Meshing cubes/s | LOD tiles/s | Setup s | SSIM vs golden | Slowest frame | What |
|---|---:|---:|---:|---:|---:|---:|---|---|
| lowland_forest | 1048 | 384 | 102765 | 1756 | 4.7 | 0.9998 | 1.42 ms, wait for gpu 0.96 | flight over broadleaf forest and a river at 46° N, summer noon |
| peak_lod512 | 2350 | 454 | 93880 | 1620 | 3.1 | 0.9998 | 2.89 ms, submit 2.53 | a full turn on a volcano's summit (420), LOD distance 512 |
| peak_lod1024 | 2350 | 454 | 93039 | 1725 | 3.0 | 0.9998 | 2.91 ms, submit 2.55 | the same turn, LOD distance 1024 |
| coast_sunset | 1601 | 393 | 69801 | 1839 | 5.1 | 0.9995 | 1.73 ms, environment 1.19 | along a rocky shore at 24° N looking out to sea at sunset |
| underwater | 1650 | 289 | 71369 | 1901 | 4.1 | 0.9999 | 2.89 ms, submit 2.55 | six blocks under the sea off the same shore, afternoon |
| cave_torches | 10 | 449 | 83055 | 1826 | 3.4 | 1.0000 | 2.75 ms, submit 2.65 | through the longest cave under a hill, lit by torches, at night |
| thunderstorm | 1048 | 384 | 101105 | 1561 | 4.5 | 0.9999 | 1.55 ms, wait for gpu 0.99 | the forest flight under a heavy thunderstorm (16 mm/h, overcast) |

## 2. No per-frame allocations in our frame code; Hi-Z bind group cached — commit bae343b, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS | p99 ms | GPU ms | CPU ms | Draws | Triangles | VRAM MiB | Upload KiB/frame (max) | Allocs/frame (max) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 866.2 | 556.7 | 1.76 | 1.13 | 0.56 | 2165 | 1.56 M | 236 | 40.0 (212) | 350.0 (423) |
| peak_lod512 | 910.9 | 427.2 | 2.00 | 1.06 | 0.68 | 894 | 1.67 M | 253 | 53.7 (58) | 329.6 (341) |
| peak_lod1024 | 968.7 | 566.7 | 1.36 | 1.01 | 0.57 | 894 | 1.67 M | 253 | 53.7 (58) | 329.6 (341) |
| coast_sunset | 1364.4 | 641.1 | 1.50 | 0.70 | 0.54 | 1153 | 0.52 M | 166 | 53.2 (188) | 348.8 (403) |
| underwater | 1477.8 | 659.3 | 1.16 | 0.62 | 0.60 | 1005 | 0.36 M | 141 | 73.7 (319) | 355.8 (428) |
| cave_torches | 1213.3 | 525.5 | 1.45 | 0.80 | 0.40 | 489 | 1.33 M | 235 | 8.8 (75) | 332.9 (365) |
| thunderstorm | 874.3 | 735.1 | 1.35 | 1.12 | 0.52 | 2165 | 1.56 M | 236 | 40.2 (212) | 355.0 (428) |

| GPU ms per pass | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| sky tables | 0.042 | 0.043 | 0.043 | 0.047 | 0.044 | 0.044 | 0.041 |
| cull 0 | 0.018 | 0.016 | 0.016 | 0.015 | 0.015 | 0.016 | 0.017 |
| terrain 0 | 0.383 | 0.078 | 0.076 | 0.149 | 0.179 | 0.148 | 0.371 |
| hi-z | 0.055 | 0.058 | 0.054 | 0.054 | 0.050 | 0.057 | 0.053 |
| cull 1 | 0.010 | 0.012 | 0.010 | 0.011 | 0.009 | 0.009 | 0.009 |
| terrain 1 | 0.009 | 0.009 | 0.009 | 0.009 | 0.009 | 0.009 | 0.009 |
| lod terrain | 0.420 | 0.648 | 0.617 | 0.148 | 0.082 | 0.444 | 0.413 |
| sky, translucent, rain | 0.136 | 0.143 | 0.136 | 0.225 | 0.187 | 0.013 | 0.159 |
| metering | 0.019 | 0.023 | 0.019 | 0.020 | 0.018 | 0.020 | 0.019 |
| tonemap | 0.035 | 0.031 | 0.028 | 0.028 | 0.026 | 0.035 | 0.029 |

| CPU ms per system | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.007 | 0.007 | 0.006 | 0.009 | 0.008 | 0.009 | 0.006 |
| lod selection | 0.002 | 0.000 | 0.000 | 0.001 | 0.001 | 0.002 | 0.002 |
| rain cover map | 0.001 | 0.000 | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 |
| prepare: terrain | 0.178 | 0.311 | 0.275 | 0.222 | 0.260 | 0.021 | 0.162 |
| prepare: lod | 0.029 | 0.032 | 0.030 | 0.023 | 0.020 | 0.032 | 0.028 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.002 |
| encode | 0.045 | 0.045 | 0.037 | 0.043 | 0.045 | 0.052 | 0.048 |
| submit | 0.296 | 0.280 | 0.227 | 0.238 | 0.266 | 0.285 | 0.273 |
| wait for gpu | 0.592 | 0.418 | 0.455 | 0.192 | 0.074 | 0.419 | 0.618 |

| Allocations per frame | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.000 | 0.000 | 0.000 | 0.015 | 0.060 | 0.030 | 0.000 |
| lod selection | 0.440 | 0.000 | 0.000 | 0.332 | 0.210 | 0.335 | 0.440 |
| rain cover map | 0.080 | 0.000 | 0.000 | 0.053 | 0.040 | 0.053 | 0.080 |
| prepare: terrain | 37.368 | 29.327 | 29.327 | 35.510 | 41.862 | 31.227 | 42.227 |
| prepare: lod | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| encode | 77.692 | 78.000 | 78.000 | 77.792 | 77.630 | 77.753 | 77.840 |
| submit | 231.400 | 219.343 | 219.265 | 232.335 | 233.442 | 220.587 | 231.392 |
| wait for gpu | 1.000 | 0.897 | 0.978 | 0.800 | 0.553 | 0.967 | 1.000 |

| Scene | Visible cubes | LOD tiles drawn | Meshing cubes/s | LOD tiles/s | Setup s | SSIM vs golden | Slowest frame | What |
|---|---:|---:|---:|---:|---:|---:|---|---|
| lowland_forest | 1048 | 384 | 97348 | 1598 | 5.1 | 0.9998 | 1.83 ms, wait for gpu 1.11 | flight over broadleaf forest and a river at 46° N, summer noon |
| peak_lod512 | 2350 | 454 | 92122 | 1661 | 3.2 | 0.9998 | 3.13 ms, submit 2.74 | a full turn on a volcano's summit (420), LOD distance 512 |
| peak_lod1024 | 2350 | 454 | 87772 | 1663 | 3.1 | 0.9998 | 3.10 ms, submit 2.74 | the same turn, LOD distance 1024 |
| coast_sunset | 1601 | 393 | 68148 | 1665 | 5.4 | 0.9995 | 1.61 ms, wait for gpu 1.09 | along a rocky shore at 24° N looking out to sea at sunset |
| underwater | 1650 | 289 | 67773 | 1877 | 4.3 | 0.9999 | 3.19 ms, submit 2.81 | six blocks under the sea off the same shore, afternoon |
| cave_torches | 10 | 449 | 86758 | 1799 | 3.4 | 1.0000 | 3.80 ms, submit 3.62 | through the longest cave under a hill, lit by torches, at night |
| thunderstorm | 1048 | 384 | 92771 | 1565 | 4.8 | 0.9999 | 1.38 ms, wait for gpu 0.60 | the forest flight under a heavy thunderstorm (16 mm/h, overcast) |

## 3. LOD quads pooled as 16-byte records, one indirect multi-draw — commit 564e0bc, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS | p99 ms | GPU ms | CPU ms | Draws | Triangles | VRAM MiB | Upload KiB/frame (max) | Allocs/frame (max) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 838.9 | 526.2 | 1.80 | 1.16 | 0.44 | 2165 | 1.56 M | 163 | 47.5 (219) | 363.0 (436) |
| peak_lod512 | 962.5 | 637.4 | 1.29 | 1.01 | 0.49 | 894 | 1.67 M | 162 | 62.5 (67) | 344.6 (354) |
| peak_lod1024 | 976.4 | 651.6 | 1.27 | 1.00 | 0.48 | 894 | 1.67 M | 162 | 62.5 (67) | 344.6 (354) |
| coast_sunset | 1367.9 | 811.4 | 1.05 | 0.70 | 0.46 | 1153 | 0.52 M | 133 | 60.9 (195) | 361.9 (416) |
| underwater | 1558.3 | 870.2 | 0.79 | 0.62 | 0.46 | 1005 | 0.36 M | 117 | 79.3 (325) | 368.8 (441) |
| cave_torches | 1309.2 | 761.3 | 1.03 | 0.74 | 0.24 | 489 | 1.33 M | 163 | 17.5 (84) | 347.5 (378) |
| thunderstorm | 862.7 | 565.1 | 1.72 | 1.14 | 0.38 | 2165 | 1.56 M | 163 | 47.7 (219) | 368.0 (441) |

| GPU ms per pass | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| sky tables | 0.046 | 0.043 | 0.043 | 0.045 | 0.043 | 0.044 | 0.043 |
| cull 0 | 0.018 | 0.015 | 0.015 | 0.015 | 0.014 | 0.016 | 0.017 |
| terrain 0 | 0.401 | 0.075 | 0.074 | 0.149 | 0.178 | 0.148 | 0.384 |
| hi-z | 0.057 | 0.053 | 0.053 | 0.051 | 0.048 | 0.053 | 0.054 |
| cull 1 | 0.009 | 0.010 | 0.010 | 0.010 | 0.009 | 0.009 | 0.010 |
| terrain 1 | 0.009 | 0.009 | 0.009 | 0.009 | 0.009 | 0.009 | 0.009 |
| lod terrain | 0.424 | 0.622 | 0.613 | 0.153 | 0.088 | 0.403 | 0.405 |
| sky, translucent, rain | 0.143 | 0.140 | 0.138 | 0.225 | 0.184 | 0.013 | 0.162 |
| metering | 0.021 | 0.018 | 0.018 | 0.019 | 0.017 | 0.018 | 0.020 |
| tonemap | 0.034 | 0.027 | 0.027 | 0.028 | 0.025 | 0.028 | 0.033 |

| CPU ms per system | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.006 | 0.006 | 0.006 | 0.010 | 0.009 | 0.008 | 0.005 |
| lod selection | 0.001 | 0.000 | 0.000 | 0.002 | 0.001 | 0.002 | 0.002 |
| rain cover map | 0.001 | 0.000 | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 |
| prepare: terrain | 0.173 | 0.274 | 0.272 | 0.227 | 0.235 | 0.018 | 0.149 |
| prepare: lod | 0.035 | 0.031 | 0.030 | 0.027 | 0.021 | 0.030 | 0.029 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.002 |
| encode | 0.017 | 0.014 | 0.015 | 0.016 | 0.015 | 0.014 | 0.015 |
| submit | 0.205 | 0.163 | 0.159 | 0.177 | 0.175 | 0.166 | 0.178 |
| wait for gpu | 0.751 | 0.547 | 0.538 | 0.267 | 0.184 | 0.522 | 0.775 |

| Allocations per frame | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.000 | 0.000 | 0.000 | 0.015 | 0.060 | 0.030 | 0.000 |
| lod selection | 0.440 | 0.000 | 0.000 | 0.332 | 0.210 | 0.335 | 0.440 |
| rain cover map | 0.080 | 0.000 | 0.000 | 0.053 | 0.040 | 0.053 | 0.080 |
| prepare: terrain | 42.227 | 33.658 | 33.658 | 40.418 | 46.688 | 35.587 | 47.033 |
| prepare: lod | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| encode | 69.840 | 70.668 | 70.668 | 69.898 | 69.803 | 70.393 | 70.042 |
| submit | 247.392 | 237.253 | 237.255 | 248.213 | 249.110 | 238.158 | 247.392 |
| wait for gpu | 1.002 | 0.993 | 0.993 | 0.925 | 0.898 | 0.995 | 1.000 |

| Scene | Visible cubes | LOD tiles drawn | Meshing cubes/s | LOD tiles/s | Setup s | SSIM vs golden | Slowest frame | What |
|---|---:|---:|---:|---:|---:|---:|---|---|
| lowland_forest | 1048 | 384 | 102565 | 1559 | 5.0 | 0.9997 | 2.01 ms, wait for gpu 1.46 | flight over broadleaf forest and a river at 46° N, summer noon |
| peak_lod512 | 2350 | 454 | 93657 | 1656 | 3.1 | 0.9998 | 2.90 ms, submit 2.59 | a full turn on a volcano's summit (420), LOD distance 512 |
| peak_lod1024 | 2350 | 454 | 96557 | 1713 | 3.0 | 0.9998 | 2.77 ms, submit 2.47 | the same turn, LOD distance 1024 |
| coast_sunset | 1601 | 393 | 70454 | 1796 | 5.1 | 0.9995 | 1.94 ms, environment 1.30 | along a rocky shore at 24° N looking out to sea at sunset |
| underwater | 1650 | 289 | 70874 | 1853 | 4.2 | 0.9985 | 2.82 ms, submit 2.56 | six blocks under the sea off the same shore, afternoon |
| cave_torches | 10 | 449 | 86378 | 1842 | 3.3 | 1.0000 | 2.65 ms, submit 2.59 | through the longest cave under a hill, lit by torches, at night |
| thunderstorm | 1048 | 384 | 103926 | 1759 | 4.4 | 0.9999 | 1.82 ms, wait for gpu 1.19 | the forest flight under a heavy thunderstorm (16 mm/h, overcast) |

## 4. LOD occlusion-culled on the GPU against the near terrain's Hi-Z — commit 5556f36, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS | p99 ms | GPU ms | CPU ms | Draws | Triangles | VRAM MiB | Upload KiB/frame (max) | Allocs/frame (max) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 888.7 | 613.7 | 1.54 | 1.10 | 0.38 | 2165 | 1.56 M | 163 | 58.1 (230) | 369.0 (442) |
| peak_lod512 | 973.5 | 574.7 | 1.39 | 1.00 | 0.50 | 894 | 1.67 M | 162 | 75.0 (79) | 348.6 (360) |
| peak_lod1024 | 965.8 | 564.9 | 1.41 | 1.01 | 0.52 | 894 | 1.67 M | 162 | 75.0 (79) | 348.6 (360) |
| coast_sunset | 1403.9 | 765.3 | 1.14 | 0.69 | 0.47 | 1153 | 0.52 M | 133 | 71.7 (204) | 367.9 (422) |
| underwater | 1387.3 | 613.3 | 1.35 | 0.69 | 0.47 | 1005 | 0.36 M | 117 | 87.3 (333) | 374.8 (447) |
| cave_torches | 2320.2 | 714.0 | 0.96 | 0.40 | 0.25 | 489 | 1.33 M | 163 | 29.9 (96) | 351.9 (384) |
| thunderstorm | 816.0 | 550.4 | 1.80 | 1.20 | 0.42 | 2165 | 1.56 M | 163 | 58.3 (230) | 374.0 (447) |

| GPU ms per pass | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| sky tables | 0.043 | 0.043 | 0.044 | 0.044 | 0.048 | 0.048 | 0.045 |
| cull 0 | 0.018 | 0.015 | 0.016 | 0.015 | 0.017 | 0.018 | 0.018 |
| terrain 0 | 0.387 | 0.075 | 0.076 | 0.144 | 0.197 | 0.159 | 0.411 |
| hi-z | 0.054 | 0.053 | 0.055 | 0.050 | 0.058 | 0.059 | 0.055 |
| cull 1 | 0.009 | 0.010 | 0.010 | 0.010 | 0.011 | 0.010 | 0.010 |
| terrain 1 | 0.009 | 0.009 | 0.009 | 0.009 | 0.011 | 0.011 | 0.010 |
| lod cull | 0.013 | 0.013 | 0.013 | 0.013 | 0.014 | 0.014 | 0.014 |
| lod terrain | 0.376 | 0.595 | 0.600 | 0.140 | 0.079 | 0.014 | 0.400 |
| sky, translucent, rain | 0.139 | 0.140 | 0.142 | 0.218 | 0.205 | 0.013 | 0.178 |
| metering | 0.019 | 0.019 | 0.019 | 0.018 | 0.022 | 0.020 | 0.021 |
| tonemap | 0.034 | 0.028 | 0.028 | 0.027 | 0.029 | 0.031 | 0.034 |

| CPU ms per system | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.005 | 0.007 | 0.007 | 0.010 | 0.009 | 0.008 | 0.006 |
| lod selection | 0.002 | 0.000 | 0.000 | 0.002 | 0.001 | 0.002 | 0.002 |
| rain cover map | 0.001 | 0.000 | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 |
| prepare: terrain | 0.146 | 0.280 | 0.293 | 0.227 | 0.238 | 0.018 | 0.162 |
| prepare: lod | 0.032 | 0.035 | 0.036 | 0.032 | 0.025 | 0.035 | 0.035 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.002 |
| encode | 0.015 | 0.015 | 0.016 | 0.016 | 0.015 | 0.015 | 0.017 |
| submit | 0.174 | 0.167 | 0.173 | 0.185 | 0.184 | 0.169 | 0.195 |
| wait for gpu | 0.746 | 0.519 | 0.507 | 0.235 | 0.245 | 0.179 | 0.803 |

| Allocations per frame | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.000 | 0.000 | 0.000 | 0.015 | 0.060 | 0.030 | 0.000 |
| lod selection | 0.440 | 0.000 | 0.000 | 0.332 | 0.210 | 0.335 | 0.440 |
| rain cover map | 0.080 | 0.000 | 0.000 | 0.053 | 0.040 | 0.053 | 0.080 |
| prepare: terrain | 47.033 | 38.658 | 38.658 | 45.287 | 51.488 | 40.450 | 51.060 |
| prepare: lod | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| encode | 72.042 | 72.668 | 72.668 | 72.035 | 72.003 | 72.530 | 73.060 |
| submit | 246.392 | 234.248 | 234.250 | 247.202 | 248.082 | 235.610 | 246.392 |
| wait for gpu | 1.002 | 0.993 | 0.993 | 0.937 | 0.928 | 0.942 | 1.000 |

| Scene | Visible cubes | LOD tiles drawn | Meshing cubes/s | LOD tiles/s | Setup s | SSIM vs golden | Slowest frame | What |
|---|---:|---:|---:|---:|---:|---:|---|---|
| lowland_forest | 1048 | 384 | 101461 | 1739 | 4.7 | 0.9997 | 1.71 ms, wait for gpu 1.19 | flight over broadleaf forest and a river at 46° N, summer noon |
| peak_lod512 | 2350 | 454 | 96295 | 1704 | 3.0 | 0.9998 | 2.76 ms, submit 2.47 | a full turn on a volcano's summit (420), LOD distance 512 |
| peak_lod1024 | 2350 | 454 | 96247 | 1695 | 3.1 | 0.9998 | 2.92 ms, submit 2.52 | the same turn, LOD distance 1024 |
| coast_sunset | 1601 | 393 | 73053 | 1965 | 5.0 | 0.9995 | 1.71 ms, environment 1.01 | along a rocky shore at 24° N looking out to sea at sunset |
| underwater | 1650 | 289 | 67706 | 1797 | 4.4 | 0.9983 | 2.88 ms, submit 2.61 | six blocks under the sea off the same shore, afternoon |
| cave_torches | 10 | 449 | 81870 | 1577 | 3.6 | 1.0000 | 2.77 ms, submit 2.69 | through the longest cave under a hill, lit by torches, at night |
| thunderstorm | 1048 | 384 | 97726 | 1630 | 4.5 | 0.9999 | 1.84 ms, wait for gpu 1.44 | the forest flight under a heavy thunderstorm (16 mm/h, overcast) |

## 7. Screen-space-error LOD selection (2 px, the default) — commit 4a721d8, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS | p99 ms | GPU ms | CPU ms | Draws | Triangles | VRAM MiB | Upload KiB/frame (max) | Allocs/frame (max) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 656.0 | 500.7 | 1.94 | 1.50 | 0.42 | 2223 | 1.94 M | 227 | 61.7 (233) | 369.7 (477) |
| peak_lod512 | 591.0 | 358.1 | 2.71 | 1.67 | 0.53 | 1182 | 3.14 M | 226 | 93.0 (117) | 348.7 (459) |
| peak_lod1024 | 588.8 | 223.8 | 2.69 | 1.65 | 0.56 | 1182 | 3.14 M | 226 | 93.0 (117) | 348.7 (459) |
| coast_sunset | 1131.5 | 687.5 | 1.41 | 0.86 | 0.42 | 1184 | 0.69 M | 133 | 73.6 (205) | 368.3 (453) |
| underwater | 1149.4 | 71.6 | 1.26 | 0.72 | 0.57 | 1005 | 0.36 M | 117 | 87.3 (333) | 375.1 (477) |
| cave_torches | 2197.0 | 871.8 | 0.85 | 0.43 | 0.23 | 693 | 2.41 M | 227 | 42.7 (117) | 352.4 (417) |
| thunderstorm | 642.3 | 388.7 | 1.98 | 1.53 | 0.40 | 2223 | 1.94 M | 227 | 61.9 (233) | 374.7 (482) |

| GPU ms per pass | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| sky tables | 0.051 | 0.050 | 0.050 | 0.049 | 0.046 | 0.048 | 0.052 |
| cull 0 | 0.021 | 0.018 | 0.018 | 0.016 | 0.015 | 0.015 | 0.020 |
| terrain 0 | 0.474 | 0.087 | 0.088 | 0.157 | 0.201 | 0.159 | 0.475 |
| hi-z | 0.065 | 0.064 | 0.061 | 0.058 | 0.056 | 0.059 | 0.064 |
| cull 1 | 0.011 | 0.012 | 0.012 | 0.010 | 0.009 | 0.010 | 0.011 |
| terrain 1 | 0.010 | 0.010 | 0.010 | 0.010 | 0.010 | 0.010 | 0.011 |
| lod cull | 0.014 | 0.015 | 0.016 | 0.014 | 0.013 | 0.014 | 0.014 |
| lod terrain | 0.589 | 1.149 | 1.140 | 0.206 | 0.081 | 0.015 | 0.583 |
| sky, translucent, rain | 0.171 | 0.172 | 0.165 | 0.250 | 0.205 | 0.013 | 0.205 |
| metering | 0.023 | 0.025 | 0.025 | 0.022 | 0.020 | 0.019 | 0.023 |
| tonemap | 0.069 | 0.065 | 0.067 | 0.065 | 0.062 | 0.063 | 0.067 |

| CPU ms per system | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.005 | 0.006 | 0.006 | 0.009 | 0.008 | 0.007 | 0.004 |
| lod selection | 0.011 | 0.000 | 0.000 | 0.006 | 0.004 | 0.007 | 0.012 |
| rain cover map | 0.001 | 0.000 | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 |
| prepare: terrain | 0.156 | 0.278 | 0.281 | 0.204 | 0.223 | 0.015 | 0.143 |
| prepare: lod | 0.052 | 0.059 | 0.061 | 0.033 | 0.024 | 0.045 | 0.047 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.002 |
| encode | 0.016 | 0.015 | 0.016 | 0.015 | 0.015 | 0.013 | 0.015 |
| submit | 0.179 | 0.170 | 0.197 | 0.154 | 0.294 | 0.141 | 0.176 |
| wait for gpu | 1.100 | 1.160 | 1.133 | 0.460 | 0.300 | 0.223 | 1.154 |

| Allocations per frame | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.000 | 0.000 | 0.000 | 0.015 | 0.060 | 0.030 | 0.000 |
| lod selection | 1.140 | 0.000 | 0.000 | 0.805 | 0.560 | 0.785 | 1.140 |
| rain cover map | 0.082 | 0.000 | 0.000 | 0.053 | 0.040 | 0.055 | 0.080 |
| prepare: terrain | 47.033 | 38.773 | 38.773 | 45.287 | 51.488 | 40.450 | 51.060 |
| prepare: lod | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| encode | 72.042 | 72.710 | 72.710 | 72.035 | 72.003 | 72.530 | 73.060 |
| submit | 246.393 | 234.257 | 234.260 | 247.152 | 248.058 | 235.597 | 246.393 |
| wait for gpu | 1.000 | 0.995 | 0.992 | 0.988 | 0.945 | 0.955 | 0.997 |

| Scene | Visible cubes | LOD tiles drawn | Meshing cubes/s | LOD tiles/s | Setup s | SSIM vs golden | Slowest frame | What |
|---|---:|---:|---:|---:|---:|---:|---|---|
| lowland_forest | 1048 | 442 | 106526 | 1729 | 5.3 | 0.9978 | 2.11 ms, wait for gpu 1.78 | flight over broadleaf forest and a river at 46° N, summer noon |
| peak_lod512 | 2350 | 742 | 95027 | 1687 | 3.8 | 0.9386 | 2.85 ms, submit 2.48 | a full turn on a volcano's summit (420), LOD distance 512 |
| peak_lod1024 | 2350 | 742 | 95231 | 1692 | 3.7 | 0.9386 | 12.87 ms, submit 12.54 | the same turn, LOD distance 1024 |
| coast_sunset | 1601 | 424 | 73577 | 1934 | 5.0 | 0.9996 | 1.56 ms, wait for gpu 1.12 | along a rocky shore at 24° N looking out to sea at sunset |
| underwater | 1650 | 289 | 72275 | 1873 | 4.1 | 1.0000 | 49.73 ms, submit 49.41 | six blocks under the sea off the same shore, afternoon |
| cave_torches | 10 | 653 | 85474 | 1668 | 3.8 | 1.0000 | 2.52 ms, submit 2.44 | through the longest cave under a hill, lit by torches, at night |
| thunderstorm | 1048 | 442 | 106160 | 1735 | 5.0 | 0.9996 | 5.16 ms, submit 4.94 | the forest flight under a heavy thunderstorm (16 mm/h, overcast) |

## 8. LOD quads grouped by facing, back-facing groups skipped — commit 0ad695e, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS | p99 ms | GPU ms | CPU ms | Draws | Triangles | VRAM MiB | Upload KiB/frame (max) | Allocs/frame (max) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 748.9 | 104.7 | 1.66 | 1.23 | 0.53 | 2223 | 1.26 M | 227 | 76.2 (248) | 369.7 (477) |
| peak_lod512 | 810.5 | 446.1 | 2.11 | 1.20 | 0.52 | 1182 | 1.90 M | 226 | 110.3 (146) | 348.7 (416) |
| peak_lod1024 | 806.1 | 444.5 | 2.11 | 1.21 | 0.53 | 1182 | 1.90 M | 226 | 110.3 (146) | 348.7 (416) |
| coast_sunset | 1229.4 | 739.4 | 1.30 | 0.79 | 0.43 | 1184 | 0.48 M | 133 | 84.4 (214) | 368.3 (453) |
| underwater | 1268.5 | 121.5 | 1.30 | 0.69 | 0.50 | 1005 | 0.25 M | 117 | 94.7 (340) | 375.1 (477) |
| cave_torches | 2152.3 | 881.6 | 0.82 | 0.44 | 0.24 | 693 | 1.21 M | 227 | 68.7 (161) | 352.5 (435) |
| thunderstorm | 771.0 | 570.9 | 1.73 | 1.27 | 0.40 | 2223 | 1.26 M | 227 | 76.4 (248) | 374.7 (482) |

| GPU ms per pass | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| sky tables | 0.050 | 0.050 | 0.049 | 0.048 | 0.047 | 0.049 | 0.050 |
| cull 0 | 0.020 | 0.017 | 0.018 | 0.016 | 0.016 | 0.016 | 0.020 |
| terrain 0 | 0.463 | 0.088 | 0.089 | 0.160 | 0.200 | 0.167 | 0.470 |
| hi-z | 0.064 | 0.063 | 0.063 | 0.064 | 0.057 | 0.058 | 0.065 |
| cull 1 | 0.011 | 0.012 | 0.011 | 0.011 | 0.009 | 0.010 | 0.010 |
| terrain 1 | 0.010 | 0.010 | 0.010 | 0.010 | 0.009 | 0.010 | 0.010 |
| lod cull | 0.015 | 0.015 | 0.015 | 0.014 | 0.014 | 0.015 | 0.016 |
| lod terrain | 0.331 | 0.692 | 0.700 | 0.126 | 0.042 | 0.012 | 0.335 |
| sky, translucent, rain | 0.173 | 0.168 | 0.168 | 0.251 | 0.212 | 0.014 | 0.205 |
| metering | 0.024 | 0.022 | 0.021 | 0.021 | 0.022 | 0.019 | 0.024 |
| tonemap | 0.067 | 0.064 | 0.065 | 0.067 | 0.063 | 0.066 | 0.066 |

| CPU ms per system | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.005 | 0.006 | 0.006 | 0.009 | 0.008 | 0.007 | 0.004 |
| lod selection | 0.011 | 0.000 | 0.000 | 0.006 | 0.003 | 0.007 | 0.011 |
| rain cover map | 0.001 | 0.000 | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 |
| prepare: terrain | 0.158 | 0.271 | 0.276 | 0.205 | 0.220 | 0.015 | 0.145 |
| prepare: lod | 0.070 | 0.071 | 0.073 | 0.043 | 0.029 | 0.057 | 0.058 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.002 |
| encode | 0.016 | 0.014 | 0.014 | 0.014 | 0.014 | 0.013 | 0.015 |
| submit | 0.265 | 0.161 | 0.162 | 0.149 | 0.230 | 0.139 | 0.167 |
| wait for gpu | 0.805 | 0.707 | 0.706 | 0.384 | 0.282 | 0.223 | 0.892 |

| Allocations per frame | lowland_forest | peak_lod512 | peak_lod1024 | coast_sunset | underwater | cave_torches | thunderstorm |
|---|---:|---:|---:|---:|---:|---:|---:|
| environment | 0.000 | 0.000 | 0.000 | 0.015 | 0.060 | 0.030 | 0.000 |
| lod selection | 1.140 | 0.000 | 0.000 | 0.805 | 0.560 | 0.785 | 1.140 |
| rain cover map | 0.080 | 0.000 | 0.000 | 0.055 | 0.040 | 0.055 | 0.080 |
| prepare: terrain | 47.033 | 38.773 | 38.773 | 45.287 | 51.488 | 40.523 | 51.060 |
| prepare: lod | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| prepare: sky, rain | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| encode | 72.042 | 72.710 | 72.710 | 72.035 | 72.003 | 72.572 | 73.060 |
| submit | 246.400 | 234.258 | 234.262 | 247.162 | 248.058 | 235.615 | 246.392 |
| wait for gpu | 0.993 | 0.995 | 0.995 | 0.978 | 0.945 | 0.945 | 1.000 |

| Scene | Visible cubes | LOD tiles drawn | Meshing cubes/s | LOD tiles/s | Setup s | SSIM vs golden | Slowest frame | What |
|---|---:|---:|---:|---:|---:|---:|---|---|
| lowland_forest | 1048 | 442 | 107689 | 1714 | 5.3 | – | 48.87 ms, submit 48.52 | flight over broadleaf forest and a river at 46° N, summer noon |
| peak_lod512 | 2350 | 742 | 96058 | 1665 | 3.8 | – | 2.82 ms, submit 2.50 | a full turn on a volcano's summit (420), LOD distance 512 |
| peak_lod1024 | 2350 | 742 | 96522 | 1673 | 3.7 | – | 2.82 ms, submit 2.51 | the same turn, LOD distance 1024 |
| coast_sunset | 1601 | 424 | 74351 | 1917 | 5.0 | – | 1.42 ms, environment 1.01 | along a rocky shore at 24° N looking out to sea at sunset |
| underwater | 1650 | 289 | 72502 | 1876 | 4.0 | – | 16.44 ms, submit 16.20 | six blocks under the sea off the same shore, afternoon |
| cave_torches | 10 | 653 | 88447 | 1668 | 3.8 | – | 2.59 ms, submit 2.49 | through the longest cave under a hill, lit by torches, at night |
| thunderstorm | 1048 | 442 | 107245 | 1732 | 5.0 | – | 1.77 ms, wait for gpu 1.18 | the forest flight under a heavy thunderstorm (16 mm/h, overcast) |

## End of the rendering audit — commits 038759c → 0d01d73 built side by side, alternating, two rounds each, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS (1st percentile) | GPU ms | VRAM MiB | Triangles | LOD tiles drawn | Allocs/frame |
|---|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 691 → 784 | 534 → 601 | 1.38 → 1.24 | 236 → 227 | 1.56 → 1.26 M | 384 → 442 | 394 → 370 |
| peak_lod512 | 762 → 799 | 506 → 480 | 1.28 → 1.22 | 253 → 226 | 1.67 → 1.90 M | 454 → 742 | 377 → 349 |
| peak_lod1024 | 763 → 805 | 508 → 491 | 1.28 → 1.21 | 253 → 226 | 1.67 → 1.90 M | 454 → 742 | 377 → 349 |
| coast_sunset | 1114 → 1229 | 855 → 797 | 0.76 → 0.79 | 166 → 133 | 0.52 → 0.48 M | 393 → 424 | 394 → 368 |
| underwater | 1085 → 1221 | 759 → 872 | 0.66 → 0.69 | 141 → 117 | 0.36 → 0.25 M | 289 → 289 | 401 → 375 |
| cave_torches | 1155 → 2178 | 713 → 1206 | 0.84 → 0.43 | 235 → 227 | 1.33 → 1.21 M | 449 → 653 | 358 → 353 |
| thunderstorm | 689 → 745 | 527 → 566 | 1.43 → 1.27 | 236 → 227 | 1.56 → 1.26 M | 384 → 442 | 399 → 375 |

The same session's numbers for the first commit are 10–20 % below those recorded for it at
the start of the audit (laptop clocks and temperature hours apart), which is why comparisons
are made between builds run alternately (`scripts/perf-gate.sh`, D59). The slower summit and
coast tails are accepted in D60.

## End of V2-2 — the performance gate: 2e45a05 against the end of V2-2, alternating, three rounds each, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS (1st percentile) | GPU ms | VRAM MiB | Triangles |
|---|---:|---:|---:|---:|---:|
| lowland_forest | 792 → 720 (-9.0 %) | 594 → 556 (-6.5 %) | 1.23 → 1.33 | 227 → 244 | 1.26 → 1.27 M |
| peak_lod512 | 811 → 746 (-8.0 %) | 482 → 467 (-3.2 %) | 1.19 → 1.31 | 226 → 243 | 1.90 → 1.93 M |
| cave_torches | 2214 → 2144 (-3.2 %) | 1272 → 1230 (-3.3 %) | 0.42 → 0.44 | 227 → 244 | 1.21 → 1.22 M |

| GPU ms per pass | lowland_forest | peak_lod512 | cave_torches |
|---|---:|---:|---:|
| sky tables | 0.051 → 0.049 | 0.050 → 0.050 | 0.048 → 0.047 |
| cull 0 | 0.020 → 0.020 | 0.017 → 0.017 | 0.016 → 0.015 |
| terrain 0 | 0.467 → 0.470 | 0.088 → 0.092 | 0.160 → 0.165 |
| hi-z | 0.062 → 0.062 | 0.060 → 0.063 | 0.057 → 0.054 |
| cull 1 | 0.010 → 0.010 | 0.011 → 0.012 | 0.009 → 0.009 |
| terrain 1 | 0.010 → 0.010 | 0.010 → 0.010 | 0.010 → 0.010 |
| lod cull | 0.015 → 0.014 | 0.015 → 0.015 | 0.014 → 0.013 |
| lod terrain | 0.333 → 0.346 | 0.684 → 0.780 | 0.012 → 0.012 |
| sky, translucent, rain | 0.167 → – | 0.165 → – | 0.013 → – |
| metering | 0.023 → 0.023 | 0.022 → 0.022 | 0.018 → 0.018 |
| tonemap | 0.066 → 0.067 | 0.063 → 0.067 | 0.062 → 0.065 |
| sky | – → 0.127 | – → 0.163 | – → 0.008 |
| water's scene copy | – → 0.036 | – → 0.003 | – → 0.004 |
| translucent, rain | – → 0.091 | – → 0.011 | – → 0.013 |

Medians of three runs each; 1 % lows as the gate judges them (the 99th-percentile frame). The
water's shading at Medium (D62–D63) costs the forest and summit their 8–9 %; accepted in D65,
where the benchmark's map building moved off its frame thread (the first gate run's 1 % lows
fell 26–29 % from it) and the water's scene copy was trimmed. The candidate's GPU passes split
the old "sky, translucent, rain" into the sky, the water's scene copy and the translucent pass.

## End of V2-3 — the performance gate: ff4431d (end of V2-2) against c1bb1e3 (end of V2-3), alternating, three rounds each, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS (1st percentile) | GPU ms | Tonemap ms | Triangles |
|---|---:|---:|---:|---:|---:|
| lowland_forest | 747 → 755 (+1.0 %) | 686 → 694 (+1.2 %) | 1.30 → 1.30 | 0.067 → 0.075 | 1.27 → 1.27 M |
| peak_lod512 | 779 → 776 (-0.4 %) | 562 → 559 (-0.6 %) | 1.25 → 1.26 | 0.066 → 0.076 | 1.93 → 1.93 M |
| cave_torches | 2152 → 2172 (+1.0 %) | 1321 → 1419 (+7.4 %) | 0.44 → 0.43 | 0.066 → 0.072 | 1.22 → 1.22 M |

Medians of three runs each; 1 % lows as the gate judges them. V2-3 added no geometry to the
benchmark's scenes (no figures are drawn there; the figure pass is skipped when empty). The
tone-mapping pass gained the body's senses (D69's sounds cost nothing here; the senses' sight
costs 0.006–0.010 ms of the tone map, within the frame-rate noise). Gate passed; the baseline
moves to the end of V2-3.

## End of V2-4 — the performance gate: 984d909 (end of V2-3) against 5e20268 (end of V2-4), alternating, three rounds each, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS (1st percentile) | GPU ms | Triangles |
|---|---:|---:|---:|---:|
| lowland_forest | 712 → 731 (+2.7 %) | 556 → 553 (-0.5 %) | 1.38 → 1.34 | 1.27 → 1.27 M |
| peak_lod512 | 755 → 741 (-1.9 %) | 463 → 465 (+0.4 %) | 1.29 → 1.32 | 1.93 → 1.93 M |
| cave_torches | 2018 → 2137 (+5.9 %) | 1163 → 1260 (+8.4 %) | 0.46 → 0.44 | 1.22 → 1.22 M |

Medians of three runs each; 1 % lows as the gate judges them. V2-4 (things, carrying, clothing,
the inventory) adds nothing to the benchmark's scenes; the differences are run-to-run noise.
Gate passed; the baseline moves to the end of V2-4.


## End of V2-5 — the performance gate: 96e9bb4 (end of V2-4) against 59a1a6a (end of V2-5), alternating, the first build alternating by round, three rounds each, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS (1st percentile) | GPU ms | Triangles |
|---|---:|---:|---:|---:|
| lowland_forest | 747 → 749 (+0.2 %) | 685 → 679 (-0.9 %) | 1.29 → 1.31 | 1.27 → 1.27 M |
| peak_lod512 | 775 → 765 (-1.4 %) | 553 → 559 (+1.1 %) | 1.26 → 1.26 | 1.93 → 1.93 M |
| cave_torches | 2176 → 2222 (+2.2 %) | 1431 → 1381 (-3.5 %) | 0.43 → 0.42 | 1.22 → 1.22 M |

Medians of three runs each; 1 % lows as the gate judges them. V2-5 (making and knowing) adds
nothing to the benchmark's scenes but 56 layers to the texture array (its blocks' textures and
the flames' and embers' frames; +0.07 MiB of VRAM). Two earlier runs of the gate, with the
baseline always first in each pair, failed the cave's 1 % lows (-5.8 %, -5.9 %); an A/B of
this build with and without the new textures (2185 vs 2185 FPS, lows -0.2 %) and one with this
build first in every pair (+0.6 % average, +1.3 % lows) showed no regression, and the gate now
alternates which build runs first (D74). Gate passed; the baseline moves to the end of V2-5.
