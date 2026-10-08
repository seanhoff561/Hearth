# Work the way the body does it

*Amendment E §7 with P §6–7 (P4 with E6). Status: implemented for strokes, poses, work kept
part done, drying and soaking by the weather, inspection and sleep. The loose-objects layer
and gathering one thing at a time are P7's.*

*Code:*
- `hearth_content::schema::process` (`WorkModel`, `WorkPose`, `StateModel`)
- `hearth_character::animate` (`Activity::Work`, `work_pose`)
- `crates/hearth/src/workshop.rs` (begun work, `batches`, `dry_step`, `inspect`)
- `hearth_body::sleep` (`hours_until`), `hearth_player` (`SLEEPY`), `crates/hearth/src/rest.rs`

## Purpose
- **No timers, no bars.** Work is done by the body, a stroke at a time, while the hand's button
  is held.
- **What is done stays done.** Work left part done keeps what was done.
- **Work left to itself** goes on by the weather and the heat, and can be looked at.
- **Sleep** comes at night, not at noon.

## Active work (E §7.1–7.2)
- **The work model:** every attended process says how the body does it (`work`):
  - its pose;
  - a stroke's seconds;
  - what one stroke does in words ("a scoop of earth, about half a litre");
  - one hand or both.

  A doing takes as many strokes as its duration over the stroke's. The durations' sources are
  the processes' own. The lint fails an attended process without a work model, and an
  unattended one with one.
- **Holding the hand's button works; letting go stops.** An Accessibility setting makes it
  click to start and click to stop.
- **Work part done is kept**, by process and by what it is done to: a block, a thing, or what
  is in hand. This holds whether the work was let go, walked away from or slept on. It is saved
  with the world (`begun` in `crafts.json`). The same work on the same thing is taken up where
  it was left.
- **The poses** (P §6.1) are fourteen. Each is a posture with a stroke looping at the work's
  tempo, led by the hand doing it, and the body stands on the ground in each:
  - kneeling to dig, standing to dig;
  - squatting to knap, sitting with work in the lap;
  - kneeling at water and at a fire, drilling fire;
  - chopping, reaching to pick, bending to pluck, snapping a branch;
  - hammering on an anvil, grinding at a quern, hauling.
- The player's body takes its work's pose while working, standing still. Any body can use the
  same poses (the Actor rule).

## Work left to itself (E §7.4–7.5)
- **The state models:** every unattended process says how it goes on (`state`, linted).
  - **Drying:** moisture, in kg of water per kg dry, falls toward the air's own moisture at the
    model's rate. The rate doubles for every ten degrees warmer, and rises with the air's
    dryness, with wind (+25 % per m/s) and in sun (×1.5). Rain wets it again, up to where it
    began.
    - Meat strips on a rack: from 3.0 to 0.35, in a day or two of fair weather.
    - A mudbrick: 0.3 to 0.03.
    - A pot: 0.25 to 0.03.
  - **Soaking:** hours counted as the water's warmth makes them (Q10: retting 2.2, leaching
    1.3).
  - **Firing:** judged by its heat, as before (D-V2-12).
- **Looking closely** (the action menu on the work) tells:
  - how long it has been left;
  - how it looks now (wet through, drying at the edges, leathery, dry through), and how long
    rain wet it;
  - to one who knows the work, how long more it would want in weather like this;
  - in Creative and Developer mode, the numbers.

## Sleep (P §7.1)
- **The model:** Borbély's two processes (`hearth_body::sleep`). Pressure rises awake
  (τ 18.2 h, faster with hard work) and falls asleep (τ 4.2 h), and the body clock swings it.
- **Falling asleep:** a body lying sleepy at ease drops off after a quarter of an hour. Sleepy
  means at the model's upper threshold (0.58 with the clock): about ten at night after a day up
  from six, not at noon.
- **The Rest screen** says when sleep would come ("You are not sleepy yet. Lying here, you would
  probably fall asleep in about 4 hours.") and offers to rest until sleepy, then sleep until
  rested.

## Known simplifications
- **Strokes:** a stroke is the doing's share at its documented duration; no real effect per
  stroke is modelled (a litre of earth moved, a notch cut deeper). The world shows the result
  when the work is done. Half-dug pits and half-cut trees do not show, and rain does not fill
  a pit left part dug.
- **Cooking** at a fire is attended work on a timer, not a core temperature. Smoking, salting,
  fermenting and tanning wait for their processes.
- **First person:** the view does not show the hands at work. Third person shows the whole
  body.
- **Later work:** the loose-objects layer, gathering one at a time and harvesting from living
  plants are P7's (E §9.1).
