# Sound

*Status: implemented (V2-3 part d, D69). Code: `crates/hearth_audio` (the engine),
`crates/hearth/src/hearing.rs` (what the player hears), `app.rs` (the output and the options).*

## Purpose
The world is heard as well as seen. Wind tells a storm before it is seen, rain drums on a roof,
footsteps say what the ground is, and a pounding heart or ragged breath says what the body is
going through without a number on screen.

## Model
- **Every sound is made as it plays**, from three parts: tones that glide and fade (thumps,
  knocks, rings, bubbles), filtered noise with an attack and a fall (swishes, hisses, splashes),
  and noise in random short grains (crunching snow and gravel, crackling leaves, the patter of
  drops). Each sound is a short recipe of these parts, varied a little in pitch (±8 %) and
  loudness each time, so no two footsteps are the same and nothing is recorded. Noise is
  normalised by its filter's share of the band, so a recipe's levels are RMS whatever the
  filter.
- **Sounds once**: footsteps on twelve surfaces (grass, soil, mud, sand, gravel, stone, wood,
  snow, ice, moss, shallow water, leaves) at a force from creeping to sprinting; landings by
  impact speed; splashes by entry speed; swimming strokes above and under water; a gasp; a
  blow, and a crack for a broken bone; the interface's click.
- **Beds** ease toward what the world says (`Ambience`, sent twenty times a second):
  - **wind**: low-passed noise rising in level (as speed^1.5) and brightness with speed,
    gusting every one to four seconds, with a whistle above 9 m/s;
  - **rain**: a hiss, a patter and larger drops, louder as the square root of the rate. Under
    a roof it is lower and duller, drumming on the roof; under rock or earth (six blocks or
    more overhead) the weather is far away. Snow falls silently;
  - **under water**: a low hush, with the world muffled (a low pass easing to 450 Hz);
  - **the heart**: "lub" and "dub" a systole apart (a third of a second at rest, shorter as it
    quickens), at the rate the body sets;
  - **the breath**: in, out and a pause, trembling when shivering.
- **The mixer** sums sounds into buses that carry the options' volume categories (weather,
  blocks, players, ambient, hostile, friendly, interface, music) and a master. The world's
  sounds (not the interface's) are muffled under water, echo when shut in (a reverberator of
  four damped feedback delays and two all-passes a side, wetter and longer as the place
  closes in), and fade out while the game is paused. The heart, the breath, gasps and hurts
  are heard inside the head: not muffled, no echo. A limiter keeps the sum under full scale.
  Up to 48 sounds play at once; a new one past that ends the oldest.
- **The output** is the system's default device or the one the options name (cpal), at its
  own rate and sample format, the mixer running in its callback and taking commands over a
  channel. A device that fails (unplugged) is opened again within two seconds.

## What the player hears (`hearing.rs`)
- **Footsteps** come one per stride: 0.75 m walking, 1.15 m jogging, 1.6 m sprinting, 0.55 m
  crouching, 0.45 m crawling, 0.7 m wading, a rung every 0.55 m climbing. Feet alternate a
  little left and right. The surface is a thin layer at the feet (snow, moss, a slab), plants
  walked through (rustling; loose stones clatter like gravel), or the block below, looking
  under the corners of the feet at an edge. Water to the shins splashes.
- Blocks name their **sound group** (`BlockDef::sound`). Natural blocks take theirs from
  their material where it says more than their template: sands hiss, clays and muds squelch,
  gravels and loose stones crunch, organic earths are soft, frozen ground is hard.
- **Landings** above 1.5 m/s thud by impact. **Splashes** come on entering the water, by the
  speed of entry. **Strokes** come every 1.1 m swum (treading water strokes too).
- **The body**: a new injury is heard (with a crack for a fracture). Coming up after holding
  the breath more than 5 s brings a gasp. Cold water below 15 °C brings the cold shock: a gasp,
  then fast breathing for a minute or so. The heart runs at 62 bpm plus 115 for exertion (work
  by gait, or the stamina spent), 120 × the share of blood lost, 10 per °C of fever, 25 for
  pain and 30 for the cold shock, and slows by 8 per °C of core below 35 °C. It is heard above
  125 bpm, or in danger: blood lost, a long breath held, deep cold. Breathing is heard with
  hard work, the cold shock and shivering, but not under water or asleep.
- **The surroundings**: the weather's wind and rain where the player stands; shelter when a
  block overhead covers the eyes; buried by the solid blocks between the eyes and the sky;
  shut in by how many of nine looks (up, the four sides, and up between them) meet a solid
  block within 16 m.
- **Captions** (the options' "Captions") name each sound in the lowest right corner for three
  seconds after it is heard, fading: footsteps, landings, splashes, swimming, gasps, hurts, a
  pounding heart, heavy breathing, wind blowing or howling, rain falling or on the roof.

## Parameters
Recipes, levels and rates are in `sounds.rs` and `beds.rs`; the body's rhythms are in
`hearing.rs`. Typical peaks at full volume: a walking step −28 to −21 dBFS, a landing at
8 m/s −12, a splash at 6 m/s −13, the click −18. A breeze of 5 m/s is −36 dBFS RMS, a gale of
15 m/s −21, 28 m/s −13; heavy rain (12 mm/h) −26.

## Interactions
- Weather (`hearth_env`): the wind and the rain where the player stands.
- Movement (`hearth_physics`): the gait, speed, landings, immersion and breath.
- The body (`hearth_body`, through the server's `BodyView`): stamina, blood, core
  temperature, pain, shivering, injuries, the water's temperature.
- Blocks (`hearth_world`): sound groups, solidity for shelter and the echo.
- Options: volumes per category, the output device, captions.

## Known simplifications
- Every sound is the player's own or around them: nothing is placed in the world yet, so
  nothing comes from a direction or falls off with distance (the mixer pans, but nothing
  aims yet).
- The echo has one character (a medium room), only wetter and longer as the place closes in;
  the size of a cave is not measured.
- No music, thunder, fire, flowing water, animals or the sounds of work; their volume
  sliders are hidden until they have sounds.
- Captions do not say which way a sound came from.

## Future extensions
- Sounds placed in the world, with distance, direction and occlusion (with fauna, V2-7, and
  the work of the hands, V2-4/V2-5).
- Thunder with lightning (V2-16); fire crackle (fire, V2-5); rivers, waves and dripping caves.
- Music (V2-15 or later).
