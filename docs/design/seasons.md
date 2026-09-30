# Calendar, seasons and weather

*Status: implemented (V2-1). Code: `crates/hearth_env` (calendar, astro, climate, weather,
phenology, tint, sky), `crates/hearth/src/season_cover.rs` (snow and ice on terrain),
`crates/hearth/src/environment.rs` (what the renderer gets). Parameters: `data/hearth/time.ron`.
Decisions: D29–D37.*

## Purpose
Seasons drive plants, animals, weather, fire and the player's needs exactly as axial tilt does
on Earth (v2 §4). V2-1 provides the clock, the sky, the seasonal climate and weather, snow and
ice, and the generic phenology that later systems (flora V2-6, fauna V2-7, physiology V2-3)
build on.

## Model

### Calendar (`calendar.rs`)
- The world clock counts ticks (20 per second of play). A game day lasts `day_length_min`
  (20–120 real minutes, default 48); a year is four seasons of `days_per_season` days
  (3–91, default 8). Both come from `time.ron` (`Calendar::from_config`).
- Year fraction 0 is the March equinox (start of northern spring); seasons are the
  astronomical quarters and are opposite in the southern hemisphere. A world can start in any
  season at any local time (`start_at`).
- The global time of day is 0 at midnight at X = 0; local solar time adds the planet's solar
  offset for the longitude (`Planet::solar_time_offset`), so it is noon on one side of the
  planet while it is midnight on the other.
- The moon's synodic month keeps its ratio to the year (12.37 months per year), so a short game
  year still has a full cycle of phases every ~2.6 game days at the defaults (D31).

### Sky positions (`astro.rs`)
- Solar declination δ = asin(sin ε · sin 2π·yf) for a circular orbit (no equation of time);
  sun direction from latitude, declination and hour angle (world +X east, −Z north, +Y up).
- Day length with the standard −0.833° altitude (refraction and the solar disc); polar day
  and night fall out of the same formula. Tests against known values: 12.1 h at the equator,
  18.8 h and 5.9 h at 60° at the solstices, the celestial pole at the latitude's elevation.
- Moon: position from its phase (opposite the sun when full) on the ecliptic; stars rotate
  about the celestial pole with the sidereal day.

### Seasonal climate (`climate.rs`)
- The planet model gives per place the annual mean temperature (at the place's elevation, via
  the lapse rate), the annual range, annual precipitation and winter/summer dry-season
  strengths. `Normals` turn these into a year: a cosine temperature wave peaking about a
  month after the solstice over land (more at sea); a diurnal cycle whose range grows with
  aridity and continentality; precipitation seasonality with winter-dry (savanna, monsoon),
  summer-dry (mediterranean), continental summer maxima and the equatorial double rainy season.
- Growing degree days (base 5 °C) and a permafrost flag (mean < −2 °C) for later systems.

### Snow and ice over the year (`SeasonalCover`)
A year-scale integration in 5-day steps from the normals, spun up four years so it is
periodic: precipitation falls as snow below −0.5 °C and as rain above 2 °C, the pack melts at
3.5 mm per degree-day, settled snow is 280 kg/m³; still-water ice grows by Stefan's law
(2.4 cm/√(°C·day)) and thaws at 1.2 cm per degree-day. Packs that never melt mark perennial
snow (glaciers are the generator's). Rivers freeze to about half the still-water thickness.

### Weather (`weather.rs`) — two layers (D30)
- **Day scale, what the player sees:** a smooth field on the sphere (fBm) advected by the
  prevailing wind of the latitude band, plus convective cells in warm humid climates. The wet
  fraction is matched to the climate's precipitation rate for the date (an inverse normal CDF
  threshold), so wet seasons really are wetter. Out of it: cloud cover, precipitation rate and
  type (rain, sleet, snow by temperature), thunder chance, humidity, wind, the day's
  temperature with a synoptic anomaly.
- **Year scale, what accumulates:** snowpack and ice come from the normals (above), not from the
  weather the player happened to see, so they are right at any calendar speed and don't
  depend on where the player was.

### Phenology plumbing (`phenology.rs`)
Generic plant types until species calendars arrive (V2-6): deciduous broadleaf trees leaf out as
spring passes ~8 °C, turn as autumn falls through 13 °C and drop leaves around 5 °C; in
winter-dry climates they keep only a third of their leaves in the dry season (mediterranean
trees keep theirs); grass greens above ~5 °C and cures in dry seasons and deserts; conifers dull
a little in hard winters.

### Seasonal colours on the GPU (`tint.rs`, `terrain.wgsl`)
Meshes store a 24-bit climate code per quad (mean temperature, range, precipitation, dry-season
type, hemisphere) instead of a colour; the terrain shader evaluates the same phenology for the
current date. Deciduous leaves thin out with their leaf cover (a stable per-texel pattern in
world space) and bare crowns show a sparse net of twigs. Changing the season never remeshes.
The dry-season type comes from the dry-season strengths, the same test the phenology and
weather use (D35).

### Snow and ice on terrain (`season_cover.rs`)
When terrain loads, the date's snow depth (8 layers = 1 m, with a little drifting) and ice are
laid on it: snow lies on full blocks and buries low plants; bare deciduous crowns let it
through to the ground (three quarters of it); conifer crowns hold two layers and shelter the
ground. Still water and rivers freeze; the sea stays open until sea ice arrives with the coasts
(V2-2). Every five days of the year the loaded terrain is refreshed: the old cover comes off
(buried plants return, ice thaws) and the date's is laid again; only changed blocks are relit
and remeshed (D36).

## Parameters
`time.ron`: day length, days per season, axial tilt, starting season, synodic month. Snow and
ice constants are in `climate.rs` (melt factor, density, Stefan coefficient); weather tuning in
`weather.rs`.

## Interactions
Temperature and weather → physiology (V2-3: cold, heat, wet clothing), fire (V2-5); phenology
→ flora (V2-6) and the fauna that eat it (V2-7); snow and ice → movement and building (V2-3,
V2-8); daylight → vision and the light level mobs and plants see.

## Known simplifications
- Circular orbit: no equation of time or eccentricity; seasons are equal quarters.
- The seasonal wave is a single cosine; sudden spring warmings and Indian summers come only
  from the day-scale weather.
- Snow and ice are block states laid per column, not a simulated pack: no drifts against
  obstacles, no avalanche, no snow on steep faces; snow on the leaves of evergreen species other
  than conifers follows the ground rules.
- Weather rendering covers clouds, rain and snow; lightning flashes, thunder, fog banks and
  wet surfaces are not drawn yet.

## Acceptance (V2-1)
- `tools/shots/v21_seasons.shots`: four seasons at 62°N (cold subpolar coast), 42°N (mixed
  forest) and 12°N (winter-dry tropics), a southern winter, sunset, dusk, night, rain and
  snowfall (`hearth --screenshot-list tools/shots/v21_seasons.shots`, 18 shots, ~14 s).
- Solar position and day-length tests (`astro.rs`), calendar tests, climate and weather tests.
- `crates/hearth_env/tests/headless_year.rs`: a year on a generated planet — snow and lake ice
  in the cold months and never in the tropics, by hemisphere; wet and dry seasons at the right
  times in savanna and mediterranean climates.
- `crates/hearth/tests/season_refresh.rs`: winter covers a summer landscape and the next
  summer restores it block for block.
