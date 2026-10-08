# Controls: bindings, lone modifiers, the controller, and what a click at nothing does

*Amendment E §3 (E2). Code: `hearth_input` (keys, actions, bindings, per-frame state),
`crates/hearth/src/controls_ui.rs` (the Controls screen), `gamepad.rs`, `strikes.rs` (blows and uses
on the server and the client's choice), `hearth_player::strike` (the body's blows),
`hearth_character::animate` (their poses), `hearth_fauna::wound` (what they do).*

## Purpose
Any key, mouse button or controller button can work any action, Ctrl and Alt alone included;
and the main button always does something sensible: what the thing in hand is for, or a blow.

## Bindings
- Every action has a **key** (keyboard or mouse, with Ctrl, Shift or Alt held as needed) and a
  **controller button**, each rebindable on the Controls screen: click the key's column or the
  controller's, then press the new one (Escape unbinds). Stored in `options.toml` as
  `controls.key_bindings` and `controls.pad_bindings`, only where they differ from the defaults.
- **Left and right modifiers are keys of their own** (Left Ctrl, Right Ctrl, Left Alt, Right
  Alt/AltGr, Left and Right Shift, the system key). As modifiers in a combination either side
  counts (Ctrl + G is either Ctrl and G).
- The capture (`RebindCapture`) is fed presses **and releases** (the cause of the owner's report:
  only presses reached it). A modifier pressed and let go with nothing else pressed binds that
  key alone; a modifier held while another key is pressed makes a combination.
- **The system's shortcuts are refused** and, in play, left to the system with nothing done in
  the game: Alt + Tab, Alt + F4, Alt + Space, Alt or Ctrl + Escape, Ctrl + Shift + Escape,
  Ctrl + Alt + Delete, Ctrl + Alt + F1–F12 (`Binding::reserved`). The system key has no
  combinations (on Windows and macOS its shortcuts are the system's) but binds alone.
- Mouse side buttons (Button 4, 5 and beyond) bind like keys.
- Losing the window's focus lets go of everything held, drops any tap waiting on a key, and
  forgets the modifiers held for a capture (their releases may never come).

## A lone modifier and its combinations
When a modifier (or the debug key, F3) is bound alone and combinations are made with it:
- a **hold** action on it (crouch, run, the hand's work) starts on the press and stays held
  through the combinations;
- a **tap** action on it (the inventory, the journal, the debug screen) waits for the key to be
  let go, and comes only if no other key or button was pressed meanwhile.

Actions declare their kind (`Kind::Hold`, `Toggleable`, `Tap`). This generalises the debug key's
old rule (F3 alone toggles the debug screen; F3 + W is a chord). The Controls screen shows such
overlaps as a note under the list for the row pointed at ("Left Control alone: Crouch, dive
from the press, held through Ctrl + G (Drop all)."), not as conflicts; only two actions on the
same binding in overlapping contexts are conflicts, shown in red.

## The controller
The sticks walk and look (fixed). Every button is bindable; the defaults keep the earlier
layout and add the hands: South jump, East crouch, West crawl, North inventory, left stick
click run, right stick click kick, right trigger use/strike, left trigger interact, right bumper
throw, left bumper quick choice (the right stick points at a slot), Start pause, Select the
globe, D-pad journal (up), drop (down), lie down (left), camera view (right). On a controller,
crouch, run and crawl **toggle** with each press (the thumb that presses them also steers); the
run ends when the stick comes back. In menus the D-pad and the left stick move the focus, South
presses and East or Start goes back (fixed).

## Platforms
- **Windows:** winit 0.30 does not pass Alt's key-up to Windows when the window has no menu bar,
  so a lone Alt (or F10) does not open the window menu or take the focus; it drops the fake Left
  Ctrl that Windows sends before AltGr on layouts that have it, so AltGr arrives as Right Alt.
  Alt + F4 and Alt + Space reach Windows. *Manual check recorded in `dev/PLAYTEST.md` #19
  (needs a Windows machine).*
- **macOS:** Alt is shown as Option and the system key as Command; Cmd + Q and the other
  application shortcuts stay the system's (the system key makes no combinations).
- **Linux:** the system key is shown as Super.

## A click at nothing (E §3.2)
With nothing highlighted, the primary button does what the thing in hand (the right's, else the
left's) is for (`strikes::use_of`):
1. food: a bite of it; a water skin with water in it: a drink;
2. a thing a wound is treated with (a process's `treats` that takes its material): the
   treatment;
3. its data's **primary use** (`primary` on item forms and items): a burning brand is held up
   (and lowered), a bow is drawn while the button is held and loosed on release (an arrow
   carried, anywhere), and the blows: **thrust** (spears, poles, digging sticks, darts),
   **swing** (sticks, hafted axes, adzes, planks), **slash** (flakes, blades, scrapers,
   sickles), **stab** (points, awls, burins, needles, arrows in the hand, bone splinters),
   **strike** (cobbles, cores, hand axes, choppers, bricks);
4. anything else: a blow with the fist that holds it; an empty hand **punches**.

A **kick** has its own key: T (it lies by the movement keys and holds nothing else in play; F3 +
T is a debug chord, apart), the right stick's click on a controller. With something highlighted,
the intent resolver decides as before (P §5.2; at an animal, a blow with what is in hand). With
nothing aimed at, E (interact) does the offer chosen with the wheel, for making things with what
is held until P3's action menu.

## Blows
Each blow is drawn back, struck and recovered from at an untrained adult's speed; the server
runs it (the client shows the same blow at once), and it lands at the end of its wind-up on
whatever lies along its path then, so an animal that moves off is missed.

| Blow | Wind-up · strike · recovery (s) | Energy at full strength (J) | Reach from the eye | Path |
|---|---|---|---|---|
| Punch | 0.15 · 0.10 · 0.25 | 40 | the arm, 0.4 × stature | along the look |
| Kick | 0.25 · 0.15 · 0.40 | 120 | the leg, 0.53 × stature, from the hip | toward what the look rests on, no lower than the ground, no higher than the chest |
| Thrust | 0.15 · 0.12 · 0.40 (from the ready) | 150 for a spear (≥ 1.5 m) with the body behind it, else 40 + 60 m | arm + reach | along the look |
| Swing | 0.35 · 0.15 · 0.45 | 60 + 100 m | arm + length | an arc across the body from the hand's side (70° to −45°), falling a little |
| Slash | 0.15 · 0.12 · 0.25 | 25 + 20 m | arm + length | a shorter arc (45° to −45°), falling across |
| Stab | 0.15 · 0.10 · 0.25 | 30 + 40 m | arm + length | along the look |
| Strike | 0.25 · 0.10 · 0.35 | 40 + 60 m | arm + length | down from overhead (65° to −25°) |

*m* is the thing's mass (kg, capped); heavier things are slower (× √m, at most 1.6). Each costs
seconds of all-out effort (0.4–0.8, a swing more for its weight) out of the body's 15 s of
stamina. A tired body strikes with 0.4 + 0.6 × its stamina of the energy, and more slowly.
Blows carry the thing's point (`piercing`) or edge (`sharp_edge`): thrusts and stabs pierce,
swings and slashes cut with an edge (blunt without one), strikes are blunt or a little cutting;
fists and feet are blunt. The animal wound model (`fauna.md`, "Hunting and wounds") gains cuts:
shallower than a point but long, a throat or a leg's great vessel bleeding out within a minute
or so, a leg's tendons laming it. A blow's momentum (√(2 × moving mass × energy): 2 kg for a
fist, 5 for a leg, 10 for a spear driven with the body) **shoves** the animal: a kick sends a
hare tumbling (capped at 6 m/s) and barely moves a hind. Thrown things and arrows shove by their
own momentum. A bow sends its arrow at the speed half its draw's work gives it (draw weight ×
0.7 m, a triangle under the force line; a full draw in a second): a 25 kg self bow sends a
30 g arrow at some 50 m/s.

Sources: an Olympic boxer's punch reaches about 9 m/s (Walilko, Viano and Bir 2005, *Br J
Sports Med* 39: 710–719); an untrained adult's is taken at 6–7 m/s and 40 J. A front kick's
foot moves at 7–10 m/s. Self bows of 20–30 kg draw weight shoot arrows at 40–55 m/s. The
spear's 150 J and the stone's 40 + 60 m are V2-7's (`fauna.md`). *(Uncertain: the swing and
slash energies and all recovery times are estimates within the ranges of sports biomechanics;
P5's motion-timing check is to measure them against recorded motion.)*

## The body shows it
`hearth_character::Doing` overlays a gesture on the pose: the arm (or the leg) drawn back,
struck and brought back for each blow, the trunk turning into it; a bow held out with the string
drawn to the cheek; a brand held high and waved (`docs/review/e2/gestures.jpg`). First person
sees the arms come into view.

## Not yet
- Other players are not struck: that waits for the PvP setting (Amendment R §3.6, R3).
- Each hand's own button (left click the left hand, right click the right) and the action menu
  are P3's (P §5.2–5.3); until then the primary button uses the right hand's thing, else the
  left's.
- Blocking, dodging and animals' evasion of a blow seen coming.

## Tests
`hearth_input` (lone modifiers held and tapped, each side its own key, the system's shortcuts,
controller buttons and their toggles), `controls_ui::tests` (Left and Right Ctrl and Alt bound alone
by clicking the row and pressing and letting go; controller buttons in their column; overlaps
explained), `hearth_player::strike` (timings, reach, arcs, momentum), `strikes::tests` (blows
meet a hind along their arcs and miss her out of reach, behind, or off the arc; a kick tumbles
a hare and barely moves a hind; each thing in hand has its use; a bow's arrow speed), and
`tests/fauna.rs` (a hunter's thrust through the server lands after its wind-up).
