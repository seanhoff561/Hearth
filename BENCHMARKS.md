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
## Run — commit 0ef49c9, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS | p99 ms | GPU ms | CPU ms | Draws | Triangles | VRAM MiB | Upload KiB/frame (max) | Allocs/frame (max) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| flythrough | 369.3 | 123.7 | 6.39 | 1.34 | 2.70 | 2359 | 1.60 M | 212 | 185.0 (3165) | 519.4 (2940) |

| GPU ms per pass | flythrough |
|---|---:|
| sky tables | 0.043 |
| cull 0 | 0.017 |
| terrain 0 | 0.564 |
| hi-z | 0.055 |
| cull 1 | 0.010 |
| terrain 1 | 0.010 |
| lod cull | 0.014 |
| lod terrain | 0.385 |
| sky | 0.123 |
| water's scene copy | 0.009 |
| translucent, rain | 0.022 |
| metering | 0.020 |
| tonemap | 0.066 |

| CPU ms per system | flythrough |
|---|---:|
| environment | 0.012 |
| lod selection | 1.780 |
| rain cover map | 0.001 |
| prepare: terrain | 0.422 |
| prepare: lod | 0.057 |
| prepare: sky, rain | 0.001 |
| encode | 0.033 |
| submit | 0.388 |
| wait for gpu | 0.007 |

| Allocations per frame | flythrough |
|---|---:|
| environment | 0.003 |
| lod selection | 145.583 |
| rain cover map | 0.175 |
| prepare: terrain | 42.267 |
| prepare: lod | 0.000 |
| prepare: sky, rain | 0.000 |
| encode | 76.560 |
| submit | 252.836 |
| wait for gpu | 0.001 |

| Scene | Visible cubes | LOD tiles drawn | Meshing cubes/s | LOD tiles/s | Setup s | SSIM vs golden | Slowest frame | What |
|---|---:|---:|---:|---:|---:|---:|---|---|
| flythrough | 1812 | 364 | 33006 | 0 | 70.4 | – | 12.17 ms, lod selection 9.17 | 1.2 km at sprint-fly speed (30 m/s) 70 m over the land, terrain streaming |

## End of V2-6 — the performance gate: 946304d (end of V2-5) against d4931e6 (end of V2-6), alternating, the first build alternating by round, three rounds each, NVIDIA GeForce RTX 4060 Laptop GPU (Vulkan), 1920x1080, preset Fancy

| Scene | Avg FPS | 1% low FPS (1st percentile) | GPU ms | Triangles |
|---|---:|---:|---:|---:|
| lowland_forest | 731 → 668 (-8.6 %) | 693 → 599 (-13.6 %) | 1.34 → 1.47 | 1.27 → 1.75 M |
| peak_lod512 | 763 → 725 (-5.0 %) | 543 → 490 (-9.9 %) | 1.28 → 1.35 | 1.93 → 2.03 M |
| cave_torches | 2167 → 2137 (-1.4 %) | 1078 → 1005 (-6.8 %) | 0.43 → 0.44 | 1.22 → 1.33 M |

Medians of three runs each. V2-6 grows its trees from their species (limbs as joined bars,
crowns of foliage), adds an understory and carries the vegetation into the distant tiles: in
the forest the near terrain pass went from 0.48 to 0.60 ms and the distant terrain pass from
0.34 to 0.39 ms. Three measured optimizations came first (foliage behind two layers of leaves;
a limb's faces toward foliage; a limb's end inside another), recovering about 6 %. The gate
failed and the cost is accepted as the content's (D79); the cave's lows are one run's 43 ms
stall in submit with the GPU time unchanged. The baseline moves to the end of V2-6.

## Baseline-S (Amendment S §12.1) — commit 024fcef plus S0's tools, the cloud machine (4-core Xeon at 2.1 GHz, no GPU)

Before the smooth world: what the terrain costs today, measured where it can be. This machine
renders only on a software device (llvmpipe), whose frame times say nothing (D190), so the GPU
half of Baseline-S (High and Low presets at 1440p: frame time p50, p99 and worst, GPU time per
pass, triangles and draws, VRAM) is taken on the owner's PC with `scripts/baseline-s.sh` at the
same commit and appended here; until then the last GPU numbers on record (the end of V2-6 above,
RTX 4060 Laptop GPU, 1080p, Fancy) stand. Two of the four cores were busy with long tests, so
the threaded figures are on two threads. Today's look is kept as screenshots of
`tools/shots/s0_baseline.shots`.

**The near terrain today** (`hearth bench --terrain-only`): every scene's cubes loaded and meshed
as the game does (greedy quads, models, light and AO), best of three; one surface cube re-meshed
on one thread is what an edit waits for. Memory is a cube's blocks and light; the payload is a
surface cube serialized (what the server sends after an edit and a region file keeps).

| Scene | Cubes loaded | With a surface | Cubes meshed/s | Surface cubes/s | One surface cube, 1 thread (ms) | Triangles | Mesh bytes per surface cube | Cube memory per surface cube | per cube | Surface cube serialized (raw / zstd) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 21294 | 2740 | 12752 | 1641 | 0.52 | 5582684 | 43186 | 5563 | 805 | 5483 / 551 |
| peak_lod512 | 14375 | 1633 | 12507 | 1421 | 0.38 | 2751210 | 33804 | 5332 | 689 | 5252 / 526 |
| peak_lod1024 | 14375 | 1633 | 13374 | 1519 | 0.42 | 2751210 | 33804 | 5332 | 689 | 5252 / 526 |
| coast_sunset | 64925 | 4314 | 11991 | 797 | 0.29 | 2378744 | 8867 | 2015 | 280 | 1935 / 164 |
| underwater | 57717 | 3895 | 11657 | 787 | 0.28 | 2144840 | 8540 | 2009 | 289 | 1929 / 164 |
| cave_torches | 17298 | 1616 | 13986 | 1307 | 0.28 | 1488062 | 20305 | 4449 | 496 | 4369 / 344 |
| thunderstorm | 21294 | 2740 | 13970 | 1798 | 0.43 | 5582684 | 43186 | 5563 | 805 | 5483 / 551 |
| flythrough | 183618 | 21495 | 11233 | 1315 | 0.77 | 45908062 | 43434 | 5551 | 734 | 5471 / 572 |

An edit today adds 43 bytes to `blocks.json` (generated terrain is never saved: an explored,
unedited area costs nothing) and the server re-sends the whole changed cube (the payload above).

**S0's smooth-mesher prototypes** (`bench smooth`; D222, `docs/design/smooth-terrain.md`, sheets in
`docs/review/s0/`), eight analytic scenes of 96 × 80 × 96 m, meshed whole on one thread and in
16³ cubes with a 2-voxel apron on two threads and on one; bytes at S2's compact layout (24 B a
vertex, 2 B an index):

| Method | Triangles (all scenes) | Surface cubes/s (2 threads) | per thread | Distance error mean cm | Face normal error mean ° | Non-manifold | Folded | Holes | Bytes per surface cube |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Surface Nets | 216752 | 10747 | 5764 | 2.50 | 3.0 | 21 | 28 | 0 | 9136 |
| Surface Nets, sharp features | 216752 | 6200 | 3553 | 1.45 | 2.4 | 21 | 61 | 0 | 9136 |
| Dual Contouring | 216752 | 6698 | 3136 | 1.41 | 3.0 | 21 | 542 | 0 | 9136 |

| Scene | Method | Vertices | Triangles | Whole scene ms (1 thread) | Surface cubes/s (2 threads) | per thread | Distance error mean / p99 cm | Face normal error mean / p95 ° | Vertex normal error mean ° | Non-manifold edges | Folded | Holes | Bytes per surface cube |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| rolling_hills | Surface Nets | 13876 | 27298 | 28.3 | 9611 | 5965 | 5.64 / 37.74 | 6.5 / 23.8 | 5.7 | 10 | 14 | 0 | 8624 |
| rolling_hills | Surface Nets, sharp features | 13876 | 27298 | 25.5 | 6772 | 3546 | 2.77 / 24.57 | 4.9 / 19.5 | 5.5 | 10 | 22 | 0 | 8624 |
| rolling_hills | Dual Contouring | 13876 | 27298 | 26.9 | 5817 | 3584 | 3.02 / 33.43 | 7.2 / 32.3 | 5.3 | 10 | 232 | 0 | 8624 |
| sea_cliffs | Surface Nets | 15595 | 30692 | 14.2 | 9253 | 5333 | 4.99 / 102.54 | 6.1 / 41.1 | 5.5 | 1 | 2 | 0 | 10362 |
| sea_cliffs | Surface Nets, sharp features | 15595 | 30692 | 20.2 | 4004 | 2734 | 3.37 / 74.75 | 5.5 / 37.0 | 5.3 | 1 | 19 | 0 | 10362 |
| sea_cliffs | Dual Contouring | 15595 | 30692 | 18.1 | 5796 | 2069 | 2.63 / 50.90 | 6.1 / 42.6 | 5.3 | 1 | 212 | 0 | 10362 |
| cave | Surface Nets | 15026 | 29644 | 16.2 | 10057 | 5404 | 1.85 / 12.35 | 1.5 / 5.2 | 1.1 | 4 | 4 | 0 | 10722 |
| cave | Surface Nets, sharp features | 15026 | 29644 | 26.3 | 5838 | 3434 | 0.80 / 6.75 | 1.3 / 4.8 | 1.1 | 4 | 7 | 0 | 10722 |
| cave | Dual Contouring | 15026 | 29644 | 22.3 | 5518 | 3652 | 0.91 / 9.08 | 1.7 / 6.4 | 1.1 | 4 | 17 | 0 | 10722 |
| dune_field | Surface Nets | 12976 | 25494 | 11.8 | 14329 | 7500 | 1.04 / 15.93 | 1.2 / 7.4 | 1.2 | 1 | 2 | 0 | 7256 |
| dune_field | Surface Nets, sharp features | 12976 | 25494 | 15.7 | 8521 | 4570 | 0.58 / 11.26 | 0.9 / 5.9 | 1.2 | 1 | 2 | 0 | 7256 |
| dune_field | Dual Contouring | 12976 | 25494 | 15.9 | 10007 | 4846 | 0.47 / 4.31 | 1.1 / 3.9 | 1.0 | 1 | 14 | 0 | 7256 |
| riverbank | Surface Nets | 10927 | 21448 | 10.9 | 11933 | 6163 | 1.19 / 17.61 | 1.2 / 5.4 | 1.1 | 0 | 0 | 0 | 8315 |
| riverbank | Surface Nets, sharp features | 10927 | 21448 | 14.4 | 8118 | 4100 | 0.65 / 10.49 | 1.0 / 4.4 | 1.1 | 0 | 0 | 0 | 8315 |
| riverbank | Dual Contouring | 10927 | 21448 | 14.2 | 7630 | 2837 | 0.73 / 10.12 | 1.4 / 5.6 | 1.2 | 0 | 15 | 0 | 8315 |
| talus_slope | Surface Nets | 14594 | 28728 | 14.4 | 11006 | 5412 | 2.28 / 26.81 | 3.3 / 11.8 | 2.9 | 2 | 2 | 0 | 9110 |
| talus_slope | Surface Nets, sharp features | 14594 | 28728 | 18.7 | 6363 | 3772 | 1.61 / 10.21 | 2.4 / 8.2 | 2.8 | 2 | 6 | 0 | 9110 |
| talus_slope | Dual Contouring | 14594 | 28728 | 18.2 | 6734 | 2498 | 1.64 / 8.36 | 2.6 / 8.2 | 2.7 | 2 | 24 | 0 | 9110 |
| dug_pit | Surface Nets | 9489 | 18592 | 14.4 | 9452 | 4747 | 0.39 / 7.58 | 0.4 / 0.2 | 0.4 | 0 | 0 | 0 | 9573 |
| dug_pit | Surface Nets, sharp features | 9489 | 18592 | 18.3 | 5256 | 3436 | 0.25 / 4.05 | 0.3 / 0.2 | 0.4 | 0 | 0 | 0 | 9573 |
| dug_pit | Dual Contouring | 9489 | 18592 | 13.3 | 6350 | 3563 | 0.22 / 2.14 | 0.3 / 0.3 | 0.4 | 0 | 5 | 0 | 9573 |
| mountain_ridge | Surface Nets | 17690 | 34856 | 19.6 | 11011 | 5672 | 2.67 / 15.17 | 3.4 / 9.8 | 3.1 | 3 | 4 | 0 | 9613 |
| mountain_ridge | Surface Nets, sharp features | 17690 | 34856 | 23.0 | 6565 | 3306 | 1.56 / 9.65 | 2.8 / 8.2 | 3.0 | 3 | 5 | 0 | 9613 |
| mountain_ridge | Dual Contouring | 17690 | 34856 | 35.3 | 6939 | 3445 | 1.64 / 9.44 | 3.1 / 8.6 | 2.9 | 3 | 23 | 0 | 9613 |

Shading prototype (turf and limestone on the rolling hills, height blending): biplanar mapping
takes 3.05 texture samples a pixel, triplanar 4.57; their images differ by 0.56 levels of 255 on
average, 2 % of pixels by more than 4.

## Baseline-S — near terrain on the CPU (`hearth bench --terrain-only`), commit 2fb8df5, 4 threads, preset Medium

| Scene | Cubes loaded | With a surface | Cubes meshed/s | Surface cubes/s | One surface cube, 1 thread (ms) | Triangles | Mesh bytes per surface cube | Cube memory per surface cube | per cube | Surface cube serialized (raw / zstd) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 13689 | 3019 | 7168 | 1581 | 0.43 | 5254062 | 43520 | 7684 | 1776 | 7596 / 854 |
| peak_lod512 | 13750 | 1326 | 8269 | 797 | 0.42 | 158032 | 25494 | 8498 | 930 | 8410 / 954 |
| peak_lod1024 | 13750 | 1326 | 8113 | 782 | 0.37 | 158032 | 25494 | 8498 | 930 | 8410 / 954 |
| coast_sunset | 67375 | 3990 | 8027 | 475 | 0.31 | 615388 | 21563 | 5883 | 503 | 5796 / 656 |
| underwater | 56628 | 3636 | 8302 | 533 | 0.32 | 548760 | 21240 | 5730 | 528 | 5642 / 625 |
| cave_torches | 16337 | 1629 | 8188 | 816 | 0.30 | 803898 | 33484 | 8449 | 949 | 8361 / 1024 |
| thunderstorm | 13689 | 3019 | 7058 | 1557 | 0.40 | 5254062 | 43520 | 7684 | 1776 | 7596 / 854 |
| flythrough | 91809 | 23150 | 6056 | 1527 | 0.87 | 44474278 | 45888 | 7479 | 1978 | 7391 / 836 |

An edit today: the dug block adds 78 bytes to `blocks.json` (generated terrain is never saved: an explored, unedited area costs nothing), and the server sends the whole changed cube to the client (the serialized size above).


## S2 smooth ground — near terrain on the CPU (`hearth bench --terrain-only`), commit 2fb8df5, 4 threads, preset Medium

| Scene | Cubes loaded | With a surface | Cubes meshed/s | Surface cubes/s | One surface cube, 1 thread (ms) | Triangles | Mesh bytes per surface cube | Cube memory per surface cube | per cube | Surface cube serialized (raw / zstd) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 13689 | 3019 | 7255 | 1600 | 0.48 | 6075792 | 43520 | 7684 | 1776 | 7596 / 854 |
| peak_lod512 | 13750 | 1326 | 8207 | 791 | 0.37 | 879000 | 25494 | 8498 | 930 | 8410 / 954 |
| peak_lod1024 | 13750 | 1326 | 7924 | 764 | 0.43 | 879000 | 25494 | 8498 | 930 | 8410 / 954 |
| coast_sunset | 67375 | 3990 | 8379 | 496 | 0.28 | 2347178 | 21563 | 5883 | 503 | 5796 / 656 |
| underwater | 56628 | 3636 | 8093 | 520 | 0.30 | 2115948 | 21240 | 5730 | 528 | 5642 / 625 |
| cave_torches | 16337 | 1629 | 8180 | 816 | 0.32 | 1596736 | 33484 | 8449 | 949 | 8361 / 1024 |
| thunderstorm | 13689 | 3019 | 7120 | 1570 | 0.39 | 6075792 | 43520 | 7684 | 1776 | 7596 / 854 |
| flythrough | 91809 | 23150 | 5790 | 1460 | 0.83 | 50050736 | 45888 | 7479 | 1978 | 7391 / 836 |

An edit today: the dug block adds 78 bytes to `blocks.json` (generated terrain is never saved: an explored, unedited area costs nothing), and the server sends the whole changed cube to the client (the serialized size above).


## S2 smooth ground — near terrain on the CPU (`hearth bench --terrain-only`), 4 threads, preset Medium

| Scene | Cubes loaded | With a surface | Cubes meshed/s | Surface cubes/s | One surface cube, 1 thread (ms) | Triangles | Mesh bytes per surface cube | Cube memory per surface cube | per cube | Surface cube serialized (raw / zstd) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| lowland_forest | 13689 | 3019 | 7255 | 1600 | 0.48 | 6075792 | 43520 | 7684 | 1776 | 7596 / 854 |
| peak_lod512 | 13750 | 1326 | 8207 | 791 | 0.37 | 879000 | 25494 | 8498 | 930 | 8410 / 954 |
| peak_lod1024 | 13750 | 1326 | 7924 | 764 | 0.43 | 879000 | 25494 | 8498 | 930 | 8410 / 954 |
| coast_sunset | 67375 | 3990 | 8379 | 496 | 0.28 | 2347178 | 21563 | 5883 | 503 | 5796 / 656 |
| underwater | 56628 | 3636 | 8093 | 520 | 0.30 | 2115948 | 21240 | 5730 | 528 | 5642 / 625 |
| cave_torches | 16337 | 1629 | 8180 | 816 | 0.32 | 1596736 | 33484 | 8449 | 949 | 8361 / 1024 |
| thunderstorm | 13689 | 3019 | 7120 | 1570 | 0.39 | 6075792 | 43520 | 7684 | 1776 | 7596 / 854 |
| flythrough | 91809 | 23150 | 5790 | 1460 | 0.83 | 50050736 | 45888 | 7479 | 1978 | 7391 / 836 |

An edit today: the dug block adds 78 bytes to `blocks.json` (generated terrain is never saved: an explored, unedited area costs nothing), and the server sends the whole changed cube to the client (the serialized size above).

Against Baseline-S above (whose rows were taken on two threads, these on four; the places moved with E4, so compare a scene's own columns): one surface cube costs as before on one thread (0.28–0.87 ms); cube memory per surface cube grows by the fill array where the surface passes (+4 KiB, 5.5 → 7.7 KiB in the forest); mesh bytes per surface cube fall in the mountains (33.8 → 25.5 KiB) and grow on coasts (8.9 → 21.6 KiB), where smooth slopes replace few large greedy quads. Triangles count the smooth ground's with the quads'.


## E4.1 before the fixes — the Earth-sized planet (`hearth bench globe|creator|load`), 4 cores, 16 GB, software adapter (llvmpipe)

The cloud machine (4 cores, 16 GB, no GPU) is the lower configuration of E4.1 §6. Seed 7's planet, cached.

**Globe** (`bench globe --seed 7`): the map as `globe::planet_map` makes it, 32.2 s a row of
2,048 texels on all four cores, the whole map projected at 33,000 s (each texel ~2.7 tiles of the
finest refinement level, 1.7 of the next, 0.2 of the coarsest built; 25.6, 18 and 16 ms a tile);
hover (`describe`) 154 ms median, 319 ms 95th percentile, 427 ms slowest; a click's start spot
(`spawn_near`) 314 ms median, its card (`Finder::verify`) 204 ms median and 940 ms slowest; the
places suggested 5.0 s; 667 MiB resident at the peak.

**Creator** (`bench creator --software`): a person's meshes 1,605 ms at Close, 529 ms Near,
251 ms Far; a slider dragged a second (60 changes): the person shown 59 frames behind at the end,
the last setting shown 2.3–2.7 s after (height, build, skin tone and hair colour alike); the main
thread's own work under 0.2 ms a frame (the rest of the 37–47 ms frames is llvmpipe drawing).

**Load** (`bench load --seed 7 --software --size 640x360`, render distance 12, LOD 256):

| | Play → world shown | → in control | → whole render distance | peak resident | main thread frame (median / 95th / slowest) |
|---|---:|---:|---:|---:|---:|
| new world, after the menus (the globe's map still being made) | never (300 s limit) | never | never | — | 0.19 / 0.51 / 20.8 ms |
| new world, alone | 2.8 s | 2.8 s | 46.2 s | 1,409 MiB | 0.21 / 780 / 1,003 ms |
| its save | 32.8 s | 32.8 s | 63.6 s | 1,530 MiB | 0.24 / 0.37 / 691 ms |

After the menus, the server's thread had 0.55 s of CPU in 300 s and the globe map's workers
1,174 s: the world's setup waits in the global pool behind the map. Opening the save, `Fauna::new`
took 30.5 s (458 tiles of the finest level). The main thread's slow frames are llvmpipe's drawing
(`queue.submit`, 63 s over 2,160 frames) and the frame the world arrives in (the scene's pipelines
made, 0.9 s); its own work (pump, update, render) took 9.1 s in all.

## E4.1 step 5 — loading and memory (`hearth bench load`), 4 cores, 16 GB, software adapter (llvmpipe)

The same machine and planet as "E4.1 before the fixes"; commit beb5aba. A new world with the
menus' work still running behind it (the globe's map, the places), then its save; the
refinement tiles' disk store emptied first, so the new world makes its own and its save reads
them back.

| | Play → world shown | → in control | → whole render distance | peak resident | main thread a frame (median / 95th) |
|---|---:|---:|---:|---:|---:|
| new world, after the menus | 2.5 s (never) | 3.2 s (never) | 35.8 s (never; 46.2 s alone) | 1,346 MiB (1,409) | 0.25 / 678 ms |
| its save | 2.2 s (32.8) | 2.2 s (32.8) | 33.6 s (63.6) | 1,557 MiB (1,530) | 0.46 / 656 ms |

The animals of a save 0.18 s (30.5 s: their regions are made again as the player comes near);
79 refinement tiles read from disk, 4 ms at most. The slow frames are llvmpipe's drawing
(`queue.submit` 56.7 s over 329 frames); the main thread's own work stays under a millisecond.

**Memory by kind** (F3's line, the bench's report): the new world after loading — the planet
grid 108 MiB, the near terrain's GPU buffers 96, the animals 74, the distant terrain's 56,
the rest of the process 972 MiB. The caches' budgets on this machine (40 % of 16.9 GB): the
refinement tiles 516 MiB, the generated columns 322, the animals set aside 129, the samples'
neighbourhoods 128.

**The soak** (`bench load --no-menus --fly 1800 --speed 60 --size 320x180`, 30 minutes across
108 km, about 15 times sprinting): before step 5, 1,331 → 2,501 MiB resident, growing
throughout (each column's record outlived its cubes, about 2 KB a column, on the server and the
client alike; the animals' regions, 8.3 MB each, were never let go); after, 1,452 → 2,580 MiB:
the caches and the GPU's buffers fill to their budgets in the first eight minutes, then 2,092 →
2,221 MiB from minute 8 to 26, and 2,580 at the end. The last minutes' rise is in what no cache
accounts for ("the rest of the process"); the next soak tells the heap in use from the heap
freed but kept.

## E4.1 after — the Earth-sized planet (`hearth bench globe|creator|load`), 4 cores, 16 GB, software adapter (llvmpipe)

The same machine and planet as "E4.1 before the fixes"; the end of E4.1. The lower configuration
of E4.1 §6: the reference machine's numbers (8 cores and a GPU) are the owner's to run
(`hearth bench load`, `hearth bench globe`). The load as before (the menus' work behind a new
world, 640x360, the refinement tiles made afresh and its save reading them back), the median of
three runs. The planet of both was the one an older build had cached here on 8 October, not the
one this build makes of seed 7 (D289): the timings stand for that planet; this build's planet
and the owner's laptop are in the next section.

| | before | after |
|---|---:|---:|
| The globe's map | never (some 9 hours) | 1.09 s, read back in 0.01 s |
| Hovering over the globe, median / slowest | 154 / 427 ms | 0.004 / 0.055 ms |
| A click to its place's details, median / slowest | 518 ms (on the interface's thread) | 481 / 1,167 ms (on a thread) |
| The places suggested | 5.0 s | 5.5 s |
| The creator, a person at Close | 1,605 ms | 711 ms |
| The creator, the last setting shown after a slider's drag | 2.3–2.7 s | colours the next frame; shapes 140–148 ms |
| The creator, at full detail after a drag | 2.3–2.7 s | 0.9–1.2 s |
| A new world, Play → in control | never (the menus' work behind it) | 3.35 s |
| A new world, Play → the whole render distance | never (46.2 s alone) | 37.1 s |
| Its save, Play → in control | 32.8 s | 2.34 s |
| Its save, Play → the whole render distance | 63.6 s | 34.6 s |
| Peak resident, the new world / its save | 1,409 / 1,530 MiB | 1,393 / 1,575 MiB |
| The main thread's own work a frame while loading, median | 0.21 ms | 0.35–0.46 ms |
| Meshes a batch of cubes made (the client may hold 512 unread) | — | 142 |

The three runs' whole render distance came at 38.3, 37.1 and 36.8 s (step 5: 35.8 s, one run);
the slow frames are llvmpipe's drawing (`frame.submit`, 60 s over 332 frames). Each cube's mesh
went to the client once: 7,935 meshes, one for each of the 23 × 23 × 15 cubes meshed (a cube is
meshed once its neighbours are loaded, so not the outermost shell of the 25 × 25 × 17 loaded);
its remeshes as the neighbours came replaced it in the outbox while the client was behind. The creator builds on the interactive pool, three threads here with a core kept
free (step 4's 0.8 s to full detail had all four).

**The soak** (`bench load --no-menus --fly 1800 --speed 60 --size 320x180`, 30 minutes across
108 km): 1,310 MiB after the first minute; from the third minute to the 24th 1,577–1,788 MiB,
the heap in use 1.0–1.1 GB throughout (glibc keeping some 0.5 GB of it freed); at the 25th the
near terrain's general-quad arena doubled for denser country (its GPU buffers 96 → 160 MiB, held
in memory on llvmpipe), then 1,869–1,900 MiB to the end; 2,092 MiB at the most (a save). The
channel held its 512 meshes unread throughout (llvmpipe at 320x180 is the slow side) and the
server's outbox 8–24 MiB. Before step 5 the flight rose 1,331 → 2,501 MiB, after it 1,452 →
2,580 MiB, rising throughout; with the outbox alone (before the in-view order, the fades and the
interactive pool's third thread) 1,253 → 1,851 MiB, the same plateau. Its save, opened 108 km
from the spawn, was in control at 2.95 s and whole at 70 s (the distant tiles there not on disk).

## E4.1 on the reference machine — the owner's laptop, and this build's planet here

The owner's laptop: Ryzen 7 7840HS (8 cores, 16 threads), RTX 4060 Laptop on Vulkan, 16 GB,
Windows, `cargo run --release`, commit 86e24ce, one run of each at the defaults (1280x720, render
distance 12, LOD 256; seed 7, its planet made afresh). Here: the cloud machine of the sections
above, `--software --size 640x360`, this build's planet of seed 7 made afresh (D289), one run.

| | the owner's laptop | here | budget |
|---|---:|---:|---:|
| The globe's map, made / read back | 0.26 / 0.014 s | 1.12 / 0.01 s | |
| The map's land by area | 27.258 % | 27.251 % | |
| Hovering over the globe, median / slowest | 0.002 / 0.024 ms | 0.005 / 0.50 ms | |
| A click to its place's details, median / slowest | 489 / 825 ms | 897 / 1,439 ms | |
| The places suggested | 4.1 s | 7.4 s | |
| The creator, a person at Close | 137 ms | 711 ms (E4.1 after) | |
| The creator, the last setting shown after a drag (height, build) | 75 / 70 ms | 140–148 ms (E4.1 after) | |
| The creator, at full detail after a drag (height, build) | 244 / 224 ms | 0.9–1.2 s (E4.1 after) | 1 s |
| A new world, Play → in control | 2.28 s | 2.99 s | |
| A new world, Play → the whole render distance | 4.84 s | 20.3 s | ~30 s |
| Its save, Play → in control | 1.08 s | 2.28 s | |
| Its save, Play → the whole render distance | 3.29 s | 14.7 s | |
| Peak resident, the new world / its save | 617 / 591 MiB | 1,101 / 1,174 MiB | |
| The main thread a frame while loading, median, new world / save | 1.48 / 1.15 ms | 0.26 / 0.26 ms | |
| The main thread's slowest frame, new world / save | 1,356 / 183 ms | 1,188 / 655 ms | |

Both budgets the cloud machine missed are met on the reference machine: the whole render
distance in 4.8 s, the creator's full detail in 0.22–0.24 s. On this build's planet the cloud
machine reaches the whole render distance in 20 s too (37 s on the older planet, its spawn on
other ground). The laptop's slowest frame is the scene's pipelines compiled when the world
arrives, the first time (PLAYTEST 33): 183 ms the next, from the driver's shader cache. Its
memory is half the cloud machine's (llvmpipe holds the GPU's buffers in memory). Both runs built
finest-level tiles for the distant terrain about the player (`lod.near` and `lod.mid`: 24 and 29
there, 17 and 22 here; none on the older planet), the level it samples at there, and none for a
coarse caller (the gate's `relief.fine.*`). The map's land differs by 0.007 points, some 150
texels, between Windows and Linux (D289).

## T1.4 — the globe in relief (`hearth bench globe --seed 7`), 4 cores, 16 GB, software adapter (llvmpipe)

This build's planet of seed 7 (D289), the same machine as E4.1's. The map gains its heights at
4096 × 2048 (D294). The first column is E4.1's end on the same planet (the owner's-laptop
section above); the second is the map of everything at 4096 wide, made and kept as before; the
third is T1.4's end.

| | E4.1 | 4096 wide, all layers | T1.4 |
|---|---:|---:|---:|
| The globe's map, made (and kept) | 1.12 s | 4.3 s | 1.22–1.28 s |
| Read back from where it is kept | 0.01 s | — | 0.025–0.032 s |
| Kept, packed | — | — | 10.3 MB |
| Hovering, median / slowest | 0.005 / 0.50 ms | | 0.006 / 0.05 ms |
| A click to its place's details, median | 0.90 s | | 0.92 s |

Within the map's time: its colours and facts at 2048 × 1024 (`grid_column`) 1.05–1.1 s as
before; the heights 0.14–0.15 s, from 0.37–0.46 s sampling each texel's centre alone (a tangent,
an inverse sine and a wrap each), once each column's bicubic weights were found once for the
whole map (`Field::bicubic_rows`, the same sums); the keeping off the map's thread. Read back:
read 7–10 ms; unpacked as one part 42–61 ms and copied out 22–25 ms (0.07–0.106 s, past the
budget at times); as four parts in parallel into the map's own buffers, 0.025–0.032 s in all.

## R1a — the sun's shadows (`hearth bench --scenes lowland_forest --size 960x540 --software`), 4 cores, software adapter (llvmpipe)

The same build (commit 20a43ac) with `--shadows off` and the default (medium); 150 frames after
40. On this device a frame is the GPU's (the submit), so the frame rates say nothing of the PC's;
the GPU's share at 1080p on the RTX 4060 is the owner's check (budget 1.5 ms).

| | Shadows off | Medium |
|---|---:|---:|
| Frame p50 (ms) | 846 | 1,047 |
| GPU: the shadow passes (ms) | 0.06 | 249 |
| GPU: terrain 0 · distant terrain (ms) | 529 · 282 | 561 · 281 |
| CPU: prepare: terrain (ms) | 1.02 | 2.52 |
| Draws · triangles | 2,492 · 0.81 M | 5,316 · 1.54 M |
| Allocations a frame (prepare: terrain · submit) | 53 · 273 | 113 · 390 |

The frame thread's 1.5 ms is the near cascades' caster lists (the cubes looked up in each
cascade's region, D298) and their uploads; the allocations are wgpu's staging for the 25 more
draw lists and 5 globals written a frame.
