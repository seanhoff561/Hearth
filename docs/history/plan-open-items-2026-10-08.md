# Open items, Audit 0 to Audit 1 (2026-10-08)

*Moved from `PLAN.md` when E4.1 was inserted; still open, each when its file is next touched.*

## From Audit 0 (medium priority; each when its file is next touched)
- Split the eight files over 2,000 lines along the cut lines in AUDIT-0 (`client.rs`,
  `workshop.rs`, `server.rs`'s `run` into a `ServerWorld`, `ecology.rs`, `live.rs`,
  `screenshot.rs`, `bench.rs`, the bot test).
- One `smoothstep`; debug tools (F3+T, the counting allocator) behind Developer mode; material
  statuses derived from use; planned knowledge cut to id, name and a line; Q §3's repetition
  checks in the screenshot suite (with S2).

## From E3 (open)
- The year from a loincloth (`slice_year`) cannot run on Earth's clock as it stands: a real year
  at rest's 100 times is some ninety hours of server ticks. Make it a season (spring to the first
  frosts), or let it pass the quiet days at Creative's faster speeds with the animals on their
  populations' tier; until then the soak suite skips it.

## From E4 (open)
- The full suite was not run after E4 (a)–(d) (it takes too long here; the owner asked to
  defer it). Known: E4 (a)'s land share (0.3 → Earth's 0.29) redraws every test planet's coasts,
  so tests that rely on terrain about a spawn may need new places. The two seen failing
  (thatch's level ground, felling) were fixed at Audit 1. Run `cargo test --profile dev-opt
  --workspace --no-fail-fast` when the owner allows the time.
  Verified so far: `hearth_worldgen`, `hearth_body`, `hearth_player`, `hearth_craft`,
  `hearth_math`, clippy, fmt, content lint.
- The grid's middle heights (1–3 km) are short of Earth's (`planet.md`): more and longer ranges.
- The Body panel's words for breathlessness and mountain sickness.

## From E5 (open)
- The waking life lies until it acts (D251): run the bot and acceptance suites (they act at
  once, which gets up) with the full suite above.
- A place's card on the globe's hover, not only on the click (E §6.3).

## From P3 (open)
- The bot and acceptance suites drive the server directly; a scripted playtest through the
  client's buttons (the §5.2 examples end to end) wants a headless client (with the full suite).
- The action menu as a radial on the controller; sub-object picking with S5 and P7.

## From P4 and E6 (open)
- Real effects per stroke beyond digging (done at S1: each stroke digs its share), notches cut
  in a trunk; rain filling a pit left part dug, a hide left part scraped drying stiff.
- Cooking by core temperature; smoking, salting, fermenting and tanning as state models.
- The hands at work in first person.
- Gathering one thing at a time, the loose-objects layer and harvests from living plants: P7
  (E §9.1).

## From P5 (open)
- Rivers flowing at their current (their surfaces carry only the wind's waves), shallow water
  slowing the waves near shore, waterfalls and splashes.
- Fog banks that drift, lightning, flames that flicker (the fire's own model).
- The timing check reads the shaders' speeds from their source; a check that renders short
  sequences and measures the motion in the images would also catch a wrong scale in a
  uniform.

## From Audit 1 (medium; as each file is next touched)
- Work kept part done (`begun`): forget it when its block or thing is gone; key work in hand by
  the stack held, not one per process.
- A learned hand preference on a thing is kept for that kind of thing, not every thing.
- Duplicates: `places.rs`'s distance and x-wrap (the planet's own), the spring's day offset
  (the calendar's), `When` from a start (Birthplace and `app.rs`), the animator's `support_y`
  and `Pose::lowest`; one bench per refresh in the hands.
- The hands' and action menu's words as language keys, with the vessel's own name.
- The place card's cold and heat against the difficulty's thresholds, with a source.
- Split `client.rs` (aim, hands, motion, HUD, debug) and `workshop.rs`; the animator's work
  poses and gestures into a module.
- Low: `intents.ron` in its run order; drop `Found::Sea`.
