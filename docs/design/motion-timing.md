# Motion timing

*Status: implemented (Amendment P §8: P0 for the sky, clouds, water, rain and snow; P5 for the
audit of everything else that moves, the clouds' change of shape, feet that do not slide and the
automated check).*

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
| Stars' size | — | each star summed over the 27 cells about the pixel, so no cell's edge cuts it: a star crossing pixels as the sky turns keeps its light (over 0.8 s at 1080p, 2–5 % against 27–33 % before); drawn at its own crisp width, widened only where a pixel is coarser than a star | `sky.wgsl` (`stars`) | P0 (`docs/review/p0/`) |
| Clouds' drift | real, summed | the wind at the cloud base: the ten-metre wind × (base / 10 m)^(1/7), 1.8–2.1 × at 600–3,000 m above the ground | `environment.rs` (`Motion`, `wind_aloft`), `sky.wgsl` (`clouds`) | P0 (`clouds_drift_at_the_wind_aloft_in_real_seconds`) |
| Clouds' change of shape | real, summed | the smaller shapes (some 300 m) slide across the larger at 0.5 m/s, the larger a quarter as fast the other way: a cloud is another in some ten minutes (a cumulus lives ten to twenty) | `environment.rs` (`CLOUD_CHURN_M_S`), `sky.wgsl` (`clouds`) | P5 (`the_sky_time_lapses_smoothly_when_time_runs_fast`) |
| The wind's direction, as clouds, waves and rain show it | real | follows the weather's with a two-minute time constant (`WIND_TURN_S`) | `environment.rs` | P0 |
| Waves: the swell (40 m tile) and the chop (11 m tile) | real, summed | the deep-water phase speed of each scale's slope-weighted mean wavenumber, c = √(gλ/2π): 3.5 m/s for crests some 8 m apart, 1.8 m/s for 2 m; the wind sets their steepness, not their speed | `water.rs` (`phase_speeds`, `wave_phases`), `water.wgsl` (`wave_slope`, `wave_slope_far`) | P0 (`waves_travel_at_their_phase_speeds`) |
| The sun's glitter on water | with the waves | moves as the waves' slopes do | `water.wgsl` (`wave_slope`) | P0 |
| Rain | real | falls at 7.2–9.8 m/s (8.5 × 0.85–1.15), carried by the ten-metre wind (summed) | `precip.rs`, `precip.wgsl` (`vs_main`, `whole_speed`, `whole_turns`) | P5 (`motion_timing`) |
| Snow | real | falls at 0.8–1.3 m/s, wobbling 0.4 m over 5–6 s, carried by the wind (summed) | `precip.wgsl` (`vs_main`) | P5 (`motion_timing`) |
| Sunlight's caustics under water | real | two layers drifting at 0.25 and 0.20 m/s | `terrain.wgsl` (`caustic_light`) | P5 (`motion_timing`) |
| Plants' and leaves' sway | real | two waves, periods of 3.7 and 2.2 s, the amplitude with the wind | `common.wgsl` (`wind`) | P5 (`motion_timing`) |
| Animated textures | real | in ticks of 1/20 s, each texture's frames at its own frame time | `common.wgsl` (`animated_layer`) | P5 (`motion_timing`) |
| Smoke | real | a puff lives 40 s by a fire (300 s for a far plume), rising first at 2.1–3.5 m/s and slowing, swelling, carried off at the wind's speed by the end of its life | `smoke.wgsl` (`vs_main`) | P5 (`motion_timing`) |
| Heat shimmer (a fevered or overheated body) | real | two waves of 0.3–0.6 Hz | `post.wgsl` (`graded`) | P5 (`motion_timing`) |
| The eye's highlight meter | frame time | adapts with a time constant of 0.67 s | `meter.wgsl` (`meter_main`) | P5 (`motion_timing`) |
| People's gaits | real | a cycle every 1.5 m walking (1.4 m/s: 112 steps a minute), 2.3 m jogging, 3.2 m sprinting; a foot down goes back under the body at the ground's pace while flat, then rolls over its ball | `hearth_character::animate` (`stride`, `stance_hip`) | P5 (`feet_do_not_slide_and_work_keeps_its_tempo`: under 0.04, 0.09 and 0.19 m/s of slip; `docs/review/p5/gaits.png`) |
| Work's strokes | real | a stroke every `stroke_s` of the process's work model (0.6–6 s) | `hearth_character::animate` (`work_pose`) | P5 (the same test) |
| Blows | real | drawn back, struck, recovered: a punch 0.15, 0.10, 0.25 s; a swing 0.35, 0.15, 0.45 s; heavier things up to 1.6 × slower | `hearth_player::strike` (`timing`) | E2 |
| Animals' gaits | real | walking, trotting, galloping by the Froude number; stride after Alexander (2.3 h Fr^0.3); a foot down for its share of the stride goes back at the ground's pace | `hearth_fauna::anim` (`stride_of`, `foot_in_stride`) | P5 (`planted_feet_keep_pace_with_the_ground`) |
| Animals' heads, ears, tails, breath | real | eased toward what the animal does (rates 1–8 /s); ears flick every few seconds | `hearth_fauna::anim` (`Motion::update`) | V2-7 |
| Heartbeat and breath heard and felt | real | the body's own rates (`heart_bpm`, `breaths_per_min`) | `client.rs` | H-series |

**Not moving yet:**
- **Fog** is the air's haze, even everywhere, with no shape to drift; banks and patches of fog
  come with weather that makes them.
- **Rivers** do not flow: their surfaces carry the wind's waves, not the current (PLAN, from P5).
- There is no **lightning**, splash or waterfall to time yet.
- **Fire's flicker** is the firelight's level, steady; flames come with the fire's own model.

## Fast-forward
When the world goes faster (a rest, sleep, Creative's time speed, the Observer), the visuals on
the world's seconds time-lapse with it, smoothly:
- The sky's sums (the clouds' drift and change of shape, the wind's turn) take at most
  `MOTION_STEP_S` (2 s) a frame: at a thousand times play's speed the clouds move two seconds'
  worth a frame, some 120 times faster than at play, never across the sky in a frame.
- The sun, moon and stars move as the date does, a little each frame.
- The waves, rain and snow, sway and smoke run on the world's seconds and blur into a
  time-lapse.

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
