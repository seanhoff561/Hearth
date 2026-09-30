# Rendering performance audit

*September 2026, at commit 038759c (distant trees in). Machine: RTX 4060 Laptop GPU, Vulkan,
1920×1080, default preset (Fancy: render distance 12, LOD 256). Numbers from `hearth bench`
(`BENCHMARKS.md`); every change the audit makes lands as its own commit with before/after
numbers and an SSIM check against the golden images.*

## Method
`hearth bench` renders seven scenes on fixed camera paths offscreen with the default preset:
a lowland forest, a volcano summit at LOD 512 and 1024, a coast at sunset, underwater, a cave
lit by torches and a thunderstorm. Frames run as a real frame loop with two frames in flight
and no presentation; the camera advances a fixed step per frame so every run renders the same
frames. It reports average FPS, 1 % lows, the 99th-percentile frame time, GPU time per pass
(timestamps between passes, read back without stalling), CPU time and heap allocations per
system on the render thread, draws, triangles, video memory (the allocator's report), bytes
uploaded per frame, meshing and LOD build throughput, and what the slowest frames were spent
on. `--golden`/`--compare` save and compare reference images (SSIM plus a diff image).
Presentation, the compositor and vsync are outside the measurement.

## Where the time goes
At about 1 ms per frame the renderer is limited by the CPU as much as the GPU:

| | forest | summit | coast | underwater | cave | storm |
|---|---:|---:|---:|---:|---:|---:|
| Average FPS | 873 | 918 | 1015 | 1034 | 1228 | 781 |
| GPU ms | 1.12 | 1.00 | 0.71 | 0.62 | 0.79 | 1.24 |
| CPU ms (render thread, excl. waiting) | 0.80 | 0.98 | 0.98 | 0.96 | 0.56 | 0.97 |

- **CPU:** environment sampling 0.23–0.37 ms (the sky's irradiance is integrated over the
  hemisphere on the CPU every frame), queue submission 0.24–0.35 ms with spikes of 0.6–2.6 ms,
  terrain preparation 0.02–0.34 ms (the visibility search, sorting, translucent re-sorting).
  The slowest frames are CPU frames: environment + terrain preparation + submission.
- **GPU:** full-detail terrain 0.08–0.41 ms (phase 0), distant (LOD) terrain 0.09–0.55 ms — it
  is drawn one tile at a time (380–450 draws) and never occlusion-culled, so it still costs
  0.40 ms inside a cave where none of it can be seen; sky, water and rain 0.02–0.24 ms; the
  sky tables, Hi-Z pyramid, metering and tonemapping about 0.2 ms together.
- **Memory:** 140–253 MiB of video memory, of which LOD tiles 115–154 MiB (64 bytes a quad).
- **Allocations:** about 390 heap allocations per frame on the render thread: ~73 in our code
  (terrain preparation 70) and ~320 inside wgpu's command encoding (91) and submission (231).

## Audit
Status: **done** (in place, evidence given), **missing** (planned below), **not worth it**
(with the reason), **n/a** (the effect itself doesn't exist yet; the requirement is recorded in
`docs/design/rendering.md` for when it is built).

### Terrain
| Optimization | Status | Evidence / impact |
|---|---|---|
| Greedy meshing | done (classic) | Uniform faces merge into one quad per run. A bitmask ("binary") greedy mesher is **not worth it**: meshing runs at 87–101 k cubes/s on all threads, while streaming is bound by generation and lighting; it does not touch frame time. |
| Packed vertex data, vertex pulling | done | 16-byte `PackedQuad` per full-face quad, four vertices generated in the vertex shader from a storage buffer; 64-byte general quads for models, fluids and translucent faces. |
| One shared index buffer | done | One 16,384-quad index buffer for all terrain draws (the LOD has its own). |
| Per-face-direction buckets | done | Quads grouped by layer × 6 directions; groups facing away are skipped (CPU path, and `facing` in `cull.wgsl`). |
| Pooled mesh buffers, sub-allocator | done | Two storage arenas growing by doubling, first-fit free list with coalescing (`RangeAllocator`). LOD tiles are not pooled (one buffer per tile): see LOD. |
| Upload budget per frame | done | 256 cube meshes and 24 LOD tiles per frame while streaming; steady state 9–74 KiB uploaded per frame (max 319 KiB). A byte budget instead of a count is **not worth it** at these volumes. |
| Cave / visibility-graph culling | done | Face-connectivity bitset per cube, breadth-first search from the camera's cube: 10 cubes visible in the cave out of 17 k loaded. |

### GPU-driven culling
| Optimization | Status | Evidence / impact |
|---|---|---|
| Frustum culling | done (CPU) | Candidates are frustum-tested during the visibility search, which must run on the CPU anyway; moving the test to a compute pass is **not worth it**. |
| Two-phase Hi-Z occlusion culling | done for terrain; **missing for LOD** | Phase 0 draws last frame's visible cubes, the Hi-Z pyramid is built, phase 1 re-tests everything (no popping; `verify_cull` checks it is pixel-identical to CPU culling). The LOD is not culled at all (0.40 ms in the cave). |
| Indirect multi-draw with count | done for terrain; **missing for LOD** | Eight `multi_draw_indexed_indirect_count` calls on Vulkan (CPU multi-draw elsewhere). The LOD issues one draw per tile: 380–450 per frame. |
| Bindless-style texture arrays | done | All block textures in one mipmapped 2D texture array. |
| Few pipeline / bind-group switches | done for terrain; LOD: see pooling | Terrain: five pipelines, two bind groups. LOD: one pipeline, but a vertex buffer bound per tile. |
| Render bundles | not worth it | Command encoding costs 0.04 ms per frame; the draws are GPU-generated. |
| No per-frame bind groups or buffers | done (result 2) | The Hi-Z level-0 bind group was created every frame; now kept until the depth target changes. Everything else is created at startup or when a buffer grows. |

### LOD
| Optimization | Status | Evidence / impact |
|---|---|---|
| Level choice by screen-space error, with hysteresis | **missing** | Tiles split within four tile sizes of the camera (columns up to ~7 px wide at 1080p) and are re-selected every 16 blocks without per-tile hysteresis; flat and rough land get the same detail. |
| Quantized tile-relative vertex data | **missing (partly)** | X and Z are tile-relative u16, but Y is an absolute i32 and every quad has four 16-byte vertices (64 B/quad): 115–154 MiB of LOD. One 16-byte record per quad with vertex pulling would take a quarter. |
| LOD in the GPU culling path, occluded by near terrain | **missing** | See culling. |
| Batched SIMD surface sampling | not worth it (now) | Tiles build at ~1.8 k tiles/s on all threads (a whole view from scratch in ~0.8 s; streaming needs tens per re-selection); it is off the frame path. Revisit with the disk cache. |
| Stale jobs cancelled | done | Queued builds are dropped at every re-selection; the ≤ 48 in flight finish and are discarded. |
| Priority by screen importance | **missing (partly)** | Nearest first, whether in view or behind the camera. |
| Disk cache reused | missing — V2-6 | Planned with edits reflected in the LOD (V2-6); build throughput above makes it a startup and revisit gain, not a frame-rate one. |
| Distant forests as canopy geometry | done | D56: the generator's own trees on the fine levels, estimated canopy on the coarse ones. |
| Seamless handoff, no double geometry | done | D54: the cubes dither out over 8 blocks with the LOD just behind; LOD fragments inside the full-detail area are discarded (tiles straddling its edge still run their vertices). |
| No holes while tiles stream | **missing** | Tiles leaving the selection are removed at once, before the tiles replacing them are built, so a moving camera can open gaps for a few frames. |

### Shaders and effects
| Optimization | Status | Evidence / impact |
|---|---|---|
| Volumetric fog, clouds, SSR at reduced resolution with depth-aware upsampling and temporal accumulation | n/a | None of these effects exists yet (clouds are one textured layer in the sky pass). |
| Atmosphere via LUTs | done | Transmittance and multiple scattering once (re-made when the haze changes by 5 %), a 256×128 sky view per frame (0.045 ms). Aerial perspective is integrated in closed form per fragment and looks up the sky view — no froxel volume is needed. |
| Shadow cascades (tight, texel-snapped, far cascades cached round-robin on LOD geometry) | n/a | No shadow maps yet (V2-16). |
| Water normals from tiling textures | n/a | Water shading comes with V2-2e. |
| Hi-Z ray-marched SSR with sky fallback | n/a | No SSR. |
| Dual-filter bloom | n/a | No bloom yet (the option exists). |
| One-pass histogram auto-exposure | done | `meter.wgsl`: one 256-thread dispatch, a workgroup histogram of 64×36 samples (0.02 ms). |
| Tonemap, grading, vignette, dithering in one final pass | done except **dithering (missing)** | One pass for exposure, the night shift and ACES (0.03–0.04 ms); no dithering, so sky gradients can band at 8 bits. |
| Half precision where supported | not worth it | The shaders are light on arithmetic and the passes are raster- and bandwidth-bound; world-space math needs full precision. |
| Specialization constants instead of runtime branches | not worth it | The branches are on uniforms (coherent); no measurable cost. |
| Pipelines precompiled at startup | done | Every pipeline is built with the renderer; none is created during frames. A persistent pipeline cache would only shorten startup. |

### Foliage and instancing
| Optimization | Status | Evidence / impact |
|---|---|---|
| Cheap alpha-test path | done | Separate cutout pipelines (discard), opaque ones without. |
| Instanced grass, particles, ambient life with compute culling | done for rain and snow; n/a for the rest | Rain and snow are generated on the GPU from the vertex index, their count scaled by intensity, hidden under cover by a height map. Grass is meshed with its cube (and culled with it); animals come in V2-7. |
| Animation LOD | n/a | No animated entities yet. |

### Frame pacing
| Optimization | Status | Evidence / impact |
|---|---|---|
| Two to three frames in flight | done | Swapchain frame latency 2. |
| No CPU–GPU stalls on the frame path | done | Draw counters and pass timings are read back asynchronously; the only waits are the frame latency. |
| Accurate frame limiter | done | Sleep, then spin for the last 1.5 ms. |
| Zero steady-state allocations | **missing** | ~73 allocations per frame in our code, ~320 inside wgpu. |

### Upscaling
| Optimization | Status | Evidence / impact |
|---|---|---|
| Render scale with a quality upscaler, optional | **missing** | The `render_scale` option exists but is not applied. A temporal upscaler needs motion vectors and TAA (V2-6). |

### Found by measuring
| Optimization | Status | Evidence / impact |
|---|---|---|
| Environment sampling | done (result 1) | Was 0.23–0.37 ms per frame, the sky's irradiance integrated on the CPU; now interpolated between cached nodes (< 0.01 ms). |
| Fewer queue writes per frame | **missing** | ~15 separate `write_buffer` calls a frame; submission costs 0.24–0.35 ms with spikes. |

## Plan
Missing and worth doing, one commit each with before/after numbers and an SSIM check:

1. Environment sampling: memoize the sky light while its inputs stay put.
2. Zero allocations in our frame code; the Hi-Z bind group cached.
3. LOD tiles in one pooled buffer as 16-byte quad records (vertex pulling), drawn indirectly.
4. LOD culled on the GPU: frustum and two-phase Hi-Z against the near terrain.
5. LOD streaming: keep replaced tiles until their replacements are ready; build what is in view
   first.
6. Dithering in the final pass.
7. Screen-space-error LOD selection with hysteresis.
8. Render scale with a spatial upscaler (FSR 1, MIT) as an option, off by default.
9. A performance gate at the end of every milestone (`scripts/perf-gate.sh`).

Results are recorded below as they land.

## Results
1. **Sky light cache** — the sky's irradiance (an integral over the sky, 0.13 ms per light,
   plus 0.07 ms re-blending the atmosphere's tables) is computed at nodes of light elevation,
   altitude and haze level and interpolated in log space (`SkyLightCache`; within 2.5 % of the
   exact integral, twilight included). Environment sampling 0.23–0.37 → 0.005–0.009 ms; CPU per
   frame 0.56–0.98 → 0.34–0.68 ms; average FPS coast 1015 → 1452, underwater 1034 → 1492, storm
   781 → 839, cave 1228 → 1273, forest 873 → 858 (GPU-bound at 1.14 ms); SSIM ≥ 0.9995 in
   every scene.
2. **No per-frame allocations in our frame code; Hi-Z bind group cached** — the visibility
   search keeps its set, queue and list between frames, sorts without scratch memory (the
   search order is deterministic), the LOD reuses its origins list, and the Hi-Z level-0 bind
   group is rebuilt only when the depth target changes. Heap allocations per frame 356–399 →
   330–356 (terrain preparation 70 → 43, the rest being wgpu's staging for each queue write;
   environment 2 → 0; encoding 91 → 77); the only per-frame bind group is gone. Frame times
   unchanged within run-to-run noise (±5 %); rendering identical (same SSIM).
