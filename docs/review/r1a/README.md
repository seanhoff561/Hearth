# R1a review pictures: the sun's shadows (D298)

Rendered on the cloud machine's software device (`hearth --software`); how they look on a real
GPU, and what they cost there, is the owner's check (`hearth bench --shadows off` against the
default on the RTX 4060).

- `off_on.jpg` (`tools/shots/r1a.shots`; shadows off left, on right):
  - a person in the 8 o'clock sun on a dry plain: their shadow cast long across the ground,
    the shrubs' and stones' beside it;
  - a ridge at dawn (sun 5° up), 40 m above it: the near ground shadowed by its own
    bumps, and thin far-cascade shadows out to some kilometres.

  The same file's low hill at dawn (400 m up) is left out: its land is too gentle for a sun 5° up
  to cast beyond each slope's own shading. That is G1a's gap, not the maps': the two renders
  differ by 0.02 %.
- `low_sun_before_after.jpg`: the realism suite's low-sun shots, T2 (left) beside R1a (right).
  Rows:
  1. a temperate rainforest at eye height;
  2. a broadleaf forest from a rise;
  3. the boreal floor underfoot;
  4. a savanna at eye height;
  5. an alpine meadow from a rise;
  6. a tropical rainforest from a hill.

  Before, a low sun lit the world as noon does. Now trees, grasses, tussocks and stones cast
  long shadows, and the floor under a canopy is dappled. From the hill the crowns' shadows are a
  texel or two and barely show (the distant crowns are boxes that do not cast, D298).
- `edges_underfoot.jpg`: the boreal floor at 7 h from eye height, looking down. Left to right:
  - shadows off;
  - an earlier build's edges, hard and stepped at the texels;
  - the final edges, as soft as the sun's disc makes them at each distance from the caster:
    sharp at a stem's foot, blurred where a crown's shadow falls far below it.
- `../realism/suite/*.jpg`: the whole suite again with the shadows (day | low sun, by scale).

Measured (`bench realism images`, `docs/review/realism/measures/images.md`; medians over the
seventeen biomes, T2 → R1a):

| Measure | T2 | R1a | Earth's |
|---|---|---|---|
| Spectral α underfoot | 1.89 | 2.26 | 1.8–2.4 |
| Spectral α at eye height | 1.23 | 1.32 | 1.8–2.4 |
| Spectral α from a rise | 1.02 | 1.06 | — |
| Spectral α from the hill | 1.02 | 1.03 | — |
| Colourfulness at eye height | 60.5 | 53.5 | 15–60 |
| Low sun's luminance p5, underfoot | 0.34 | 0.14 | — |
| Low sun's luminance p5, at eye height | 0.12 | 0.07 | — |

Broad shade now breaks up the pixel-fine texture. The dark tones a low sun leaves are where the
shadows fall.
