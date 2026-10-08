# Processes and the knowledge graph

*Status: implemented for Eras 0–2 (V2-5, D72). Data in `processes/`, `knowledge/`,
`workstations/`; code in `crates/hearth_craft` (`knowledge.rs`, `engine.rs`, `knap.rs`), the
discovery vocabulary in `crates/hearth_content/src/triggers.rs`, the server's side in
`crates/hearth/src/workshop.rs`, the player's in `crafting_ui.rs`, `journal_ui.rs` and
`knapping_ui.rs`.*

## Purpose
v2 §11.3 and §12: things are made by processes done in the world, with tools, at workstations,
under conditions; what a person can do comes from what they know, and knowledge is gained,
never granted: by trying things, watching, thinking it over, or reading what others left.

## Model
### Processes
- A process declares inputs (an item, a form with a material filter, a bulk material by the
  kilogram, a tag, a garment), tools by property (`hard_hammer ≥ 0.5`, `sharp_edge ≥ 0.3`),
  a workstation or a **target** (a block by id, material, name suffix or any of several; a thing
  lying in the world; water; a fire; open ground), conditions (heat at the fire, water, dry
  weather, daylight, cold, shelter, a feature nearby, the season), a duration in real hours, the
  knowledge and skill involved, outputs with quality rules, by-products,
  failures (chance for a novice falling with skill, words, inputs lost, an injury), and:
  - a **verb** (`strike`, `cut`, `scrape`, `heat`, `dig`…) for the triggers it teaches;
  - an **effect** on its target (keep, remove, excavate, deplete, ignite, feed, bank, mend);
  - whether it is **attended** (the person works throughout) or left to itself (drying,
    soaking);
  - its work in METs, the wear it puts on tools, and how often a block yields it a year.
- **Planning** (`engine::plan`) reads a *bench*: the things at hand (the target thing, the
  hands, everything carried, things lying within 2.5 m), what is aimed at and the surroundings.
  What is worked is chosen first and the tools from what is left in hand (the flint is struck,
  the granite is the hammer). Bulk materials count in their bulk form's units: a quarter-kilo
  cut of meat, a handful, a hank of fibre, a lump of clay, a hide, a strip of bark, a blob of
  tar, a piece of bone, a sheaf of reeds.
- **Time**: the data's duration is a skilled person's; a novice takes twice as long, a poor
  tool longer and a fine one less (×(0.5/strength)^½, 0.6–2), and it takes that long in play:
  work in hand is never hurried (E3, `time.md`); work left to itself is waited on with Rest.
- **Outcome** (`engine::perform`): failures roll in order (coarse stone fails more in
  knapping, damp air in friction fire); outputs are made of the material in play (the first
  input's, else the target's) and get a quality from skill, material and the data's rule; tools
  lose condition; what doing it teaches is heard (below). Unattended work lies where it was set
  up as a thing with a batch and finishes when its conditions have held long enough; rain on it
  raises its failure chance.
- **Things' state** (`Stack`): quality (scales a tool's strength, 0.7–1.3×), condition (an
  edge's or point's wear, 0.4–1×; worn through it breaks; retouching, resharpening and rebinding
  mend it), decay (food going off), glow (an ember's hours left).
- **Workstations** are laid out by a generated `build_<station>` process from their parts and
  stand as blocks; hearths and lamps keep a fire (see [fire-and-food.md](fire-and-food.md)).

### Contextual actions
Looking at something with things in hand, the crosshair lists what can be done *with what the
person knows*: the doable first (with the play time), then up to three that lack something
(and what), with eating, drinking and filling a skin when they apply. The mouse wheel chooses,
the primary action (left button) does it, E does the first; doing it again or walking away stops
attended work. Throwing (hold R, let go) flies a thing in the right hand ballistically.

### Knowledge and discovery
- A node listens for **triggers** (`hearth_content::triggers`): `verb:key` from a process's verb
  applied to each thing it uses and its target (keys: the form, the material, their tags), plus
  `do:<process>`, `use:<tool keys>`, what the process `teaches`; `see:<keys>` for what the eyes
  rest on (once an hour), `throw:<keys>`; the systems' own (`carry:full_hands`,
  `see:wildfire`, `sleep:on_hide`); and `infer:<node>`, heard when every prerequisite of a node
  with an inference route is known: on learning one of them, and on doing a process one of them
  enables.
- Each trigger heard adds its route's insight; at 1 the node is learned once everything it
  requires is known (insight waits). The first insight from a route with a hint writes a hunch
  in the journal; Guided mode names the technique in every hunch.
- **Knowledge Modes** (a world setting): Discovery (default), Guided, Open (everything
  implemented known, for sandbox and tests).
- **Skills**: practice (real hours of work, failures too) raises a skill with diminishing
  returns, 1 − e^(−h/25 h); unused for a month (Authentic preset) it slips a thousandth a day,
  never below three fifths of its best.
- **Death** (v2 §9.8): under Legacy the new person inherits what the dead knew as *legends*
  (in the journal): a legend's processes may be attempted, and doing one learns it again (its
  triggers count four times); skills are lost. Hardy keeps everything. Permadeath's tale lists
  what was learned.
- **The journal** (J): known techniques by era with their real history and date, hunches not
  yet understood, skills in words, and every note by day.

### Knapping by hand
Shaping a chopper, hand axe, point or scraper opens the stone seen from above with the shape
wanted drawn over it (`knap.rs`). Drag from the edge the way the blow should drive the flake,
further for a harder blow; the flake's run follows the force, and how predictably it runs follows
the stone and the knapper's skill (flint takes the line; quartzite and basalt stop short or run
on). A blow that splits the piece snaps it. The quality is how well what is left fills the
shape (overlap share 0.45 → 0, 0.9 → 1); it mostly sets the made tool's quality (70 %, the rest
from skill and material). "Work it as usual" leaves it to the hands' habit (the engine's roll).

### Eras 0–2
58 nodes (Appendix C); 51 implemented. 117 processes: gathering by hand, experiments that anyone
can try (knock stones, hack at a carcass, roll fibres, rub sticks, hold things in the fire…)
and the techniques knowledge opens. Seven nodes wait for their systems: windbreaks and lean-tos
(building, V2-8), fish weirs (fish and building), rafts (watercraft, V2-12), taming wolves (fauna,
V2-7), fletching (feathers, V2-7) and painting (marks on surfaces, V2-8).

## Parameters
- Durations, failure chances, quality rules, insights and hints: in the data.
- `PRACTICE_H` 25 h (skill to two thirds), `SKILL_KEEPS_DAYS` 30; tool speed 0.6–2×; novice 2×.

## Interactions
- Body: the work's METs, injuries from failures, eating and drinking, fires' radiant heat.
- Items: inputs and outputs by unit, tools by property and condition, things lying about.
- World: blocks dug, picked, removed or depleted; stations as blocks; harvest limits per block
  and year; seasons gate fruit, nuts and fibre.
- Weather: rain stops drying, puts out fires and soaks fuel; humidity hampers friction fire.

## Known simplifications
- Processes check their conditions when started and when finished, not between.
- A legend is learned again by doing one of its processes once.
- Inference is heard on learning and on doing related work, not on resting.
- Observation of animals and found evidence wait for their systems (V2-7). Being taught waits for Phase F; every node is reachable by a lone player (Amendment E §2.4).
- The knapping game is a plan view; thickness and platform angles are not modelled.

## Future extensions
- Minigames for whittling, hide scraping, weaving and sewing; Phase F's simulated
  humans running the same processes and knowledge.
