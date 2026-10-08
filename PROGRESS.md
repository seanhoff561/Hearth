# Progress

Current status only (Amendment Q §5.4). The full record of M0 to E0 (2026-09-30 to 2026-10-08)
is `docs/history/progress-2026-09-30-to-10-08.md`; decisions are indexed in `DECISIONS.md`
with their texts in `docs/decisions/`.

Direction: v2 (`docs/spec/v2-direction-change.md`) as amended by S (smooth world), P
(playability) and, above them, **E and Q** (`docs/spec/amendments-e-q.md`, D227): Earth-True
and the Quality Charter. V2.1's simulated humans were removed in E0 and archived
(`docs/archive/humans-v2.1/`, D230); Phase F plans them anew. Amendment R waits until V2-16.

## Resume
1. Read this file, `PLAN.md` and the `DECISIONS.md` index; `git log --oneline -20`. Open
   `docs/decisions/`, `docs/history/` or a design doc only when a task needs it.
2. Run `scripts/check.sh` (it runs `scripts/lean-check.sh`); the long runs are
   `scripts/soak.sh`, at audits (D235).
3. Work in `PLAN.md`'s order; each milestone ends with its five-test checklist (Q §8.1) below.
4. Builds: `cargo test --profile dev-opt` (no LTO, D233); `--release` is fat LTO, for the game
   and the perf gate. The cloud machine renders only on a software device: frame rates, the
   perf gate and how screenshots look need the owner's PC.

## Milestones
Done: M0–M3 (v1 engine), V2-0 – V2-10, V2-12 (the Neolithic), H0–H10 (removed in E0), S0,
P0, P1, P2, E0, Audit 0, Q1, E1, E2, E3, E4, E5, P3. V2-11 superseded.

| Next, in order | State |
|---|---|
| Audit 0, then its high-priority fixes | done 2026-10-08 (D231–D234) |
| Q1 — interface design | done 2026-10-08 (D236) |
| E1 — the humanity plan | done 2026-10-08 (documents only) |
| E2 — controls | done 2026-10-08 (D237–D239) |
| E3 — Earth's clock | done 2026-10-08 (D240–D243) |
| E4 — Earth's size | done 2026-10-08 (D244–D248) |
| E5 — Wild Earth start | done 2026-10-08 (D249–D251) |
| P3 — looking and the hands | done 2026-10-08 (D252–D254) |
| P4 with E6 → P5 → P6, Audit 1 | next |
| S1 → S2 → E7 → S3 → S4, Audit 2 | planned |
| S5 with P7 → P7G → S6 → S7 → S8 → P8, Audit 3 | planned |
| V2-13 → V2-14, Audit 4; V2-15 → V2-16, Audit 5 | planned |
| Phase R-A, Audit 6; R-B, Audit 7; R10; Phase F | planned |

| Row | State |
|---|---|
| Smooth world (S) | S0 done (D222: Surface Nets with sharp features, biplanar shading); Baseline-S's CPU half recorded, its GPU half needs the PC (`scripts/baseline-s.sh`); prototype mesher 3,553 surface cubes/s on one thread (target 2,000 on eight) |
| Playability (P) | P0–P3 done; open issues in `dev/PLAYTEST.md` |
| Earth-True (E) | E0–E5 done 2026-10-08; E6–E7 planned |
| Quality (Q) | Audit 0 done 2026-10-08 (`docs/review/audits/AUDIT-0.md`); open high-priority findings: none; next: Audit 1 after P6 |

## Latest: P3 — looking and the hands (2026-10-08, D252–D254)
- **The two hands:** left click works the left hand and right click the right (E the right;
  the controller's triggers). Each does its natural use on what is looked at, with what it
  holds:
  - the use comes from data (`interaction/intents.ron`, a resolver in `hearth_craft`,
    mind-agnostic);
  - the tools are that hand's;
  - a held thing is put away when the use wants an empty hand, and brought back after;
  - held, the button does the work again as each is done.
- **Safe defaults only:** no punching animals, no eating what is not known to be food, no
  building (linted).
- **The action menu** (the wheel's click):
  - every action for what is looked at, with its tools and about how long, greyed with what it
    lacks; one's own actions and rest when looking at nothing;
  - done with either hand;
  - a process picked twice running over a hand's own use becomes that hand's use there
    (`hands.json`; Controls forgets).
  - Creative's pick moves to Ctrl + the wheel's click.
- **By the crosshair:** what is looked at is outlined and named (off, brief or always), with
  what each hand would do. There is no offer list and no work bar. Discoveries are a quiet
  note by a small journal.
- **Picking:** some 2.5 µs a frame (under 0.2 ms asked).
- **Deferred** (`PLAN.md`): a scripted playtest through the client's buttons; the controller's
  radial menu; sub-object picking (S5, P7).
- **Real?** The hands do what hands do with the thing held: the amendment's every example is
  tested.
- **Lean?** The rules are data. One resolver serves both hands and any future mind; the offer
  list's code is the menu's.
- **Fast?** 2.5 µs a pick. The hands' uses are drawn up with the list, five times a second.
- **Whole?** Hands, menu, hints, highlight, names, stowing and learned uses work together. The
  layout test lays out the new Controls rows.
- **Organic?** A hand's use follows from what it holds and what is there, through the
  processes' own verbs.

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
- Deferred (`PLAN.md`): the full suite (with E4's); cards on hover; Easy's hints at waking (P6);
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

## Content
`hearth content status` (implemented = used by a system; planned = data only):

| Domain | Implemented | Planned |
|---|---|---|
| Materials | 103 | 256 |
| Rocks, minerals, provinces, deposit models, soils | 35, 27, 14, 52, 16 | 0, 3, 0, 0, 0 |
| Plant species | 223 | 3 |
| Animal species | 360 | 0 |
| Ecosystems | 20 | 0 |
| Item forms | 66 | 0 |
| Processes | 1,551 | 0 |
| Knowledge nodes | 86 | 94 |
| Workstations, construction pieces, garments | 11, 21, 11 | 0 |
| Injuries, illnesses | 10, 7 | 0 |
| Eras | 1 | 9 |

## Known issues
- Night by a fire (D201): a camera in the dark looking at a fire from beyond its light washes
  the firelit ground out white; adaptation is reckoned where the camera stands.
- Finite water: a channel dug through a river's bank a block below the water takes the river's
  water without end (the hydrology's rivers do not fall as a breach takes from them; D190).
- Presents never block in the cloud environment (likely an occluded window); re-check pacing
  on a visible window.
- Oceans: the sea's realms are its coasts' (D152); seabirds only over land cells; crabs, seals
  and sea fish not drawn walking; deep animals drawn near the surface; mangrove channels' fish
  not kept (D159).
- Wetlands: waterfowl stand on the bank rather than swim; a walking bird is drawn with its
  wings spread; reed beds hide much (D148).
- Mountains: the planet's tropics have seasons, so tropical mountains have a winter (D136; E4
  recalibrates); a realm's mountains are one community (D140).
- Time going faster (E3): the finite water's flow keeps its own pace; beyond 100 times the
  animals about the player lag the clock until the populations' tier takes over.
- Animals: no lion's mane or peacock's train; thin limbs under dense crowns draw nearly black.
- Distant terrain: the player's changes reach it as each column's top block; the tile
  selection runs on the frame thread (5–9 ms after a 16 m move); FXAA is not built.
- Not drawn yet: lightning, fog banks, wet and snowy surfaces, splashes; no shadow maps (S2,
  V2-16). Snow per column (no drifts). Sounds placed in the world, music, thunder, water and
  animal sounds wait (S6–S8).
- Deferred to their milestones (detail in the history file): flocks and insects seen; seasonal
  flowers; migrations as journeys; trees regrowing beyond their sites, rot, spreading fire,
  smoke; panning and deposit zoning (V2-13); warm fumarole ground; containers opened only by
  picking up, wetness of things; snow loads, decay on textures, arches and domes (S6, V2-13).

## Environment
- The owner's PC: Ryzen 7 7840HS (8C/16T), RTX 4060 Laptop + Radeon 780M, 16 GB; Rust by
  rustup (`export PATH="$HOME/.cargo/bin:$PATH"` in older shells).
- The cloud machine: 4 cores, 15 GB, no GPU (llvmpipe); the git proxy refuses tag pushes.
