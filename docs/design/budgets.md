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

## Times
| What | Budget | Measured | Source |
|---|---|---|---|
| World creation (Huge planet, the opening) | ≤ 60 s | — (E4 recalibrates at Earth's size) | this table |
| Loading a saved world | ≤ 15 s | — | this table |
| Clean release build of `hearth` | ≤ 10 min on the cloud machine | see AUDIT-0 | Q §9 |
| Default test suite (dev-opt) | ≤ 20 min on the cloud machine | about 15 min of tests after the soak runs left it (Audit 0) | Q §5.5 |
| Soak suite (`scripts/soak.sh`) | ≤ 90 min, at audits | over an hour (54 long runs) | D235 |

## Restart files (Q §5.4)
| File | Budget |
|---|---|
| `PROGRESS.md` | ≤ 25 KB |
| `DECISIONS.md` (index) | ≤ 40 KB |
| `PLAN.md` | ≤ 20 KB |
