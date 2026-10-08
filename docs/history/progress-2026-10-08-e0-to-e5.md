# Progress, E0 to E5 (2026-10-08)

*Moved from `PROGRESS.md` at Audit 1.*

## E5 — the Wild Earth start (2026-10-08, D249–D251)
- The character creator is back (making a world: name and mode, the place, then who; and the
  death screen):
  - saved profiles; body, height and build;
  - skin tone, undertone and freckles; six face presets and seven feature sliders;
  - eyes; eleven hair styles with length, natural colours and a fine picker; eyebrows; facial
    hair; a name;
  - Randomize drawing adults' variation with pigmentation correlated as it is;
  - the person turning under daylight, overcast, dusk, firelight and moonlight
    (`docs/review/e5/`).
- Suggested places: three to five per new world, of different climates and far apart,
  survivable at the date.
  - Each card has a plain name, the climate and season now, the terrain and a difficulty.
  - "What to look for": water, toolstone on the ground, wood, fibre, food in season, tinder,
    each with distance and direction.
  - "Watch out for": predators, venomous and defensive animals, cold nights, heat, thin air.
  - Every claim is found by sampling the world made; a place without fresh water is passed over.
  - Choose anywhere reads any spot's card. 5 places in 7.8 s on Earth's planet (≤ 15 s).
- Eras: every era listed; the ones not yet playable are greyed, "Coming soon".
- A life wakes twenty minutes before sunrise on a spring day, lying awake in the grass as the
  eyes open and the birds sing; moving gets up.
- The death screen offers:
  - who begins the next life;
  - a new life near where the last one lived, at a place suggested for the season, or anywhere;
  - restarting the world.
- Deferred (`PLAN.md`): the full suite (with E4's); cards on hover; Easy's hints at waking (removed with P6);
  Near a friend (Phase R).
- **Real?** Heights about real adult means. Pigmentation correlated as the genes act. Places
  judged by the climate normals of the day. Every card's claim found in the world (tested by
  finding each again).
- **Lean?** One places module serves new worlds, Choose anywhere and new lives. The profiles of
  before load as they were.
- **Fast?** 3 s on a test planet, 7.8 s on Earth's. A card on a click in some 0.3 s.
- **Whole?** The creator, places, eras, waking and death fit together end to end:
  Create World → places → who → dawn → death → a new life as chosen. The layout test lays out
  the creator at every window size.
- **Organic?** Places come from the world's own rivers, springs, stones, plants and animals.
  None are authored.

## E4 — Earth's size (2026-10-08, D244–D248)
- Every world is Earth's: 40,075 km round, a block a metre up and down; the size, height,
  rarity, land-share and spawn-climate settings are gone (Developer mode keeps the test planets).
- The grid takes Earth's heights at Earth's size (the test planets keep theirs): land 29 %, mean
  land 600–800 m (Earth 797–840), mean ocean 3.54–3.64 km (Earth 3.68), ranges to 6–7 km over
  20 km, trenches to 8–11 km; short in the middle heights (`bench hypso`, `planet.md`).
- Refinement levels between the grid and the blocks (2.4 km, 306 m, 38 m), each made a tile at
  a time from the one above: relief at its own wavelengths as rough as the place, its own
  drainage cut by Priority-Flood with breaching and stream-power erosion, the parent's rivers
  kept and falling all the way, the sea over the low ground joined to it, the parent's lakes in
  their outline; seamless and the same whichever tile is made first (`terrain.md`,
  `docs/review/e4/`). The blocks read the finest level once per area; far tiles read coarse
  ones: the far view builds in 11 s (9 s before). Rivers take the hydraulic geometry.
- Life at real heights: aerobic capacity with the air's oxygen, acclimatisation over days,
  mountain sickness climbing too fast, the death zone; boiling slower up high; cloud bases
  over the ground (`physiology.md`).
- Creating an Earth world: the grid in some 15 s on four cores, with the progress screen; the
  first view about 11 s more.
- Deferred (`PLAN.md`): the far-field layer to the horizon goes to S4; the full suite to Audit 1
  (E4 (a)'s new coasts move some tests' places); the middle heights; altitude in the HUD.
- Real? Hypsometry against Earth's measured, rivers by hydraulic geometry, altitude by the
  standard atmosphere and VO2max data, lakes 1–4 % about the sites against Earth's 3.7 %.
  Lean? One refinement module, one neighbourhood read per area, shared atmosphere functions;
  the size and height settings gone. Fast? A tile in some 23 ms, a column in under a microsecond,
  the far view near its old time. Whole? Rivers, lakes, coasts and relief agree across levels and
  tiles (tested: no river runs up, means kept, values the same whichever tile first); the body,
  cooking and clouds know the height. Organic? Valleys and lakes come from drainage and wear,
  not stamped shapes; the test planets keep their own model.

## E3 — Earth's clock (2026-10-08, D240–D243)
- One clock, Earth's: tick 0 is a moment of real time, a day 86,400 s, the dates real, leap
  years and all; the seasons are the sun's own (its ecliptic longitude). The sun by the USNO's
  approximate coordinates with the equation of time, the moon by a low-precision series (its
  phases on their days), the stars by sidereal time: sunrise and sunset within four minutes of
  published tables in London, New York and Sydney, polar night and day at Tromsø. A world begins
  on a spring morning where its first life is, or today, now (Create World); save format 7
  (format-6 worlds walked forward), protocol 5.
- Every rate real: the time scales and their settings are gone; processes, healing, illness,
  spoiling, weathering and growth run their real hours, days and years. The audit found three
  things tuned to the short day per tick: a vegetation fire's chances (now over game seconds; it
  raced thirty times over), the animals' thirst and their signs' age (the real day), stamina
  back in 30 s (now a 100-s time constant, as phosphocreatine); sleep comes after a quarter of
  an hour lying sleepy.
- Rest is the only way time goes faster: the sleep key opens Rest (sleep until morning or
  rested; rest one, two or four hours or until dusk; wait until the work left nearby is done);
  the world eases up to 100 times, living every tick (the animals in half-second steps, the body
  in half-minutes), until what it was for comes or the body's needs, a hurt or an animal close
  end it; a line says how it went. Work in hand is never hurried (the work warp is gone).
  Creative's clock shows local mean time and the date. `docs/design/time.md`,
  `docs/review/e3/rest.png`. Suite: 608 passed, 60 soak tests ignored, in 6 min 43 s.
- The year from a loincloth (soak) cannot run a real year as it stands: set aside, to be cut
  to a season before Audit 1 (`PLAN.md`, "From E3").
- Real? The calendar and the sky are Earth's to minutes against published tables; every rate in
  data is a real one; the three per-tick holdovers now run in real time. Lean? The time scales,
  their settings and conversions, the work warp, the per-call day length and the sleep-speed
  constant are gone; one clock, one ease for rests, one `Calendar::when` for the tools' dates.
  Fast? The sky is a few dozen trigonometric terms a sample; resting at 100 times costs ten
  animal steps a tick while no terrain streams; tests hurry their waits at 1,000 times. Whole?
  Body, fire, food, plants, animals, weather and sky run on the same seconds, and a rest lives
  every tick (the finite water's flow keeps its own pace: known). Organic? Light and seasons
  follow the real sun at each place and date; no fixed day lengths or season starts remain.

## E2 — controls (2026-10-08, D237–D239)
- Any key alone: key releases reach the Controls screen's capture, so Left or Right Ctrl, Alt or
  Shift let go alone binds that key. A hold action on a lone modifier lasts through its
  combinations; a tap one comes on the release if nothing else was pressed (the debug key's rule,
  generalised); the screen explains such overlaps in a note line. The system's shortcuts are
  refused and left alone in play; Option and Command named on macOS. Every controller button
  binds, in a column of its own (crouch, run and crawl toggle on a pad); mouse side buttons bind.
- A click at nothing uses what is in hand: eat, drink, treat a wound, hold a brand up (a fire to
  hungry animals), draw and loose a bow; else the item's data-defined blow (thrust, swing,
  slash, stab, strike), else the fist; an empty hand punches; T kicks. Blows are the server's:
  wind-up, strike and recovery at real speed, stamina, reach, paths tested against animal bodies
  (misses happen), blunt, cutting or piercing wounds and a shove by momentum; the body shows
  them (`docs/review/e2/gestures.jpg`). Protocol 4. `docs/design/controls.md`. Suite: 600
  passed, 60 soak tests ignored, in 6 min 48 s.
- **Manual check for the owner (Windows):** a lone Alt neither opens the window menu nor takes
  the focus; AltGr acts as Right Alt (`dev/PLAYTEST.md` #19).
- Real? Speeds, energies and reach from sports biomechanics and V2-7's hunting figures, the
  bow's arrow speed from its draw, cuts and shoves from the wound model and momentum; the
  uncertain ones marked, for P5's recorded-motion check. Lean? One input path for keys, mouse
  and pad buttons (the pad's hard-wired flags gone); the debug rule generalised, not doubled; one
  `use_of` and one blow choice for client and server; throwing and loosing share their flight.
  Fast? Input costs nothing measurable; a blow tests a few segments against nearby animals
  once. Whole? Blows use the body's stamina and the animals' wound model; every action works
  from a pad; other players wait for PvP (R3). Organic? A blow lands where the animal is then,
  so the outcome follows what it does.

## E1 — the future humanity plan (2026-10-08)
- `docs/design/future/humanity/`: a README, one document per section of E §10 (the three loops;
  the History Engine, Historian and World Bible; the Person record and levels C0–C4; System 1,
  System 2, the planner and memory; conversation and voice; societies; lives and childhood;
  compute; eras; the hard safety rules; evaluation; foundations), the F0–F12 roadmap and research
  notes checked against the papers. `future-humanity.md` points there. No code.
- Real? The design rests on cited work (generative agents, Project Sid, Voyager, AgentSociety,
  Turchin's models, Henrich, SHOP2, ORCA) with its limits noted. Lean? Documents only; the old
  page cut to a pointer. Fast? Budgets to be measured in F0. Whole? One person model at every
  level; the same Actor as players. Organic? Not applicable.

## Q1 — the interface's look (2026-10-08, D236)
- Source Sans 3 for the interface and Source Serif 4 for the journal (SIL OFL,
  `ASSETS_LICENSES.md`) drawn from signed distance fields in one atlas, sharp at every
  interface scale; umber panels and warm off-white words; the journal as ruled notebook paper in
  inks; scrolled areas keep their bar's room; `ui=` draws a specimen in screenshots
  (`docs/review/q1/`). Item icons from meshes wait for S6's item meshes (D236).
- Real? Real typefaces at sizes matching the old capitals; the journal reads as a field
  notebook. Lean? The 560-line pixel font and its glyph table gone; one atlas and one shader
  path; no icon pipeline for boxes. Fast? The atlas is built once at start (1 MiB on the GPU); no
  per-frame cost added. Whole? Every screen passes the layout test at every resolution and
  scale; words on panels about 15:1, dim words 7:1, the journal's inks at least 4.5:1; checked in
  the specimen at scales 1 and 3. Organic? Nothing generated.

## Audit 0 — the baseline (2026-10-08, D231–D234)
- Metrics: 117,300 lines of non-test Rust in 24 crates; 56 direct dependencies; restart files
  41 KB (were 393); 8 files over 2,000 lines; no frame or tick numbers here (the PC's gate).
- Lean: 13 dependencies, 51 public items, ~30 options, 18 key actions, 89 language keys and E0's
  leftovers removed; presets Low, Medium, High; `scripts/lean-check.sh` in `scripts/check.sh`.
- Real: `materials/reference.ron` and its lint (24 colours brought to measured albedo); land
  animals no longer placed at sea (this was E0's failing hunting test).
- Fast: tests build in the dev-opt profile, not fat LTO (D233); `docs/design/budgets.md`.
- Next: Q1. Medium findings are in `PLAN.md`; the report is `docs/review/audits/AUDIT-0.md`.

## E0 — remove the human systems (2026-10-08, D230)
- Archived first (branch `archive/humans-v2.1`, f244710; `docs/archive/humans-v2.1/`), then
  removed `hearth_people`, `hearth_ai`, the persons, births, childhood, conversation, history
  and chronicle modules and screens, the people's protocol (now version 3), data, schemas,
  lints and tests: 42,900 lines. After death a new life begins here or elsewhere; saves are
  format 6 and older worlds, which had people, are refused with E §2.4's message. Content
  lint: 0 errors; every implemented knowledge node and process reachable by a lone player.
- Real? The least Earth-true part is gone; nothing invented added. Lean? Two crates and 42,900
  lines removed, no shims for old saves or protocol. Fast? Ticks no longer run persons or
  history. Whole? The death screen and era list pass the layout test; the Actor rule (E §2.3)
  stands for P3, P4 and E6. Organic? Nothing generated changed.
