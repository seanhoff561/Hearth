# Audit 0 — the baseline after E0

*2026-10-08, after E0 (Amendment Q §8.3). The first audit: it sets the baseline the later ones
compare with. A fresh-eyes review of the code, data and docs was made by an independent reviewer
with no context but the charter; its verified findings are folded in below.*

## Metrics (Q §9)

| What | Now | Note |
|---|---|---|
| Rust, non-test (`src/`) | 117,300 lines in 24 crates and the bench tool | hearth 27.6k, worldgen 15.3k, fauna 12.6k, render 12.0k, content 8.3k; E0 and this audit removed about 44,000 |
| Dependencies | 56 direct, 180 packages in the build | duplicate versions: hashbrown, linux-raw-sys, miniz_oxide, rustc-hash, rustix, syn, thiserror (all through dependencies) |
| Dead-code allowances | 12 (7 of them `unused_variables` in the bot test) | threshold 12 |
| Files over 2,000 lines | 8 | `client.rs` 3,705, `workshop.rs` 3,201, `ecology.rs` 2,992, `live.rs` 2,979, `tests/bot/mod.rs` 2,275, `bench.rs` 2,226, `server.rs` 2,197, `screenshot.rs` 2,141 |
| Restart files | 41 KB (PROGRESS 5, DECISIONS index 17, PLAN 19) | were 393 KB (138, 229, 26) |
| Repository | 12.7 MB tracked | `docs/review` 4.6 MB of it; archived era images and stale generated graphs dropped from the tree |
| Release binary | 21.7 MB | |
| Build | dependencies and one test in the dev-opt profile: 6.5 min on the cloud machine's 4 cores | the release profile's fat LTO took ~45 min to build the test binaries |
| Test suite | see the PROGRESS entry (dev-opt, the whole suite) | default suite budget 30 min (`budgets.md`) |
| Content | 103 materials implemented of 359; 1,551 processes; 86 knowledge nodes implemented, 94 planned; 1 era playable of 10 | 121 materials held to measured ranges |
| Frame and tick times | not measurable here (software device) | the gate and a GPU profile of the worst scene wait for the PC (`scripts/perf-gate.sh`) |
| Repetition scores | none yet | the checks of Q §3 are not built (fix list) |

## Lean pass

Removed in this audit (all verified unused by search across the workspace):
- **13 dependencies** that no source named; **51 public items** nothing called (math helpers,
  world storage and lighting helpers, noise helpers, render counters, a fuel getter, a dead
  illness treatment, …); `hearth_core`'s event queue, tick scheduler, registry id mapping and
  the 24,000-tick day of the original game; the folders made for resource packs, mods and logs.
- **About 30 options** nothing read (chat, skin, resource packs, brightness, cloud height,
  particles, biome blend, entity distance and shadows, smooth lighting, chunk builder, attack
  indicator, planet curvature, wind sway, the shader group but water, raw input, scroll
  settings, auto jump, darkness pulsing, damage tilt, high contrast, the map-knowledge setting);
  FXAA (never built). The presets were renamed Low, Medium and High over the three settings
  they control; an old file's names still load (D231).
- **18 key actions** nothing handled (use, swap offhand, chat, command, player list, hide HUD,
  screenshot, smooth camera, seven debug chords of the original game, hotbar slots 7–9) and 5
  input helpers; **89 language keys** nothing used.
- E0's leftovers: the band camp code (220 lines), the archaic body plans and child growth, the
  world list's age (read from a field E0 removed: every adult showed as 0), a dead permadeath
  flag, an item owner nobody read, a help line for a deleted command.

Fluff kept for now, with its milestone: `Category` and its headings (Q1 groups the Controls
screen by them or removes them); `skin_presets` and `Figure::set_appearance` (E5's creator); the
two meshers D222 rejected (`hearth_smooth`, until S2's mesher lands); the region-file save
store used only by its tests (decided at S1); about 65 public items used only by tests.

## Realism and cohesion pass

- **Materials against measurement** (`data/hearth/materials/reference.ron`, the lint): 24 of
  121 materials were outside their family's measured albedo. Basalt, scoria, obsidian, the coals,
  charcoal, black sand, humus, chernozem and peat were darker than any real surface of their
  kind (charcoal 0.011 against about 0.04); shell sand (0.81) and birch bark (0.83) were as
  bright as fresh snow; five pale woods brighter than the palest timber. Each base colour was
  scaled to the measured luminance with its hue kept.
- **Animals at sea:** a herd's or family's place of the day was drawn anywhere in its range,
  open sea included; on a coast the land animals' groups stood in the ocean and were never met.
  Groups now move only within cells of their medium (`Ecology::lives_in`).
- Found, not fixed here (fix list): on the Tiny test planet, biomes change every few hundred
  metres about seed 7's spawn and several realms' animals mix (E4 recalibrates at Earth's
  size); the lighting is not yet in real units (S2); the screenshots' looks need the PC.

## Leftovers of the original game (Q §5.8)

| Leftover | Plan |
|---|---|
| Pixel font and pixel-style panels (`hearth_ui` glyphs, `gui_scale`) | Q1 replaces them with an OFL typeface as SDF text |
| Fast / Fancy / Fabulous presets | done: Low / Medium / High (D231); Ultra when S §12.3's settings give it something to set |
| Resource-pack layout: option, folders, `write_pack`'s `.mcmeta`-style output, `block/<name>` texture names, `BlockDef.model` | option and folders gone; `write_pack` and the block JSON fold into the RON content at S1; texture names go at S2 |
| 16×16 textures and cube block models | natural ground at S2, grass at S5/P7G, built pieces at S7; the coat painter stays until S6 |
| The 48-minute day | E3 (real Earth time) |
| Sound categories modelled on the original game (Hostile, Friendly, Blocks, Music without a source; 5 of 9 shown) | Q1 |
| The generic decorator's flowers, sugar cane by any water, cacti and cobblestone boulders | P7 and P7G |
| Unused block shapes (Slab … Cake), unread block fields (`resistance`, `behavior`, `item`, `model`) | S1, with the block data |

## Fix list

**High (done in this audit):** everything under the lean pass; the two realism fixes; the
restart files; `budgets.md`, `scripts/lean-check.sh` (in `scripts/check.sh`), the reference
table and its lint; the design docs that still described people, births and the character
screen, or listed built systems as planned; `art-direction.md` rewritten for photographic
plausibility (Q §2.2); tests built in the dev-opt profile (no LTO) for iteration and CI.

**Medium (in `PLAN.md`):** split the eight long files (the reviewer's cut lines are in this
report's history: client by aim, hands, motion, HUD and debug; workshop by fields, animals,
trees, ground and fire; server's 1,200-line `run` into a `ServerWorld` with setup, messages,
tick and streaming; ecology, live, screenshot, bench and the bot similarly) as each is next
touched; one tick-rate constant (done) and one `smoothstep`; debug tools behind Developer mode
(F3+T, the counting allocator); the repetition checks of Q §3 in the screenshot suite; material
statuses derived from use (251 marked planned are used); planned knowledge reduced to id, name
and a line.

**Low:** three notes longer than two sentences; the bot test's unused variables.
