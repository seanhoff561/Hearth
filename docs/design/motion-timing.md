# Motion timing

*Status: the sky, clouds, water, rain and snow at real speeds (Amendment P P0, §8); the audit of
everything else that moves and the automated check that renders it come with P5.*

## Purpose
Everything that moves moves at its real speed in real seconds, unless the world is going faster
(a rest, Creative's time speed; P §8): a cloud at the wind's speed, a wave at its own phase
speed, rain at nine metres a second, and since E3 the sun, the moon and the stars too, the clock
being Earth's (`time.md`).

## The clocks
- **The world's seconds**: the world's ticks at 20 a second (`Environment::real_seconds`, exact
  in `f64`; `seconds`, the same wrapped at 100 000 s in `f32`, for the terrain shader's sway,
  caustics and animated textures). A second of play is a second of the world; when the world
  goes faster (a rest, Creative's time speed, the Observer) the ticks run faster and these
  visuals time-lapse with them.
- **The date** (`Moment`, from the same seconds): the sun and moon, the turning of the star
  field, the seasons' colours.
- **Frame time**: the eye's adaptation, the interface.

Motion that depends on a changing rate (the wind) is **summed** from frame to frame, never
computed as rate × the world's age: `wind × seconds` moves a thing by the whole age of the world
times every change of the wind, so a breeze picking up by a tenth of a metre a second threw the
clouds kilometres across the sky (the clouds' race, `dev/PLAYTEST.md` #7). The sum is carried in
`EnvSampler`'s `Motion`, each sample adding at most `MOTION_STEP_S` (2 s): a jump in time (a skip,
a sleep, a load) moves the clouds two seconds' worth, never across the sky in a frame, and a
fast-forward time-lapses them at most 120 times at 60 frames a second.

## What moves

| What | Clock | Speed | Where | Checked |
|---|---|---|---|---|
| Sun and moon | the date | where they really stand: the sun a turn a solar day (the equation of time with it), the moon some 50 minutes later each day | `hearth_env::astro` | E3 (`astro` tests) |
| The star field's turning | the date | a turn a sidereal day (23 h 56 min) | `environment.rs` (`star_rotation`) | E3 |
| Stars' scintillation | real (wrapped at 600 s) | two waves of 1–4 Hz; 1.5 % of a star's light overhead, 21.5 % at the horizon, halved in calm air (`turbulence` = wind / 12 m/s) | `sky.wgsl` (`stars`) | P0 |
| Stars' size | — | each star summed over the 27 cells about the pixel, so no cell's edge cuts it: a star crossing pixels as the sky turns keeps its light (over 0.8 s at 1080p, 2–5 % against 27–33 % before); drawn at its own crisp width, widened only where a pixel is coarser than a star | `sky.wgsl` (`stars`, `px`) | P0 (`docs/review/p0/`) |
| Clouds' drift | real, summed | the wind at the cloud base: the ten-metre wind × (base / 10 m)^(1/7), 1.8–2.1 × at 600–3,000 m above the ground | `environment.rs` (`Motion`, `wind_aloft`) | P0 (`clouds_drift_at_the_wind_aloft_in_real_seconds`) |
| The wind's direction, as clouds, waves and rain show it | real | follows the weather's with a two-minute time constant (`WIND_TURN_S`) | `environment.rs` | P0 |
| Waves: the swell (40 m tile) and the chop (11 m tile) | real | the deep-water phase speed of each scale's slope-weighted mean wavenumber, c = √(gλ/2π): 3.5 m/s for crests some 8 m apart, 1.8 m/s for 2 m; the wind sets their steepness, not their speed | `water.rs` (`phase_speeds`, `wave_phases`), `water.wgsl` | P0 (`waves_travel_at_their_phase_speeds`) |
| Rain | real | falls at 7.2–9.8 m/s, carried by the ten-metre wind (summed) | `precip.rs`, `precip.wgsl` | P0 |
| Snow | real | falls at 0.8–1.3 m/s, wobbling 0.4 m over 5–6 s, carried by the wind (summed) | `precip.rs`, `precip.wgsl` | P0 |
| Sunlight's caustics under water | real | two layers drifting at 0.25 and 0.2 m/s | `terrain.wgsl` (`caustic_light`) | P5 |
| Plants' and leaves' sway | real | periods of 2.2 and 3.7 s, the amplitude with the wind | `terrain.wgsl` (`wind`) | P5 |
| Animated textures | real | in ticks of 1/20 s | `terrain.wgsl` (`animated_layer`) | P5 |
| Smoke | real | puffs rising, swelling and leaning downwind over their life | `smoke.wgsl` | P5 |
| Heat shimmer (a fevered or overheated body) | real | waves of 0.3–0.6 Hz | `post.wgsl` | P5 |
| Bodies' gaits | real | stride from the body's speed (feet must not slide) | `hearth_character::animate` | P5 |

Not yet moving, for P5: cloud shapes do not evolve (they should, over minutes); fog does not
drift; there is no lightning, splash or waterfall to time; rivers' surfaces do not flow at their
current. When the world goes faster, the sky's visuals time-lapse at the capped rate above
(P §8 asks smooth blends and a capped visible rate of change, which P5 checks).

## Parameters
`MOTION_STEP_S` and `WIND_TURN_S` (`environment.rs`); the wave tiles `SWELL_TILE_M`,
`CHOP_TILE_M` (`water.rs`); the particles' clock `PERIOD_S` (`precip.rs`, 1,000 s, in which every
fall is a whole number of boxes and every wobble a whole number of turns, so it wraps unseen);
the stars' scintillation and width in `sky.wgsl` (a star's own width 0.22 of its cell, about half
a pixel at 1080p; at least 0.45 of a pixel where pixels are coarser, below about 1000 pixels of
height; 0.45 of a cell at most).

## Interactions
Weather (wind speed and direction, cloud base) — `seasons.md`; rendering — `rendering.md`; the
clock and the world going faster — `time.md`.

## Known simplifications
- Each wave scale slides as a whole at one speed: within a scale the shorter waves keep pace with
  the longer (the true field disperses).
- Rain and snow drift with the ten-metre wind at every height in the particles' box.
- The sky's motion is not saved: clouds start where the noise puts them each time a world loads.
