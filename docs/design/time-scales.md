# Units and the two time scales

*Status: implemented. Units and time scales (V2-0, `hearth_content::time`,
`data/hearth/units.ron`, `data/hearth/time.ron`); the calendar, sun and seasons (V2-1,
`hearth_env`, see `seasons.md`).*

## Units
SI everywhere inside the game: metres, kilograms, seconds, litres for fluids, °C, kcal for
food energy, MPa for strengths. Field names carry the unit (`density_kg_m3`, `size_m`,
`heal.hours`). `units.ron` lists display units per quantity (`display = si × factor + offset`);
conversion happens only in the UI.

## Two time scales (v2 §4.2)
- **Day scale** (body & action time): one game day stands for one real day. A real duration
  takes `duration × day_length / 24 h` of play. At the default 48-minute day, a 15-minute task
  takes 30 s, a night's sleep (8 h) 16 min.
- **Year scale** (life-cycle & calendar time): one game year stands for one real year. With
  8-day seasons (32-day years, 25.6 h of play), six weeks of bone healing take ≈3.7 game days.
  At 91-day seasons the two scales coincide.

Every process, injury, illness and growth rate in data declares its scale; all conversion goes
through `TimeScales` (`factor`, `play_seconds`, `game_days`, `ticks`).

## Parameters
`time.ron`: day length (20–120 min, default 48), days per season (3–91, default 8), axial tilt
(0–45°, default 23.44), starting season, real day/year lengths, synodic month, sleep
acceleration (up to 100× with a 3 s ramp).

## The calendar in play (V2-1)
`hearth_env::Calendar::from_config(time.ron)` turns world ticks (20 per second of play) into
days, the local solar time and the year fraction. Day-scale things (the sun's course, weather,
the body) run on the day; year-scale things (seasonal temperature, snowpack, ice, phenology)
read the year fraction, so a short game year still passes through every season in order. The
moon keeps its 12.37 months per year (D31).

## Interactions
Physiology (needs on the day scale, healing mostly year scale), processes (declared per
process), flora/fauna growth and phenology (year scale), weather (day scale), sleep
acceleration (both).

## Known simplifications
Injuries whose real healing exceeds about a week use the year scale; shorter ones the day
scale (so a mild sprain and a fracture heal in comparable game days — see D26).
