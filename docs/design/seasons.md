# Calendar, seasons and weather

*Status: planned (V2-1). Parameters already in `data/hearth/time.ron`.*

## Purpose
Seasons drive plants, animals, weather, fire and the player's needs, exactly as axial tilt does
on Earth (v2 §4).

## Planned model
- Calendar from day length and days per season; year = 4 seasons; the world clock counts ticks.
- Solar declination from the day of year and axial tilt; sun position and day length by
  latitude (Mercator latitude from the planet model); polar day and night; opposite seasons in
  the two hemispheres.
- Seasonal climate: monthly temperature from the annual mean and a continentality-dependent
  amplitude; the rain belts follow the sun (wet/dry tropics, monsoons).
- Weather cells following the seasonal climate; snow cover accumulating and melting as
  layers; lake and river ice growing with freezing degree-days; permafrost flags.
- Phenology state per species (leaf-out, flowering, fruiting, leaf fall) from their calendars.
- Seasonal grass/foliage colours as GPU parameters (no remeshing).

## Acceptance (V2-1)
Screenshots at 4 seasons × 3 latitudes; solar position and day length tests; a headless year
with correct snow, ice and wet/dry timing by latitude.
