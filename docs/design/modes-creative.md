# Game modes and Creative

*Status: implemented (Amendment P P2, as amended by Amendment E §9.1: no people in Creative).*

## Purpose
Three modes in place of the many world-rule settings, and Creative's powers for building,
exploring and testing (Amendment P §2–3).

## Model
- **Modes** (`data/hearth/balance/modes.ron`, `modes.rs`): Realistic, Easy and Creative, each
  setting the realism preset, the knowledge mode, what death means, the predators' ways, hints,
  the clock, watching and Creative's powers. A new world's `WorldSpec.mode` is applied over its
  life settings (`server::new_life`) and kept in `WorldSettings.mode`; a world of no mode (tests
  and tools, worlds from before) keeps the rules as set, everything open. A world's Edit (the
  Worlds screen's Mode) changes it only toward less strict, by the modes' `strictness`
  (`worlds::set_mode`); to Creative marks it `played_in_creative` for good.
- **What the server refuses** outside Creative (or a world of no mode): watching from an eye
  (`Observe`), being put somewhere (`Place`), moving time (`SkipHours`, a `TimeWarp` above
  zero), holding the weather, Creative's acts and its remove tool. The client hides the same
  (Watch, Spectate on death, the debug time keys, the free camera).
- **F3:** in Realistic and Easy only how the game performs (fps, the terrain's work); in Creative,
  in a world of no mode or with Developer mode (Options) everything.
- **Creative's body** (`server::creative_body`): injuries and illness cleared every tick,
  stamina full, the body made whole whenever worn and every 40 ticks; no harm from landing or
  drowning.
- **Flight and no-clip** (client): Jump twice quickly takes off or lands; the wheel sets the speed,
  Sprint triples it, Jump rises, Crouch descends; F7 passes through the ground
  (`hearth_physics::fly`).
- **The inventory** (`creative.rs`, `creative_ui.rs`, the inventory key in Creative; its Carried
  button the things carried): Terrain (natural blocks), Plants, Animals, Items (every form and
  material of the item registry), Building (each piece in each material, the workstations),
  Knowledge; searched by name. Its acts (`ToServer::Creative`) at what is looked at or just
  before the player (`workshop/creative.rs`): take into the hands, place a block, plant (a crop
  sown or ripe, an understory plant, a tree young or grown from its template), summon 1–16
  animals of a sex and age, build a piece or a station finished, learn or forget a node.
  A thing is put where a body could walk through (a plant gives way), never where the player
  stands. **Instant** (on by default): work is done as soon as begun.
- **Pick** (the pick key in Creative) opens the inventory at what is looked at; **Remove**
  (Delete) takes away what is looked at: a thing lying, an animal, a block (a standing tree
  whole, both halves of a tall plant).
- **Time and weather** (pause menu): go on to an hour, time stopped to ×600, a weather held
  (clear, cloudy, rain, downpour, snow), the wind and the air's temperature.
- **Clear view** (F4; its parts on the pause menu): daylight at any hour, no haze, no clouds or
  precipitation, no adapting eye or body's senses (`clear_view.rs`, a filter on the frame's
  environment). Caves stay dark (their light is the mesh's).
- **Spectate** (F6): the Observer's free eye, the world streamed about it as about a player; F6
  again resumes there (`Place` sets the body on the ground below), Esc returns to the body.

## Keys (Amendment P §13)
F4 clear view, F6 spectate, F7 no-clip, Delete remove — none held by an earlier action. Pick is
the pick key (middle click) in Creative until P3 moves it to Ctrl + middle click with the action
menu.

## Tests
`tests/creative.rs` (Realistic and Easy refuse watching, time and Creative's acts; Creative is
unhurt and its inventory takes, places, plants, builds, summons and teaches; the remove tool;
spectating far off streams the world and resuming stands the body there), `modes::tests`,
`worlds::tests` (mode changes), `clear_view::tests`, `creative::tests`, `tests/layout.rs`.

## Known
Brushes for terrain and plants (Amendment S §8.3, P §3.1) come with S1 and P7G; favourites and
item quality in the inventory, and outlines in clear view, are not built.
