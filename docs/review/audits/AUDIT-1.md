# Audit 1 — after the Earth-true start and the hands

*2026-10-08, after P5 (Amendment Q §8.2). Covers E1–E6 and P3–P5; P6 was removed (D258). An
independent reviewer, given only the charter, re-read the newest modules (places, the
Birthplace screen, the hands and action menu, the intent resolver and its rules, the animator,
the work kept part done and drying). Its verified findings are folded in below.*

## Metrics (Q §9), against Audit 0

| What | Now | Audit 0 | Note |
|---|---|---|---|
| Rust, non-test (`src/`) | 123,200 lines, 24 crates | 117,300 | hearth 32.2k (+4.6k: places, creator, Birthplace, hands, work), worldgen 17.4k (+2.1k: Earth's grid and levels), character 3.4k |
| Dead-code allowances | 12 | 12 | at the threshold |
| Files over 2,000 lines | 8 | 8 | `client.rs` 4,390 (+685), `workshop.rs` 3,522 (+321), `ecology.rs` 3,000, `live.rs` 2,999, the bot 2,284, `server.rs` 2,283, `bench.rs` 2,226, `screenshot.rs` 2,221 |
| Restart files | PROGRESS 24 KB → 12 KB after this audit's trim, DECISIONS 20, PLAN 19 | 5, 17, 19 | older PROGRESS entries moved to `docs/history/` |
| Repository | 15 MB tracked | 12.7 MB | review images of E2–P5 |
| Unused dependencies, unlicensed assets | none | none | `scripts/lean-check.sh` |
| Frame and tick times | not measurable here (software device) | — | the gate waits for the PC; the cheap paths measured: picking 2.5 µs, a place's survey 7.8 s for the Earth's suggestions (off the frame) |
| Repetition scores | none yet | none | still on the fix list |

## Lean pass
- **Removed:** the action menu's unused `first_act`; the resolver's unread `HandUse::rule` and its
  lifetime; a no-op division; an empty branch in the place finder.
- **Found, for when each file is next touched (PLAN):**
  - duplicates: `places.rs` re-implements the planet's great-circle distance and x-wrap; the
    spring's offset in days repeats the calendar's private constant; the Birthplace screen and
    `app.rs` both turn a start into a `When`; the animator's `support_y` repeats
    `Pose::lowest`'s corner loop;
  - the hands build their bench three times a refresh, and filter what is lying twice;
  - the menu's words are hard-coded English ("fill the skin" for any vessel);
  - `places.rs`'s `words()` special-cases argument names.
- `client.rs` grew by 685 lines; Audit 0's cut lines stand (aim, hands, motion, HUD, debug).
  The animator (1,210 lines) would split its work poses and gestures into a module of their own.

## Bugs found and fixed
- **Resuming part-done work** showed nothing done and the whole time left for a tick. It now
  shows what was done.
- **Looking closely at drying work** before its first minute said it was dry through. It now
  says it is wet.
- The look's sun ignored a roof. The look and the drying now share one test of cover and sun.
- **Drying's equilibrium** was always below "dry", so meat dried through even in saturated air.
  The thing's water activity now meets the air's humidity: dry through at 75 % relative
  humidity, staying leathery in damper air ("it will dry no further").
- **The action menu's time for felling** ignored the trunk's girth (up to 40 times the process's
  time). Felling shows no guess there now; the work's line gives the time once begun. Spans under
  45 s read in seconds, not "about a minute".
- **A felled slim tree's stem** was clipped where it met the ground: its trunk is limbs, not
  logs, so the fall rested on nothing. The stem now counts its thick limbs, and the felling test
  (failing since P4) passes. The thatch acceptance looks inland when its beach spawn has no
  level ground, and passes.
- **Docs:** `start.md` named the wrong screen file and ring count; the animator's header and its
  `left_leads` comment were wrong.

## Realism and cohesion pass
- **Gaits:** the feet slid at some 0.5 m/s at a walk; P5 fixed this for people and animals
  (`docs/review/p5/gaits.png`).
- **Work poses** stand on the ground (`docs/review/p4/work_poses.png`). The bodies are boxes
  until S6.
- **Mountains at Earth's size** (`docs/review/e4`): the slopes read as speckled steps of 1 m
  voxels at distance, which S2's smooth surface addresses. The lakes are flat and bright.
- **Light** is not in real units yet (S2). The screenshots' looks need the PC.

## Fix list
- **High:** all done above.
- **Medium (PLAN):**
  - work kept part done is never forgotten when its block or thing is gone, and work in hand is
    one per process, not per stone;
  - a learned preference on one thing applies to every thing;
  - the duplicates and the language keys listed under the lean pass;
  - the card's cold and heat thresholds against the difficulty's, with a source;
  - the full test suite;
  - splitting `client.rs` and `workshop.rs`.
- **Low:** the order of `intents.ron`'s entries; the `Found::Sea` variant that is never claimed.
