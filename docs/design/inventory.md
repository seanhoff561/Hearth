# Inventory, carrying and clothing

*Status: implemented (V2-4, D71). Code: `crates/hearth_items` (kinds, stacks, containers,
carrying), `crates/hearth_player` (the load on the body), `crates/hearth/src/server.rs` (things
in the world), `client.rs` (aiming, the hands, quick slots), `inventory_ui.rs` (the screen),
`crates/hearth_character/src/rig.rs` (clothing drawn).*

## Purpose
v2 §10: everything has a real mass, volume and footprint. What a person can carry depends on
their hands, clothing, containers and body; nothing goes into an invisible pocket.

## Model
- **Kinds of things** come from the content (`items/`):
  - every form × matching material (a flint flake, an oak log section), with mass = box ×
    fill × density;
  - explicit items;
  - every garment × material it can be made of.
  Each has a footprint in grid cells, a box and colour in the world, stacking (one per cell,
  several to a cell, or bulk), properties, and optionally:
  - a **container** spec (grid, most it holds, liquid, carried on the back);
  - the **attachment points** it hangs from;
  - how it is **worn** (layer, regions, attachment points it gives).
- **Stacks** are named by id, so saves outlive content changes. They keep what a container
  holds and the liquid in it.
- **Containers** hold things by cell, either way round, never overlapping, never more than
  their load; stacks top up; containers nest. In data: a bundle (3×3, 10 kg, on the back), a
  pouch (2×2, 1.5 kg; on a tie or a belt loop), a water skin (2 L), a basket (4×4, 12 kg) and a
  back basket (4×6, 25 kg).
- **Carrying**:
  - a thing in each hand (up to 12 kg in one), or one in both arms (up to half the body's
    mass);
  - things heavier still are dragged (up to three times the body's mass);
  - worn garments, one per layer and region (one belt, one back load);
  - attachment points: a loincloth's tie holds one small thing (one cell, half a kilogram),
    a belt's three loops hold what hangs on loops (pouches, water skins, hand axes);
  - a back load (a back basket or a bundle).
  Moves go by path (a root, then places in containers) and are all or nothing.
- **The load against the body**:
  - walking slows from a fifth of the body's mass, to 55 % at half;
  - no running above two fifths, no sprinting above a quarter; jumps lower with load;
  - no climbing with the hands full.
  - A drag goes as fast as a person's sustained 120 W of pulling allows against the ground's
    sliding friction underfoot (snow 0.12, ice 0.05, stone 0.35, soil and grass 0.45, sand 0.6,
    mud 0.7). A 90 kg log section on grass goes a quarter of a metre a second, the same log on
    snow almost a metre.
  - The load's work is added to the body's: Pandolf's equation for what is carried, and a
    drag's pull at a quarter's efficiency.
- **Things in the world** (`items.json`):
  - put down where the eyes rest (G, Ctrl+G for the whole stack), resting on the first
    surface below;
  - picked up (E) into the containers carried, onto a free attachment point or into a hand,
    or taken hold of to drag if too heavy;
  - dragged (hold F) trailing behind on the ground, and let go;
  - loose stones gathered by hand as three cobbles of their rock.
  Under Legacy and Hardy rules, a dead person's belongings stay where they fell.
- **The inventory screen** (Tab):
  - the body's places, and a grid for each carried container (one held in a hand opens);
  - click to lift and place, right-click to turn or halve, click outside to put down;
  - a tooltip, and the load in words.
- **Quick access**: keys 1–6 draw what hangs at the attachment points into the right hand after
  half a second's reach (again, it goes back). Holding Q opens a wheel of the same places,
  chosen by leaning the mouse.
- **Clothing on the person**:
  - each garment drawn over the flesh it covers, standing off the skin by its layer, in its
    material's colour;
  - a loincloth, chest band and belt in their own shapes;
  - a hood leaves the face open and covers the hair.
  What is worn also sets the body's insulation.
- **Controls** (v2 §10.6):
  - crouch C, prone Z;
  - inventory Tab, drop G (Ctrl+G stack), interact E (looking at a person within reach with
    something in hand: hand it to them, a gift — V2.1 §8.7), drag F (hold);
  - quick slots 1–6, quick choice Q (hold);
  - Body panel B; lie down X.

## Interactions
- Physiology: insulation from worn garments, the load's work, heavy loads tire.
- Movement: speeds, running and sprinting, jumps, climbing, the drag's pace.
- Death rules: belongings left where one fell.
- Blocks: loose stones gathered, surfaces things rest on, friction underfoot.

## Known simplifications
- Containers lying in the world are not opened in place; pick them up (or hold them) to look
  in.
- Things do not get wet, spoil, rot or need care yet (with the thermal and process models,
  V2-5); water skins are not filled yet (drinking, V2-5).
- No throwing; rolling a log (easier than dragging on flat ground) is a contextual action to
  come (V2-5); no travois, sledges, rafts or pack animals yet.
- Heavy loads do not yet make swimming harder or drown a swimmer.
- A grid cell does not know the thing's real shape: a cobble fits a pouch's cell.

## Future extensions
- Pockets and textile bags (later technologies), quivers and sheaths with draw times by kind,
  stashes and caches, carts and boats.
