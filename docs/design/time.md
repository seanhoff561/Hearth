# Time: Earth's clock, and resting while it runs

*Amendment E §4 (E3). Code: `hearth_env::astro` (the sun, the moon, the stars, dates),
`hearth_env::calendar` (a world's calendar and its moments), `crates/hearth/src/rest.rs`
(resting and waiting), `server.rs` (the clock and its start), `client.rs` (time in words).
Data: `data/hearth/time.ron` (how much faster a rest has the world go). Units: SI inside,
conversions only at the interface (`data/hearth/units.ron`).*

## Purpose
One clock, Earth's. A second of play is a second of the world for everything: bodies, work,
fire, food, weather, plants, animals, the sky, water, the seasons and lives. The sun, the moon
and the stars stand where they really do for the place and the date. Time goes faster only while
the player sleeps, rests or waits (and with Creative's time speed), and nothing is skipped: the
world lives every tick of it, only more of them to the second.

## The calendar (`calendar.rs`)
- Tick 0 of a world is a moment of real time, in days since J2000.0 (2000-01-01 12:00 UTC). The
  clock counts 20 ticks a second; a mean solar day is 86,400 s. Dates are the Gregorian
  calendar's, leap years and all (`astro::civil_date`, after Hinnant's algorithms).
- The year's share is the sun's ecliptic longitude (0 at the March equinox), so the seasons are
  the astronomical quarters, opposite in the south, and the tropical year (365.2422 days) falls
  out of the sun's own motion. The moon's phase is its elongation from the sun (the synodic
  month, 29.530589 days).
- Local solar time at a place is the sun's: UTC, plus the place's longitude as a share of a turn
  (`Planet::solar_time_offset`), plus the **equation of time**. Local mean time, which a clock
  keeps, leaves the equation of time out, so the sun's noon drifts from noon by the clock: some
  14 minutes late in mid-February, 16 early at the start of November.
- A new world's clock is set (`life.start` in its settings) to:
  - **A spring morning** (the default): twenty days after the equinox that begins spring in the
    first life's hemisphere, at seven by the sun where it begins, in the year the world is made;
  - **Today, now**: the real date and time the world is made.

  The world's creation time (`created_unix` in `level.json`) and its first spawn fix it each time
  it opens. A world never saved (tests, tools) takes the year 2000's spring.

## The sky (`astro.rs`)
- **The sun:** the U.S. Naval Observatory's approximate solar coordinates (mean anomaly and
  longitude, the equation of the centre, the obliquity 23.439° falling 3.6 × 10⁻⁷° a day), good
  to about a minute of arc from 1950 to 2050: right ascension, declination, ecliptic longitude
  and the equation of time.
- **The moon:** a low-precision lunar series (the mean longitude, the chief inequality of
  6.289°, the latitude of 5.128° about the node), about half a degree, so its phases and its
  rising fall on their real days.
- **The stars:** Greenwich mean sidereal time; the star field turns once a sidereal day about the
  celestial pole, which stands at the latitude's height.
- **Seen from a place:** altitude and azimuth from the hour angle; sunrise and sunset at −0.833°
  (refraction and the sun's disc), polar day and polar night where the sun never crosses it.
  Twilight is the sky's light under a sun just below the horizon (`sky.rs`).
- **Tides:** none (the finite water has no tide); the main lunar tide's period, 12.42 h, is theirs
  when they come.

## Everything at its real rate (E §4.2)
With the two time scales gone, every rate in data is a real one and runs as long as it really
takes:

| What | Now | Source / where |
|---|---|---|
| A day, a year, a month | 86,400 s; 365.2422 days; 29.530589 days | the astronomy above |
| Walking, jogging, sprinting | 1.4, 3 and 6.5 m/s, a sprint lasting about 15 s | `movement.md`, `human.ron` |
| Stamina coming back | 1 − e^(−t / 100 s): half in about a minute, nearly all in five | phosphocreatine resynthesis (Harris et al. 1976; Bogdanis et al. 1995), `human.ron` |
| Falling asleep | after a quarter of an hour lying sleepy and at ease | sleep latency (Ohayon et al. 2004), `hearth_player::DROP_OFF_S` |
| A night's sleep | about eight hours after a long day (the two-process model) | 7–9 h (Hirshkowitz et al. 2015), `physiology.md` |
| Hunger, thirst, heat and cold | per real hour and day | `physiology.md` |
| Healing | a moderate sprain in about four days, a splinted fracture in about six weeks | `injuries.ron` |
| Illness | onset and course in real hours | `illnesses.ron` |
| Work in hand | the hours its process says, never hurried | `processes/*.ron` |
| Fires, cooking, drying, firing | per game minute, in real hours | `fire-and-food.md` |
| Food going off | `keeps_days` at 20 °C, 2.5 × faster per 10 °C warmer | `hearth_craft::food` |
| A fire in the vegetation | its neighbours catching at chances reckoned over 15 game seconds | `wildfire.rs`, `flora.md` |
| Weather | systems drifting with the latitude's wind over days, the daily cycle by the sun | `seasons.md` |
| Plants | phenology and succession through the real year | `flora.md` |
| Animals | active by the local hour, thirsty by the real day, breeding and dying by the real year; the populations' tier in steps of the real year | `fauna.md` |
| What was built | weathering a day at a time, rotting over real years | `building.md` |

The audit's surprises: the near fire's chances were per step of ten ticks, so with the day no
longer compressed a fire spread thirty times as fast in game time (its chances are now reckoned
over game seconds, whatever the step); the animals' thirst and their signs' ageing took a day
of 48 minutes (they take the real day); and stamina came back fully in 30 s (now as the muscles'
phosphocreatine does).

## Passing time: rest and sleep (E §4.3, Amendment P §7.1)
- The sleep key opens **Rest** (`docs/review/e3/rest.png`): *Sleep until morning*, *Sleep until
  rested*, *Rest for an hour*, *two hours* or *four hours*, *Rest until dusk*, and — when work
  left to itself lies within 12 m — *Wait until it is done* (named for the nearest: "Wait until
  it is done: Dry meat"). The sleep key again gets up.
- The body lies down. Sleep comes when it is sleepy and at ease (the two-process model,
  `physiology.md`), resting or not.
- The world goes faster, eased up to `sleep.max_factor` (100 times) most of the way in
  `sleep.ramp_s` (3 s), and back down as the rest ends (at once below twice as fast). It lives
  every tick: the body, the weather and the water's weather, fires, work left to itself, food
  going off, plants, the animals about the player (in steps of at most half a second, ten a
  tick).
- It ends when what it was for comes: the sun up, the body rested, the hours passed, the sun
  down, the work done; after a day at most. Anything that needs the player ends it sooner:
  asleep, the body wakes for the cold, heat, rain, pain, hunger or thirst; awake, it cannot rest
  for them; a hurt; an animal of 15 kg or more within 10 m, or one that has turned on the
  person within 40 m (`rest::animal_near`; the person's own kept animals apart).
- A line then says how it went: "The cold wakes you. You slept about three hours.", "Morning.
  You slept about seven hours.", "A wolf came near. You rested about half an hour."
- **Work in hand is never hurried.** The work warp (long work ran the world up to 20 times as
  fast, so no task waited more than a minute) is gone; lying down stops the work. E6 makes
  active work stroke by stroke.
- Creative's time speed (a world of no mode has it too) is the other way time goes faster. Tests
  use it to wait on work and for hours to pass (`tests/common`'s `HURRY`, 100 times).

## Time in words
- Without a clock: the part of the day by the sun and the season by its thirds, "Late
  afternoon, early autumn" (the pause menu, the Rest screen).
- Creative's exact clock: local mean time and the date, "Late afternoon, early autumn · 16:42 ·
  12 April 2026".
- The debug screen: the date, the season, the time by the sun and the local mean time.

## Saves
Format 7: `life.start` takes the place of the day's and the season's lengths, the starting season
and the axial tilt. A format-6 world is walked forward to a spring morning of the year it was
made, its ticks counted as real seconds from there.

## Consequences, stated honestly (E §4.4)
Some things now take real years: a tree grows over decades, an animal or a crop is domesticated
over generations, a child grows up over years. They are reachable in long-running multiplayer
worlds, with "the world goes on while I'm away" (E §4.3's optional catch-up, not built yet), and
in Creative with its time speed. The Field Guide (P §9.3) is to say so when it comes.

## Known simplifications
- The finite water's flow keeps its own pace (ten steps a second) when the world goes faster;
  its slow changes (evaporation, rain filling hollows) keep the world's.
- Faster than 100 times, the animals about the player lag the world's clock until a catch-up of
  more than a week passes to the populations' tier; their calls are heard at the lived pace.
- The ephemeris is good to an arcminute or so within a century or two of 2000.
- Rest is lying down; sitting to rest and wait comes with P4's poses.
- Sleeping until morning in a polar winter, or resting until dusk in a polar summer, ends after
  a day.

## Tests
- `hearth_env::astro`: dates and leap years; the tropical year and the 2024 equinoxes; the
  equation of time's course (−14.2 min on 11 February, +16.4 on 3 November); sunrise and sunset
  in London at both solstices, New York in December and Sydney in June within four minutes of
  published tables; polar night and the midnight sun at Tromsø; solar noon's drift; the moon new
  and full on its days (11 and 25 January 2024); the pole at the latitude and the sidereal turn.
- `hearth_env::calendar`: a real day and year; a spring morning in either hemisphere; now is
  now; the synodic month; a date by the sun's course and the hour.
- `rest::tests`: each rest ends when its end comes; waiting on work left nearby; what needs the
  player ends a rest; the world eased up and back.
- `hearth_save`: a format-6 world wakes to a spring morning (the migration and the fixture).
- `hearth_body` acceptance: a sprain heals in days and a fracture in weeks; `hearth_player`:
  sleep in a quarter of an hour, stamina back over minutes.
- `tests/wildfire.rs`: a fire in damp air goes nowhere (its spread reckoned in game seconds).

## Sources
- USNO, "Approximate Solar Coordinates" (the sun, the equation of time, GMST); the
  Astronomical Almanac's low-precision formulae for the moon.
- The equation of time's extremes: −14 min 15 s on 11 February, +16 min 25 s on 3 November
  (epoch 2000; EarthSky's yearly tables agree to a few seconds).
- Reference sunrises and sunsets: published tables for 2024 (London 03:43/20:21 UTC at the June
  solstice, 08:04/15:53 at the December one; New York and Sydney likewise); Tromsø's polar
  night, about 27 November to 15 January.
- Sleep: Hirshkowitz M. et al. 2015, "National Sleep Foundation's sleep time duration
  recommendations", *Sleep Health* 1: 40–43 (7–9 h for adults); Ohayon M. M. et al. 2004,
  *Sleep* 27: 1255–1273 (sleep latency).
- Stamina: Harris R. C. et al. 1976, *Pflügers Archiv* 367: 137–142; Bogdanis G. C. et al. 1995,
  *Journal of Physiology* 482: 467–480.
