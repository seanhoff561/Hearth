# Budgets

*One table for every system's cost (Amendment Q §7). Audits check it; a feature is not done
until it meets its line. "Measured" is the latest audit's number (Audit 0 unless noted); "—" is
not measured yet, with where it will be.*

Reference machine (the specs'): an 8-core CPU and an RTX 3060 / RX 6600-class GPU at 1440p.
The owner's PC (Ryzen 7 7840HS, RTX 4060 Laptop) stands in for it; the cloud machine renders
only on a software device, so frame and GPU numbers come from the PC.

## Frame (render thread and GPU)
| What | Budget | Measured | Source |
|---|---|---|---|
| Frame rate, render distance 12 + LOD 512, High | ≥ 144 FPS average, no spike > 2 × median | 745–2178 FPS average at 1080p, Fancy (perf audit) | v1 §8.4 |
| Frame time against Baseline-S | p50 ≤ +10 %, p99 ≤ +15 % at High; Low at or better | Baseline-S's CPU half recorded; GPU half on the PC | S §12.2 |
| Any change, gate scenes | ≤ 5 % fall in average FPS or 1 % lows | `scripts/perf-gate.sh` on the PC | D59 |
| Grass and ground cover | ≤ 1.5 ms GPU High, ≤ 0.6 ms Low (1080p), ≤ 0.2 ms CPU | — (P7G) | P §11.5 |
| A close-up character | ≤ ~1 ms GPU at High | — (E7) | E §8 |
| Distant-tile selection on the frame thread | ≤ 1 ms | 5–9 ms after a 16 m move (known) | perf audit |

## Tick (server, 20 ticks a second, 50 ms)
| What | Budget | Measured | Source |
|---|---|---|---|
| Whole tick | ≤ 25 ms worst (half the tick) | — (soak suite, Audit 1) | this table |
| Animals near players (~300) and instanced life | ≤ 3 ms across workers | — (bench) | v2 §21 |
| Ecological cells | ≤ 1 ms amortized; a planet's year headless ≤ 1 min | — | v2 §21 |
| Structural solve, typical edit | ≤ 2 ms, never blocking | — | v2 §21 |
| Physiology and thermal | O(1) per body per tick | holds by design | v2 §21 |

## Memory, VRAM, disk
| What | Budget | Measured | Source |
|---|---|---|---|
| VRAM total, High | ≤ 4 GB | 117–227 MiB (perf audit, before Amendment S) | this table |
| Terrain material textures | ≤ 1.5 GB High, ≤ 400 MB Low | — (S2) | S §12.2 |
| LOD VRAM | `lod_vram_budget_mb`, LRU | as set | v1 §8.4 |
| Memory per surface cube, save per explored km² | ≤ 1.5 × Baseline-S | Baseline-S recorded (S0) | S §12.2 |
| Installed game, assets included | ≤ 1.5 GB | — (no third-party assets yet) | Q §4 |
| Repository, tracked files | ≤ 40 MB; no file over 1 MB | 13 MB (Audit 0) | Q §5.6, `scripts/lean-check.sh` |

## Earth scale (E4.1 §5: `hearth bench globe|creator|load`, judged by `scripts/perf-gate.sh`)
Measured on the cloud machine (4 cores, a software adapter): the lower configuration.
| What | Budget | Measured | Source |
|---|---|---|---|
| The globe's map | made with the planet ≤ 2 s, read back ≤ 0.1 s, no refinement tile | 1.28 s, 0.03 s, none (with its relief, T1.4) | E4.1 §4.2, T §2.4 |
| Hovering over the globe | < 1 ms | 0.004 ms median | E4.1 §4.2 |
| A click's details | on a thread ("Looking closer…"), ≤ 1 s median | 0.48 s median, 1.2 s slowest | E4.1 §4.2 |
| The places suggested | ≤ ~15 s | 5.5 s | E4.1 §4.2 |
| The creator: a colour slider | shown the next frame | the next frame | E4.1 §4.3 |
| The creator: a shape slider | the last setting shown ≤ 0.25 s, full detail ≤ 1 s on the reference machine | 0.14–0.15 s, 0.9–1.2 s (three threads) | E4.1 §4.3 |
| A new world: Play → in control | ≤ ~10 s on the reference machine | 3.35 s | E4.1 §4.4 |
| A save: Play → in control | ≤ ~5 s | 2.34 s | E4.1 §4.4 |
| The whole render distance | ≤ ~30 s on the reference machine | 34.6–37.1 s (llvmpipe's drawing) | E4.1 §4.4 |
| The main thread while loading | its own work ≤ 1 ms a frame | 0.35–0.46 ms median | E4.1 §4.4 |
| The caches together | ≤ 40 % of the machine's memory, a budget per kind | as set (`hearth_core::memory`) | E4.1 §4.7 |
| A 30-minute journey | resident memory flat once the caches are full | 1.6–1.8 GB from minute 3, 1.9 GB after the near arena's one doubling; 2.1 GB at the most | E4.1 §4.7 |
| Meshes a client has not taken | at most 512 in the channel; the rest one per cube in the server's outbox | 512; 0–14 MiB waiting | D287 |
| Finest-level tiles for coarse callers | none | none (`tests/fine_tiles.rs`) | E4.1 §4.1 |
| Disk caches, every planet together | refinement tiles ≤ 512 MiB, distant tiles ≤ 1 GiB | as set | E4.1 §4.4 |
| A gated number's median | ≤ 5 % worse than the baseline's, beyond a small floor | `bench earth-judge` | D59 |

## Times
| What | Budget | Measured | Source |
|---|---|---|---|
| World creation (Huge planet, the opening) | ≤ 60 s | — (E4 recalibrates at Earth's size) | this table |
| Loading a saved world | ≤ 15 s | 2.2 s to control, Earth (E4.1) | this table |
| Clean release build of `hearth` | ≤ 10 min on the cloud machine | see AUDIT-0 | Q §9 |
| Default test suite (dev-opt) | ≤ 20 min on the cloud machine | 8 min with the changed crates' build: 576 tests, 60 soak runs left out (Q1) | Q §5.5 |
| Soak suite (`scripts/soak.sh`) | ≤ 90 min, at audits | over an hour (54 long runs) | D235 |

## Restart files (Q §5.4)
| File | Budget |
|---|---|
| `PROGRESS.md` | ≤ 25 KB |
| `DECISIONS.md` (index) | ≤ 40 KB |
| `PLAN.md` | ≤ 20 KB |
