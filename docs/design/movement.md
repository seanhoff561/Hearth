# Movement

*Status: implemented (V2-3 part b, `hearth_physics`, `hearth_player`, D67); driven from input
by the client every frame (V2-3 part c, `world-loop.md`). Since S3 natural ground is collided
as the smooth ground's field, with slopes walked, slid and paced as `smooth-terrain.md` says
(D271).*

## Purpose
How a body moves through the block world (v2 §9.2): human speeds, realistic jumps, climbing
instead of leaping, crawling into low spaces, wading and swimming with breath held, and falls
that injure by their speed rather than by a damage formula. Animals will use the same code.

## Model
`hearth_physics::step(world, mover, intent, ability, seconds)`, in substeps of at most 1/60 s.

- **The box.** 0.5 m wide; 1.75 m standing (eyes at 1.62), 1.3 crouching, 0.6 crawling or
  swimming. It is swept against the blocks' collision shapes one axis at a time (Y, X, Z), so it
  slides along walls and never passes through a floor however fast it falls. Unloaded terrain
  is solid. A taller stance is taken only where it fits (no standing up under a low ceiling).
- **Gaits.** Walk 1.4 m/s, jog 3, sprint 6.5 (while the body allows), crouch 0.8, crawl 0.4,
  swim 0.8; speed comes within a fraction of a second on firm ground (ice keeps the body
  sliding), barely changes in the air, and wading slows with the depth.
- **Steps, scrambles, ledges.** Up to 0.6 m is a step in stride; up to a block (1.05 m) is
  scrambled up when walking or jogging — slower for a moment and hard work; ledges up to head
  height (1.9 m above the feet) are climbed by jumping into them with both hands free and the
  strength and stamina for it, taking about one to two seconds. Higher walls are out of reach
  (rock faces with holds come later).
- **Jumps.** About 0.45 m standing, scaled by the body's strength and the ground underfoot;
  none with a hurt leg.
- **Edges.** Crouching keeps the feet on an edge; walking off it falls.
- **Water.** The water over the body's column decides: wading below the chest, swimming above
  it with no ground underfoot (until the feet find shallower ground). A swimmer floats with the
  eyes just out of the water, swims up and dives on command, and an exhausted or unconscious one
  sinks. The breath lasts 45 s (less for a weak body) and comes back three times as fast;
  without air a body faints after 25 s and drowns after 60 (`hearth_player`).
- **Plants.** Pushing through plants and foliage slows a body by how much of it they reach
  (Amendment P §10.1): each column of plants the box overlaps slows it by the plants' density
  (`drag` in the block data: stems and leaves, how stiff and close) times the square of how
  high up the body they stand, as a share of its height, weighted by the share of the box's
  footprint the column takes. Grass, herbs and seedlings below the knee cost a sprint at most
  2 %; knee- to waist-high grass a few percent; a shrub belt the body pushes through 20–50 %; a
  thicket over the head its full density. A plant stands as tall as its species grows
  (`max_height_m` in the flora data, by the understory block it is drawn as), else as its
  block's outline. A crouching body, shorter, is slowed more by the same grass. Plants bent and
  flattened by passing bodies, and grass as a living sward, come with P7 and P7G.
- **Ladders.** Climbable blocks are climbed at 0.6 m/s by moving into them.
- **Landings.** The speed of every landing is reported, softened by the ground's cushion (snow,
  leaves) or by water, which passes on about a third of the speed of entering it.
  `hearth_body::Body::land` turns it into injuries: nothing below 5.5 m/s (a 1.5 m drop);
  sprains and bruises; a fracture in a fifth of falls from 3 m and most from 6 m; deep wounds
  past 11 m/s; death in a fifth of falls from 10 m, half from about 12 m and all past 22 m.
- **The player** (`hearth_player::Player`): the body's effects become the mover's ability
  (speeds × the body's walking factor, no running on a sprained or broken leg, sprint while
  stamina lasts, no climbing with a broken arm, a shorter breath when weak), and the motion
  becomes the body's activity (METs, effort, posture, speed through the air); immersion reaches
  the heat balance; a body that cannot act goes limp.

## Parameters
Speeds in `body/human.ron`; the box, steps, reach, drag and breath in `hearth_physics`; block
friction, speed and jump factors and `cushion` in the block data.

## Interactions
Body (effects, stamina, landings, immersion, METs) — `physiology.md`; blocks (collision
shapes, water, friction, cushion, climbable); later the load carried (V2-4), trees and branches
to climb (V2-6), animals (V2-7), the world loop and input (V2-3c).

## Known simplifications
- No rock-face climbing with holds yet; no vaulting, no mantling onto moving things.
- Swimming has no strokes or currents; rivers do not push swimmers.
- One box for every stance's width; a swimmer is a cube, not a horizontal body.
- Falls land on the feet: no head-first falls, no rolling to soften a landing.
