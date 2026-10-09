# People: the sculpted body, skin, hair and eyes

*Status: implemented (E7, Amendment E §8; D266–D270). Code: `crates/hearth_character`
(`anatomy.rs`, `garment.rs`, `hair.rs`, `eyes.rs`, `person.rs`), `crates/hearth_body/src/skin.rs`,
`crates/hearth_render/src/body.rs` with `shaders/person*.wgsl`, `body.wgsl`, `hair.wgsl`,
`eye.wgsl`, and `crates/hearth/src/people.rs`. The rig, proportions and movement are
`character.md`'s.*

## Purpose
The player characters are the only humans for now and the camera is often close to them
(E §8), so they are drawn as realistic people: an anatomically shaped, smoothly skinned body
from the creator's proportions, skin that shows what the body has lived through, hair of every
creator style that moves, and eyes that look about and blink.

## The body (`anatomy.rs`)
- **A sculpture of forms.** The body is a signed distance field: some 150 anatomical forms
  (round cones and ellipsoids), each on the joint that carries it, blended smoothly (a
  polynomial smooth minimum over a few centimetres): pelvis, belly and ribcage; clavicles,
  trapezius and shoulder blades; pectorals (breasts on a female body), buttocks; deltoids,
  biceps, triceps, forearm masses, elbows; palms with the thumb's mound, three-segment fingers
  with nails, the thumb; thighs with quadriceps and hamstrings, kneecaps, calves, ankle bones,
  heels, arches, balls and five toes with nails; the cranium, the face's mass, jaw, chin,
  cheekbones, brow, nose and nostrils, lips, ears, and the eye sockets and mouth line cut in.
- **Sizes** are the rig's `Proportions` (adult surveys) with build swelling flesh and muscle;
  the creator's face sliders scale jaw, cheekbones, brow, nose, eye spacing, lips and ears.
- **The bind pose** is an A-pose: the arms 30° out, the legs 6° apart, so hands, thighs, arms
  and flanks do not blend into one another; each form is built on the straight rest pose and
  carried into it by its joint.
- **Meshing.** Surface Nets (`hearth_smooth`) through the field at 6 mm cells close up, 12 mm
  within some 14 m and 24 mm beyond (`person::Detail`), the field first sampled four times
  coarser and the fine samples evaluated only near the skin. Each vertex is then moved onto
  the field's zero by Newton steps and takes its normal from the field's gradient: the
  quantized field flattens where forms meet, the field does not. At 6 mm a body is about
  68,000 vertices and 135,000 triangles, meshed in about 0.7 s (dev-opt build).
- **Skinning.** Each vertex follows the four joints whose forms are nearest it, weighted by
  nearness (bytes summing to exactly 255: a sum off by one moves the skin by a part in 255 of
  its distance from the origin, centimetres at the head). The palette is each joint's pose
  times the inverse of its bind pose.
- **Tissue** per vertex: lips and nails (blended over a cell at their edges), the short hair
  on the skin (the scalp under any style, a buzz cut, stubble and the skin under a beard), and
  whether it is a garment's.

## Garments (`garment.rs`)
The garments a life starts in are fitted meshes merged into the skin's: the **loincloth**, a
cord following the hips' surface and front and back flaps hung from it and draped by the
body's field (their lower parts carried partly by the thighs); the **chest band**, a strip
wrapped round the chest on the trunk's skin, spanning the cleavage. Other garments are drawn
as the rig's boxes over the body (`Show::clothes_only`) until S6's item meshes.

## Skin (`body.wgsl`, `skin_lut.rs`, `hearth_body::skin`; Amendment T §2.2, D293)
- **Colour:** the diffuse albedo of measured skin, ten steps from the fairest (some 0.64 of red
  light reflected) to the deepest (0.05), redder than a palette's swatches (green 0.5–0.8 of
  red, blue 0.3–0.7: the blood under the skin takes green and blue, melanin blue most),
  anchored on the ColorChecker's skin patches; the undertone tilts the hue at the same
  luminance.
- **The light under it:** pre-integrated through skin's diffusion profile (d'Eon and Luebke's
  six Gaussians) on a ring of the surface's curvature (Penner and Borshukov), a 128 × 64 table
  made at start: Lambert's law on a cheek's curve, red light carried round past the terminator
  on an ear's rim, a nostril's wing, a fingertip. Each vertex's curvature is the mean of its
  edges' normal curvatures, smoothed once.
- **Its surface by region** (`Anatomy::surface`, per vertex): oil from the sebaceous glands'
  density (the T-zone of forehead, nose and chin most, the rest of the face, scalp, upper chest
  and back less, the limbs little, palms and soles none); how shut in (the body's field stepped
  out along the normal against its own rate of growth: creases, the ear's folds, between the
  fingers); fine-line depth (forehead, the eyes' outer corners, neck, hands most).
- **Highlights:** two GGX lobes, the main at α 0.26 (oily) to 0.47 (dry) and a weaker, broader
  sheen (α 0.64–0.81), mixed 0.85 : 0.15, F0 0.026–0.034 (keratin's 0.04 on nails); widened by
  the relief finer than the pixel so that, seen from a few metres, the face's highlights match
  the roughness Weyrich et al. measured (Beckmann 0.3–0.5). The diffuse is given only what the
  surface does not reflect.
- **Relief:** pores (a cell of 0.8 mm, larger and deeper where oily), the micro-relief's
  furrows (0.7 mm), fine lines (3–5 mm) and goosebumps, in the bind pose's frame so they move
  with the skin; drawn as tilted normals where a pixel resolves them and folded into the
  highlights' roughness and the light's occlusion where it does not (no shimmer, no vanishing).
- **The sky:** its light in the mirror direction by the lobes' directional albedo, and the
  ambient, both shut out in creases (the reflection more).
- **Wet:** a smooth film of water (F0 0.02, α 0.014) over the skin; the skin's own sheen drowned
  under it (water against skin hardly reflects), darker beneath.
- Lips moister and a little glossier; nails smooth keratin, lit without scattering, never shut
  in. Review: `docs/review/t1/skin_lights_*.jpg` (before and after, four lights) and
  `skin_world_*.jpg`; `hearth_render/tests/body_preview.rs` `skin_under_four_lights`.
- **State, from the body simulation** (`hearth_body::skin`, saved with the body, sent in
  `BodyView`):
  - *Sun:* the erythemal dose from the UV index (1 UVI = 25 mW/m² weighted; an SED is
    100 J/m²) on bare skin; past the skin's minimal erythema dose (2 SED at the fairest tone
    to some 20 at the darkest) it burns, the redness coming over hours and fading over days;
    every dose tans a little, the melanin coming over days and fading over weeks. The UV
    index comes from the sun's height (12.5 × sin(elevation)^2.42), cloud (−70 % under full
    overcast) and altitude (+6 % a kilometre).
  - *Dirt* on the legs from walking on soil (mud most) and on the hands from handling it;
    *blood* where wounds bleed; both washed off by water (seconds in it, minutes in rain) and
    worn off over a day or two. *Scars* where injuries of severity 0.3 or more healed.
  - Drawn: wet (a film of water), tan, sunburn (not under the fitted garments), pallor (cold,
    blood lost; lips bluish), flush (heat, effort), goosebumps, and per joint dirt in
    patches, blood in streaks, scars as pale lines.

## Hair (`hair.rs`, `hair.wgsl`)
- **Cards** grown from roots on the scalp within a hairline (high at the forehead, receding at
  the temples, over the ears, low at the nape), each path set off along the style's flow (down
  and away from a parting, the front swept back clear of the face), held to the head's curve
  while on it and falling free below, bending under its weight and kept off the body by its
  field. Styles: layered flat cards (short crop, shoulder length, long straight, long wavy,
  curly with waves); shells of outward cards (coily); cornrows and hanging braids, locs and a
  tied-back tail as crossed cards; brows (two rows along the arch) and facial hair (moustache,
  goatee, short and full beards) lying close. 400–1,000 cards a head.
- **Shading:** strands painted per card (straight, wavy, coiled, braided, felted, brow,
  beard), ending raggedly, alpha-tested; Kajiya–Kay diffuse, the surface's white highlight
  shifted toward the root and the coloured one through the fibre toward the tip, light
  through the hair from behind, darker deeper in and at the roots; wet hair darker and glossier.
- **Motion:** up to 32 guide strands of five points (Verlet), pinned to the head, pulled toward
  the style by their stiffness (short hair stiff, long hair loose), dragged by the wind, kept
  out of the head; each card follows its nearest guide. Brows and beards stay put.

## Eyes (`eyes.rs`, `eye.wgsl`)
Two eyeballs (12 mm radius at 1.75 m, the cornea bulging 0.6 mm) set in the sockets the face
cuts, turning about their centres to the gaze; upper and lower lid shells with wet margins,
the upper lid following the gaze down and closing to blink; lashes on the upper lids.
`EyeMotion`: saccades every 0.4–2.6 s, blinks every 2–10 s (70 ms closing, 150 ms opening).
The iris (radial fibres, a lighter collarette, a dark limbal ring, the pupil) is seen through
the cornea shifted by the view; the sclera is warmer toward the corners and shaded under the
lid; the cornea has a sharp wet highlight and the sky in it.

## Drawing (`body.rs`, `people.rs`)
- The skin, hair and eye shaders share `person.wgsl` (the person's data) and take their view
  from a prefix: the preview's own camera, light and tone map (`person_preview.wgsl`), or the
  world's globals (`common.wgsl` + `person_world.wgsl`: the sun where the sky is open, the
  sky's and the fire's light by the light levels where the person stands, aerial perspective),
  into the scene's HDR target after the terrain.
- `people::Person` keeps a person's meshes for their appearance, fitted garments and detail,
  building new ones on a worker thread one at a time (the last ones, or the boxes, drawn
  meanwhile). In first person the head and neck fold into the chest.
- The creator's preview shows the sculpted person, breathing, hair and eyes moving.

## Limits and what comes next
Recorded in `PLAN.md` (From E7): face morph targets for expressions and lip sync, teeth and
tongue; detail normals (pores, creases); hair alpha-to-coverage, self-shadowing, wet clumping,
growth and cutting; cloth simulation; foot IK on uneven ground; impostors far away; and the
GPU budget (a close-up person within about 1 ms at High) measured on the owner's PC.
