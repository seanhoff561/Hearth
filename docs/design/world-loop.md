# The world loop: server and client

*Status: implemented in process (V2-3 part c, D68). Code: `crates/hearth/src/server.rs`,
`client.rs`, `crates/hearth_protocol`.*

## Purpose
A game rather than a camera: one authority that owns the world and simulates it at a steady
rate (v1 M4's integrated server), and a client that renders it and plays the player
responsively — ready for a network transport later without changing what either side does.

## Model
- **The server** (a thread, 20 ticks per second of play) owns the world: generation, light,
  the seasonal cover, the finite water, the clock and the player's body. Each tick it reads the
  client's reports of the player's movement (where they are, the hardest landing, immersion,
  breath), lives the body for the tick in the **exposure** where the player stands — the
  weather's air, humidity, wind and rain, the sun's beam on the body (its lux at 105 lm/W, 27 %
  of the body facing it, 70 % absorbed), the night sky's chill, shelter where a block overhead
  covers the head (no rain or sun, a third of the wind), the natural water's temperature when
  in it — moves the finite water (ten times a second; its slow changes every five game
  minutes) and sends the clock and the body's state (status, ability, injuries, illnesses,
  exposure). Between ticks it streams terrain around the player: generate, cover, light,
  mesh, and send each cube's blocks (for the client's collision) with its mesh; cubes that
  load again get their finite water back. It saves the clock and the player every five
  minutes and when it stops; a saved world keeps its seed and planet.
- **The client** (the render thread) mirrors the cubes it is sent, and moves the player
  itself every frame against that mirror with `hearth_physics` — no waiting for the server —
  reporting the movement twenty times a second. What the body allows (speeds, sprint, jump,
  climbing, breath) comes from the server's latest body state. The camera looks from the
  player's eyes (smoothed over steps up); F3+N switches to a free camera for development. The
  world map key opens the globe; a click asks the server to put the player there. The clock
  runs on between the server's messages.
- **Input**: WASD walk; the sprint key jogs, pressed twice quickly it sprints; Space jumps,
  climbs a ledge ahead or swims up; the sneak key crouches (and dives or climbs down); C
  crawls. After death Space lives on as a new person in the same region.
- **The first spawn** is warm-temperate lowland (D68), so a body in a loincloth lives through
  its first nights.
- **Saves**: `level.json` (clock), `player.json` (format 1: the body and the mover).

## Parameters
`TICK_S` 0.05 s, `REPORT_S` 0.05 s, autosave every 6,000 ticks, `BATCH` 192 cubes per
streaming step, the render distances from the options.

## Interactions
Body (`physiology.md`), movement (`movement.md`), the finite water (`geology.md`), the weather
(`seasons.md`), saves (`saves.md`), the globe (`rendering.md`).

## Known simplifications
- In process only: messages carry shared data (cubes, meshes, the generator); a network
  transport would serialise cubes and mesh on the client.
- The client is trusted with the player's movement (single player).
- The player wears a loincloth; clothing, carrying and actions come with V2-4 and V2-5.
- Sleep has no time acceleration yet and nothing to lie on; death rules beyond living on in the
  same region come with part f.
- Changed cubes are not yet written to region files (nothing but the finite water changes
  blocks so far, and its parcels live in memory).
