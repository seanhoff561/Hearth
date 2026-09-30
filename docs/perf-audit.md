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
| Pooled mesh buffers, sub-allocator | done | Two storage arenas growing by doubling, first-fit free list with coalescing (`RangeAllocator`); LOD quads in a third (result 3). |
| Upload budget per frame | done | 256 cube meshes and 24 LOD tiles per frame while streaming; steady state 9–74 KiB uploaded per frame (max 319 KiB). A byte budget instead of a count is **not worth it** at these volumes. |
| Cave / visibility-graph culling | done | Face-connectivity bitset per cube, breadth-first search from the camera's cube: 10 cubes visible in the cave out of 17 k loaded. |

### GPU-driven culling
| Optimization | Status | Evidence / impact |
|---|---|---|
| Frustum culling | done (CPU) | Candidates are frustum-tested during the visibility search, which must run on the CPU anyway; moving the test to a compute pass is **not worth it**. |
| Two-phase Hi-Z occlusion culling | done | Terrain: phase 0 draws last frame's visible cubes, the Hi-Z pyramid is built, phase 1 re-tests everything (no popping; `verify_cull` checks it is pixel-identical to CPU culling). LOD: tested against the same pyramid in the same frame (result 4; cave LOD pass 0.40 → 0.014 ms). |
| Indirect multi-draw with count | done | Terrain: eight `multi_draw_indexed_indirect_count` calls on Vulkan (CPU multi-draw elsewhere). LOD: one `multi_draw_indexed_indirect_count` after the GPU cull (results 3, 4). |
| Bindless-style texture arrays | done | All block textures in one mipmapped 2D texture array. |
| Few pipeline / bind-group switches | done | Terrain: five pipelines, two bind groups; LOD: one pipeline, two bind groups, one draw call (result 3). |
| Render bundles | not worth it | Command encoding costs 0.04 ms per frame; the draws are GPU-generated. |
| No per-frame bind groups or buffers | done (result 2) | The Hi-Z level-0 bind group was created every frame; now kept until the depth target changes. Everything else is created at startup or when a buffer grows. |

### LOD
| Optimization | Status | Evidence / impact |
|---|---|---|
| Level choice by screen-space error, with hysteresis | done (result 7) | The distance rule (columns 3–6 px wide) stays as the floor — coarser tiles would blur colours and trees — and rough tiles are split until the steps between their columns stray at most 2 px on screen (1 px on Fabulous) from what finer columns would show; hysteresis on both rules, neighbours kept within a level. It adds detail, so it costs frame time: −27 % average FPS on the summit, −6 to −10 % elsewhere. |
| Quads grouped by facing, groups facing away skipped | done (result 8) | As the full-detail terrain's buckets: each tile's quads in six groups by geometric facing; only runs of groups that can face the camera are drawn (the pipeline draws both sides, so back faces cost vertices and fragments). LOD pass −40 %. |
| Quantized tile-relative vertex data | done (result 3) | One 16-byte record per quad (was four 16-byte vertices), expanded by the vertex shader: 73–91 MiB less video memory in LOD-heavy scenes. |
| LOD in the GPU culling path, occluded by near terrain | done (result 4) | See culling. |
| Batched SIMD surface sampling | not worth it (now) | Tiles build at ~1.8 k tiles/s on all threads (a whole view from scratch in ~0.8 s; streaming needs tens per re-selection); it is off the frame path. Revisit with the disk cache. |
| Stale jobs cancelled | done | Queued builds are dropped at every re-selection; the ≤ 48 in flight finish and are discarded. |
| Priority by screen importance | done (result 5) | Nearest first, tiles behind the camera weighted as four times farther. |
| Disk cache reused | missing — V2-6 | Planned with edits reflected in the LOD (V2-6); build throughput above makes it a startup and revisit gain, not a frame-rate one. |
| Distant forests as canopy geometry | done | D56: the generator's own trees on the fine levels, estimated canopy on the coarse ones. |
| Seamless handoff, no double geometry | done | D54: the cubes dither out over 8 blocks with the LOD just behind; LOD fragments inside the full-detail area are discarded (tiles straddling its edge still run their vertices). |
| No holes while tiles stream | done (result 5) | Replaced tiles stay drawn until their replacements are built (`hearth_lod::cover`), with no ground drawn twice. |

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
| Tonemap, grading, vignette, dithering in one final pass | done (result 6) | One pass for exposure, the night shift, ACES and dithering in sRGB space (0.055 ms); no grading or vignette yet. |
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
| Fewer queue writes per frame | not worth it | Tried a staging belt for the per-frame uploads: no change in average FPS in alternating A/B runs, 1 % lows slightly lower; reverted. |

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
8. LOD quads grouped by face direction, the groups facing away from the camera skipped (as
   the full-detail terrain does) — to win back some of 7's cost.
9. Render scale with a spatial upscaler (FSR 1, MIT) as an option, off by default.
10. A performance gate at the end of every milestone (`scripts/perf-gate.sh`).

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
   *Tried and reverted:* routing the per-frame uploads through a staging belt instead of
   `queue.write_buffer` (which makes a staging buffer per call). Alternating A/B runs (three
   rounds, two scenes) showed no change in average FPS (862 vs 863, 1386 vs 1392) and 1 % lows
   lower in five of six pairs (708 → 664 on average), so the change was not kept (queue writes
   per frame: not worth it).
3. **LOD quads pooled as 16-byte records, one indirect multi-draw** — every tile's quads
   live in one sub-allocated storage buffer as packed records (tile-relative corner, extents,
   face, colour, tint, climate) that the vertex shader expands (vertex pulling); the visible
   tiles go out in a single `multi_draw_indexed_indirect` (one draw per tile without
   first-instance support). Video memory: forest 236 → 163 MiB, summit 253 → 162 MiB, cave
   235 → 163 MiB; render-thread CPU 0.40–0.68 → 0.24–0.49 ms per frame; draw calls for the
   LOD ~380–450 → 1. Average FPS summit 911 → 963, underwater 1478 → 1558, cave 1213 → 1309,
   forest and storm unchanged (GPU-bound). SSIM ≥ 0.9985 (the lowest, underwater: grazing
   fragments of distant water seen from below, which the water shading of V2-2e replaces).
4. **LOD occlusion-culled on the GPU against the near terrain's Hi-Z** — the tiles inside
   the frustum go to a compute pass (`lod_cull.wgsl`) that tests their bounds (curvature
   included) against the Hi-Z pyramid the terrain builds from this frame's depth after its
   first phase; the survivors are drawn with one `multi_draw_indexed_indirect_count`. Cost
   0.013 ms. Alternating A/B runs: cave 1253 → 2463 FPS average (GPU 0.78 → 0.38 ms; the LOD
   pass 0.40 → 0.014 ms), underwater, storm and the open views unchanged within noise (their
   distant land is not hidden by near terrain). SSIM unchanged; the horizon depth test passes.
5. **LOD streaming without holes, in-view tiles first** — tiles leaving the selection stay
   drawn until the tiles replacing them are ready (`hearth_lod::cover`: a parent until all
   its children are built, children until their parent is; never both), and builds go to the
   tiles in view before those behind the camera (weighted as four times farther). Quality of
   motion rather than frame time: the benchmark builds every tile up front; unit-tested
   (refine, merge, nothing built) and smoke-tested in the preview.
6. **Dithering in the final pass** — triangular noise of about one 8-bit step added in sRGB
   space where the output is quantized (a per-pixel hash, static so nothing crawls): the
   stepped bands of dusk skies and fog are gone (contrast-stretched comparison of the sunset
   sky). Tonemap pass 0.03–0.04 → 0.055 ms; SSIM 0.996–0.998 against the goldens, the
   difference being the intended per-pixel noise.
7. **Screen-space-error LOD selection** — each built tile records its vertical error (half the
   largest step between neighbouring column tops); a tile is split when the part of that
   error finer columns would remove (× (1 − 1/column width)), projected at its distance (and
   foreshortened by the viewing angle), exceeds the allowed error — at most three levels past
   the distance rule, which stays as the floor. Hysteresis: a tile split before stays split to
   110 % of the split distance and down to 70 % of the error. Refined selections are balanced
   (neighbours at most one level apart) so the tile-edge skirts still seal every border. Errors
   are known once tiles are built, so the stream re-selects when a rough tile arrives (at most
   every 15 frames), and tools build in rounds. Measured before choosing the tier (summit,
   forest; SSIM against a 0.25 px reference): distance rule 1.21 / 1.34 ms GPU, SSIM 0.749 /
   0.979; 4 px +0.1 ms, 0.749; **2 px** +0.5 / +0.2 ms, 0.790 / 0.979; 1 px +1.5 / +0.7 ms,
   0.923 / 0.980. The first steps fix what shows most — the coarse terraces and blocky trees on
   slopes seen from above — so the default (Fancy) allows 2 px, Fabulous 1 px, Fast 4 px
   (`lod_detail`). Alternating A/B runs of the 2 px default against the distance rule (two
   rounds each): summit 810 → 590 FPS (GPU 1.21 → 1.67 ms; LOD pass 0.70 → 1.15 ms; tiles
   drawn 454 → 742; triangles 1.67 → 3.14 M), forest 724 → 655, storm 703 → 643, coast
   1194 → 1124, underwater 1244 → 1170 (1 % lows there dominated by 40–50 ms submission
   hitches in both builds), cave 2203 → 2181. SSIM against the previous images: summit 0.939
   (the intended finer slopes), every other scene ≥ 0.998. Selection CPU ≤ 0.04 ms per frame;
   LOD preparation 0.03 → 0.06 ms. A quality gain paid in frame time — justified in D57.
8. **LOD quads grouped by facing** — each tile's quads are stored in six groups by the way
   they face (down, north, west, up, east, south; a ground top lit as shade under a crown
   still faces up), and each frame the CPU draws only the runs of groups that can face the
   camera: a side group when the tile is not wholly behind its faces' planes, tops when the
   camera is above the tile's lowest point, bottoms when below its highest (plus the
   curvature's tilt). The LOD pipeline draws both sides of every face (skirts must show from
   either side), so the back faces cost vertex and fragment work; now a typical tile draws
   half its sides. Draws per tile 1 → 1–3 (still one indirect-count multi-draw). Alternating
   A/B against result 7 (two rounds each): summit 592 → 805 FPS (LOD pass 1.14 → 0.69 ms,
   triangles 3.14 → 1.90 M), forest 658 → 788, storm 638 → 764, coast 1128 → 1246,
   underwater 1249 → 1328, cave unchanged — result 7's cost is won back (summit 810 FPS
   before it). SSIM ≥ 0.9993 in the open scenes: the differences are LOD column walls seen
   from inside through the dithered cubes of the handoff band, where other LOD surfaces now
   show; underwater 0.987, where the backs of skirts hanging under the distant water surface
   no longer draw dark dashes along the horizon (that view is incomplete until V2-2e).
