# Looking and the hands

*Amendment P §5, as amended by E §3.2 and §9.1 (P3). Status: implemented. Code:
`data/hearth/interaction/intents.ron` (the rules), `hearth_content::schema::interaction`,
`hearth_craft::intent` (the resolver), `crates/hearth/src/crafting_ui.rs` (each hand's use,
the action menu), `client.rs` (`use_hand`, `pick_block`, `highlight_boxes`, `draw_hands`),
`hearth_input` (the buttons).*

## Purpose
Each hand does its natural thing with what it holds, on the one thing looked at. Nothing
happens without a highlighted target or a use of one's own. Nothing risky is a default.

## Looking
- **Picking:** the one thing looked at within reach is picked, and the nearest wins:
  - a thing lying there (its resting box);
  - an animal (its rig's parts, within the reach of the arm and what the right hand holds);
  - a block, by the boxes of its shape (`pick_block`: 3 cm steps along the look, water at its
    surface).
  - It costs some 2.5 µs (`tests/picking.rs`; P asks under 0.2 ms).
- **Highlight:** what is looked at is softly outlined:
  - a block by its shape's bounds;
  - a thing by its box;
  - an animal by a box about its body.
- **Name:** the name of what is looked at, as the player knows it, shows by the crosshair. A
  plant not yet told apart from its look-alikes says what it looks like. A thing too heavy to
  lift says it can be dragged. The Controls setting is off, brief (default: in after a fifth of
  a second, out after three) or always.
- **Hand hints:** under the name, what each hand would do ("L: pick · R: —"). A Controls
  setting hides them.

## The hands
- **The buttons:**
  - Left click works the left hand and right click the right; E is the right hand's, for the
    keyboard.
  - On the controller, the triggers are the hands.
  - A button's names of before ("key.attack") keep their keys.
- **Each hand's use** comes from `intents.ron`'s rules: with what the hand holds, on what is
  looked at, what it does. The rules are tried in order, and the first that can be done now is
  the hand's.
  - **Held:** empty, anything, a thing with a property (`chopping`, `hard_hammer`), a thing of
    a use (thrust), a weapon, a vessel, food known to be food.
  - **Looked at:** nothing, a block (by id, material, suffix or prefix), any block, a thing
    lying there, water, fire, an animal.
  - **Done:** a process by verb (the first that can be done) or by id; pick up, drink, fill,
    eat, or a blow.
- **The tools are that hand's:** `Bench::by_hand`; the server is told which hand
  (`ToServer::Act::with`, `Blow::with`).
- **The examples of P §5.2** (all tested in `hearth_craft/tests/intent.rs`):

  | Hand holds | Looked at | Does |
  |---|---|---|
  | empty | a branch | snaps it off |
  | empty | berries | picks |
  | empty | water | drinks from cupped hands |
  | empty | a loose stone or a thing | picks it up |
  | empty | soft soil | digs by hand |
  | a hammerstone (a nodule in the other hand) | — | strikes a flake |
  | a stone axe | a tree | chops |
  | a spear | an animal | thrusts |
  | a water skin | water | fills it |
  | food | nothing | eats |

- **Safe defaults only:**
  - An empty hand does not strike an animal.
  - Food is eaten by default only when the player knows it for food.
  - No default builds.
  - The lint (`intents`) checks every rule's verb and process and refuses risky ones.
  - The rest is in the action menu.
- **Freeing a hand:** a hand holding something tries the empty hand's rules after its own,
  putting the thing away first (`Target::Stow`: a container, a hanging place, the other hand).
  It comes back to the hand when the work is done (Controls: on by default).
- **Hold to repeat:** a hand's button held does its work again as each is done. E6 makes the
  work itself stroke by stroke.
- **Nothing looked at** (E §3.2): food is eaten, a vessel drunk from, a wound treated, a brand
  held up, a bow drawn; anything else is a blow with what the hand holds, or the fist.

## The action menu
- **Opening and choosing:** the wheel's click (Ctrl + it is Creative's pick) opens the menu.
  It lists every action for what is looked at, with what is carried and known:
  - with what it is done (the tools in hand) and about how long;
  - greyed, with what it lacks.
- Looking at nothing, it lists one's own actions (eat, drink, treat, knap what is held) and
  lying down to rest or sleep.
- **Doing:** the wheel chooses, and each hand's button does it with that hand. Esc or the
  wheel's click closes it.
- **Learned preferences:** a process picked twice running over a hand's own use, for the same
  thing looked at and held, becomes that hand's use there. These are kept in the world's
  `hands.json`, and Controls can forget them.

## Feedback
- **Work under way:** a line says what is being done and how long it has left; there is no bar
  (E §9.1).
- **What came of it:** a short line under the top, fading.
- **Discoveries and hunches:** one quiet line each by a small journal at the top right.
- **Gone:** the list of offers by the crosshair.

## Known simplifications
- Trees are still blocks, so a branch is picked as its block. Sub-object picking of the
  skeleton's segments, plants by instance and terrain by the sculpt preview come with S5 and
  P7.
- **The controller:** the action menu is a list chosen with the triggers, not a radial.
- **The ring at the crosshair:** E §9.1 removed the progress ring. Work in progress shows as a
  line of words until E6 makes work visible in the world.
- **Learned preferences** are per world, not per character: there is one player per world.
