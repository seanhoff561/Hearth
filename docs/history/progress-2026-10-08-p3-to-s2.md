# Progress, P3 to S2 (2026-10-08)

*Moved from `PROGRESS.md` at Audit 2.*

## S2 — the smooth ground drawn (2026-10-08, D263–D265)
- **Meshes:** the server meshes each cube's natural ground through its fill with sharp nets.
  - Each cube has two voxels of apron, so neighbouring cubes meet without a crack or a fold (27
    generated cubes checked).
  - Vertices are 24 bytes, about 25 bytes a triangle, with light, ambient occlusion from the
    fill, and the column's climate for grass.
  - The blocks no longer draw natural ground.
- **Shading:** each of the 105 ground materials is 3D noise in world space at its grain, in its
  material's colours, so nothing tiles.
  - Materials meet by height blending, and bedded rock shows its beds.
  - Crisp rock shades toward its faces.
  - Grass takes its place's and season's colour at a sward's albedo, with thinner places where
    the soil shows.
  - Wet ground darkens after rain and dries by the weather.
  - Snow lying on the ground covers it (snow layers there are no longer drawn as blocks).
- **In play:** the look meets the smooth surface (12 µs a pick). Smooth meshes are drawn from
  their own arenas, first in the GPU-culled path.
- **Measured:** a surface cube meshes in 0.28–0.87 ms on one thread, as before. Mesh memory falls
  in the mountains and grows on coasts. Shots are in `docs/review/s2/`.
- **Deferred** (`PLAN.md`):
  - occlusion culling of the smooth ground;
  - `meshopt`;
  - mid-range simplification;
  - normals, specular and further overlays from the materials;
  - debug views;
  - frame times on the PC.
- **Real?** Albedos from the materials' measured colours; wet ground at 0.6 of dry; fresh snow at
  0.8; grass at a sward's half of a blade's colour.
- **Lean?** No texture sets: the 105 materials are a 48-byte record each and a few lines of
  noise. One mesher serves the server and the tests.
- **Fast?** Meshing costs as the blocks did; a pick costs 12 µs.
- **Whole?** The look, the dig, the saves and the mesh read the same fill, and GPU and CPU
  culling draw the same image.
- **Organic?** Sand runs into turf by height, and beds waver across a face. Nothing repeats.

## S1 — fill data and editing (2026-10-08, D260–D262)
- **Fill in the world:**
  - every cube the ground's surface passes through keeps each voxel's depth inside it, to
    1.2 cm;
  - generated from the terrain's continuous height across its slope, the cliffs' shaping and
    the caves' walls;
  - regenerated the same from the seed, the surface within 6 cm of the terrain's height;
  - written with the cube to the client and to disk.
  Blocks say what they are to it (natural, structure, fluid, foliage, empty).
- **Ground families** (`materials/reference.ron`, linted for every natural block): sharpness,
  angle of repose dry and wet, friction dry, wet and iced, and footsteps, with sources. Sand
  rests at 34°, gravel at 40°, loose earth at 37°; wet clay at 20°.
- **Editing** (`hearth_world::ground`):
  - a look meets the ground to the millimetre;
  - digging takes a volume as a bowl, in whole steps of the fill;
  - piling fills the lowest voxels, burying low plants;
  - loose ground slumps to its repose.
  Volume is conserved through all of it. A heap of sand tipped at 76° settles to 34.3°, and a
  pit's walls to 34.7°.
- **In play:** a dig takes its cubic metre stroke by stroke where the digger looks. The spoil
  is thrown clear of the hole and slumps if loose; intact earth stands in a pit's wall.
- **Saves** (format 8): changed ground keeps its fill, and older changes blend into the
  regenerated ground.
- **Deferred** (`PLAN.md`):
  - the client's aim and a dig preview from the field (S2);
  - collision and Level ground on the field (S3);
  - fresh cuts holding sharper;
  - the bot suite.
- **Real?** Repose angles and friction from measurements, with sources; the volume dug is the
  volume moved.
- **Lean?**
  - one fill byte per voxel, only in surface cubes;
  - one module for looking, digging, piling and slumping;
  - the old block-level dig and slump code is removed.
- **Fast?** A stroke's dig, heap and settle touch a few thousand voxels. Generation adds one
  subtraction a voxel.
- **Whole?**
  - generation, saves, the wire, digging, the spoil and the hole after a reload agree (the
    workshop test);
  - the lint holds every natural block to a family.
- **Organic?** Heaps run and pits' walls lean back by their material and by the rain, not by
  a rule of blocks.

## Audit 1 (2026-10-08, `docs/review/audits/AUDIT-1.md`)
- Covers E1–E6 and P3–P5. An independent reviewer re-read the newest modules.
- **Fixed:**
  - resuming part-done work showed it undone;
  - drying work looked at before its first minute read as dry;
  - drying now stops short of dry in damp air;
  - the action menu's felling time ignored the trunk's girth;
  - a felled slim tree's stem was clipped (the felling test passes again); the thatch
    acceptance looks inland from a beach;
  - dead code and wrong docs.
- **Metrics:** 123,200 lines of Rust (+5,900). Eight long files, `client.rs` now 4,390. PROGRESS
  was trimmed to its recent entries.
- **To PLAN:** forgetting work kept for gone things, per-thing preferences, duplicate helpers,
  language keys, the full suite, the long files.
- **Next:** S1.

## P5 — motion timing (2026-10-08, D258–D259)
- **Feet do not slide:** a person's planted foot now goes back under the body at the ground's
  pace while it is flat, then rolls over its ball. The slip is under 0.04 m/s walking, 0.09
  jogging and 0.19 sprinting; before, it was some 0.5 m/s at a walk. A foot swinging through
  no longer dips into the ground (`docs/review/p5/gaits.png`). Animals' feet keep pace walking,
  trotting and galloping: legs reach further, and a hare's bound has a shorter footfall.
- **Clouds change shape** as they go: a cloud is another in some ten minutes. This is capped
  like their drift when time runs fast.
- **Smoke** is carried off at the wind's speed (it went at 0.3 of it).
- **The audit:** `docs/design/motion-timing.md` lists everything that moves: its clock, its speed,
  where it is done and what checks it. That covers the sky, water, rain and snow, sway,
  caustics, textures, smoke, shimmer, the eye's meter, gaits, work strokes, blows and animals.
  It says what does not move yet (rivers' flow, fog banks, lightning, flicker) and how the
  visuals time-lapse in fast-forward.
- **The check** (`hearth_render/tests/motion_timing.rs`):
  - every clock a shader reads, in every function, must have its row in the document;
  - a shader naming a clock the check does not know fails it;
  - the speeds the shaders give must be the ones the document states.
  - With it: the sky in fast-forward (a thousand times: a capped step a frame), and the feet
    tests for people and animals.
- **P6 removed** (D258): no tutorial, first-time hints or Field Guide.
- **Deferred** (`PLAN.md`): rivers flowing at their current, waterfalls, fog banks, lightning,
  flames; a check that measures motion in rendered images.
- **Real?** Stance by the leg's geometry against the ground passed; the stride after
  Alexander; smoke at the wind's speed; a cumulus's life.
- **Lean?** One solver for the stance hip. One check reads the shaders and the document
  together.
- **Fast?** Five Newton steps a leg a frame. The checks run in milliseconds.
- **Whole?** People, animals, the sky, water and particles are all timed against one table,
  and fast-forward is covered.
- **Organic?** Clouds grow and fade as they drift. Feet grip the ground.

## P4 with E6 — work by the body; sleep (2026-10-08, D255–D257)
- **Stroke by stroke:** every attended process has a work model (pose, stroke, hands; linted).
  Holding the hand's button works and letting go stops (or click to start and stop,
  Accessibility). Work part done is kept, saved, and taken up where it was left.
- **Poses:** the body works in fourteen poses (kneel and dig, squat and knap, sit and work in
  the lap, chop, pick, pluck, drill fire, grind, haul…), a stroke looping at the work's tempo
  (`docs/review/p4/work_poses.png`).
- **Work left to itself:** every unattended process has a state model. Drying follows moisture
  by warmth, dryness, wind and sun, and rain sets it back: meat dries in a day or two of fair
  weather, more than twice as long in damp. Soaking goes by the water's warmth.
- **Looking closely** tells how the work looks now, what rain did, and to one who knows, how long
  more it would want; with the numbers in Creative.
- **Sleep at night:** the drop-off threshold is the two-process model's. The Rest screen says
  when sleep would come and offers resting until then.
- **Deferred** (`PLAN.md`):
  - real effects per stroke and work in progress shown in the world;
  - cooking by core temperature, smoking and tanning;
  - hands in first person;
  - gathering one at a time and the loose-objects layer (P7).
- **Real?** The two-process sleep model; drying by the vapour deficit, warmth, wind and sun;
  strokes at their documented rates.
- **Lean?** One work model and one state model per process, in data. Poses are shared by any
  body.
- **Fast?** Poses are a few trigonometric terms a joint. A batch's drying is a step a game
  minute.
- **Whole?** Hands, poses, strokes, kept work, waiting work, inspection and sleep fit
  together. The lint holds every process to it.
- **Organic?** Work's pace follows the weather and the body. Nothing jumps from a bar to done.

## P3 — looking and the hands (2026-10-08, D252–D254)
- **The two hands:** left click works the left hand and right click the right (E the right;
  the controller's triggers). Each does its natural use on what is looked at, with what it
  holds:
  - the use comes from data (`interaction/intents.ron`, a resolver in `hearth_craft`,
    mind-agnostic);
  - the tools are that hand's;
  - a held thing is put away when the use wants an empty hand, and brought back after;
  - held, the button does the work again as each is done.
- **Safe defaults only:** no punching animals, no eating what is not known to be food, no
  building (linted).
- **The action menu** (the wheel's click):
  - every action for what is looked at, with its tools and about how long, greyed with what it
    lacks; one's own actions and rest when looking at nothing;
  - done with either hand;
  - a process picked twice running over a hand's own use becomes that hand's use there
    (`hands.json`; Controls forgets).
  - Creative's pick moves to Ctrl + the wheel's click.
- **By the crosshair:** what is looked at is outlined and named (off, brief or always), with
  what each hand would do. There is no offer list and no work bar. Discoveries are a quiet
  note by a small journal.
- **Picking:** some 2.5 µs a frame (under 0.2 ms asked).
- **Deferred** (`PLAN.md`): a scripted playtest through the client's buttons; the controller's
  radial menu; sub-object picking (S5, P7).
- **Real?** The hands do what hands do with the thing held: the amendment's every example is
  tested.
- **Lean?** The rules are data. One resolver serves both hands and any future mind; the offer
  list's code is the menu's.
- **Fast?** 2.5 µs a pick. The hands' uses are drawn up with the list, five times a second.
- **Whole?** Hands, menu, hints, highlight, names, stowing and learned uses work together. The
  layout test lays out the new Controls rows.
- **Organic?** A hand's use follows from what it holds and what is there, through the
  processes' own verbs.

Earlier entries (E0–E5, Q1, Audit 0): `docs/history/progress-2026-10-08-e0-to-e5.md`.
