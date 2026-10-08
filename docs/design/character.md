# The player's person: appearance, rig and movement

*Status: implemented (V2-3 part e, D70); the character creator returned with E5 (Amendment E
§6.2). Since E7 the body is drawn as a sculpted, skinned person (`people.md`); the boxes here
remain the rig's parts, the fallback while a body is meshed, and the garments not yet fitted. Code: `crates/hearth_character` (appearance, rig,
animation, boxes), `crates/hearth_render/src/figure.rs` (drawing), `crates/hearth/src/client.rs`
(the body in the world), `profiles.rs` (who the player begins as).*

## Purpose
The player is a person (v2 §9.1–9.2): an adult of their choosing, blocky but built like a
human, who moves as people move. Their body is seen looking down and in a third-person view.

## Model
- **Appearance** (`Appearance`; each world keeps the person it began with in `player.json`;
  until E5's creator, a name and a body chosen on Create World, the rest from the world's seed,
  `profiles.rs`):
  - body (female or male) with an adult's height, 1.45–1.95 m, and build (slight to heavy);
  - skin tone on a continuous scale across the human range (ten swatches ordered by lightness,
    mapped to skin's diffuse albedo of about 0.6 at the lightest and 0.04 at the darkest), with
    an undertone tilting the hue at the same lightness;
  - eleven hair styles and six kinds of facial hair; eleven natural hair colours, plus hue,
    saturation and lightness for any other;
  - seven eye colours, a name, and the loincloth's material.
  It is cosmetic: the body model is one reference adult (D66).
- **Proportions** come from anthropometric fractions of stature (after Drillis and Contini):
  eyes at 0.936, chin 0.87, shoulders 0.818, waist 0.6, hip joints 0.53, crotch 0.47, knees
  0.285, ankles 0.039; upper arm 0.186, forearm 0.146, hand 0.108, foot 0.152. Breadths
  follow adult surveys: bony shoulders 0.237 / 0.224 and hips 0.196 / 0.220 (male / female).
  Build thickens flesh, the waist most, and bones little.
- **The rig** has seventeen joints (root at the hip joints, waist, chest, neck, head, and
  shoulder, elbow, wrist, hip, knee and ankle on each side). Parts are boxes on the joints,
  some rounded as three overlapping boxes:
  - the trunk in pelvis, abdomen, chest and shoulders, with a bust for a female body;
  - limbs as two tapering boxes each (deltoid to elbow, forearm to wrist, thigh to knee,
    calf to ankle), hands with thumbs, and feet from heel to toe;
  - a face with eyes (white and iris), brows, nose, mouth and ears;
  - hair and beards as boxes laid over the head by style;
  - the loincloth: a tie, front and back flaps, and the cloth between the legs. A female body
    also starts with a band across the chest (D70).
- **Movement** is procedural, from what the body is doing (`Activity`), cross-faded over a
  quarter second when that changes:
  - The gaits are driven by distance, a cycle being two steps (1.5 m walking, 2.3 jogging,
    3.2 sprinting, 1.4 wading). Each foot is down for a share of the cycle (62 % walking,
    40 % jogging, 30 % sprinting) and lies flat while it carries the body (the ankle
    dorsiflexes by knee minus hip). It rolls onto the toes before it lifts, the knee already
    bending before toe-off when walking. Then it swings forward, knee bent, reaching past where
    it lands and settling back.
  - The hips ride on the feet that are down, so the body bobs as it does in life (about 4 cm
    walking, 6–8 running); between running strides it flies in an arc from push to landing.
  - Arms swing with the other side's leg, the trunk twists against the pelvis and leans into
    speed.
  - Footsteps heard and feet seen are kept together: each footstep pulls the gait's phase to
    that foot's strike.
  - Other activities:
    - crouching, with feet flat;
    - crawling low on forearms and knees (the crawling box is 0.6 m, too low for hands and
      knees);
    - breaststroke at the surface and under water, and treading water upright with sculling
      hands;
    - pulling up onto a ledge (arms pull, then press, a knee comes up), climbing a ladder hand
      over hand;
    - falling with arms out, and lying on the back (asleep, unconscious, dead).
  - Standing, the chest rises and falls with the breath and the weight shifts. Shivering hugs
    the arms to the body with a tremor.
  - The head turns and tips where the eyes look, against the trunk's lean.
- **First person**: the camera is at the eyes of the posed body: looking down tips the head
  and shows the chest, belly, legs and feet. The head and neck are left out (the eyes are in
  them). With view bobbing off, the eyes keep a steady height for the person's stature. The
  body turns to where the eyes look when moving, or when the head would turn more than 55°.
- **Third person** (the camera view key, mouse button 4 by default): behind the shoulders, then facing the player,
  3.5 m back from the eyes or short of anything solid.
- **Drawing**: each box is an instance of the unit cube placed by a 3×4 matrix, about 90 a
  person.
  - In the world they are lit as the terrain is: sky light by the sky-light level where the
    body is, the sun where the sky is open (wrapped a little, as skin scatters), firelight by
    the block light, then aerial perspective.
  - A preview (`figure.rs`, for E5's creator) has its own lights (daylight, overcast, dusk, firelight),
    exposure and tone mapping.
  - `hearth --screenshot person=4` stands someone in a shot; `body=true` looks through their
    eyes.

## Parameters
Gait tables (`WALK`, `JOG`, `SPRINT`, `WADE`) and poses are in `animate.rs`; proportions in
`rig.rs`; colours in `appearance.rs`.

## Interactions
- Movement (`hearth_physics`): activity from motion and stance, speed, climbing progress.
- The body (`hearth_body`): shivering, unconsciousness, sleep and death take the pose.
- Hearing (`hearing.rs`): footsteps set the gait's phase; breathing sets the chest's rhythm.
- Light (`hearth_world`): the sky and block light where the body is.
- Clothing (`clothing/basic.ron`): the starting garments match what is drawn.

## Known simplifications
- No hand or arm actions yet (knapping, carrying): they come with the hands' work (V2-4/V2-5).
- Garments other than the starting ones are not drawn yet (V2-4 makes worn clothing
  visible).
- Sideways steps use the forward gait (backward steps run it in reverse).
- Feet are not placed on slopes and steps individually; hair is rigid and follows the head.
- Hair does not grow yet (v2 §9.1's nice-to-have).

## Future extensions
- Phase F's simulated humans will use the same rig with their own proportions.
- Inverse kinematics for feet on uneven ground and hands on what they hold.
