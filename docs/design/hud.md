# What the body tells the player: senses, the Body panel and the Guided HUD

*Status: implemented (V2-3 part f). Code: `crates/hearth/src/client.rs` (senses, breath, the
Guided HUD, eyes), `hearing.rs` (the body's sounds), `crates/hearth_render/src/post.rs` and
`shaders/post.wgsl` (the image), `crates/hearth/src/body_panel.rs` (the Body panel).*

## Purpose
v2 §9.9: no numbers by default. The body says how it is through sensations a person would
have, and a Body panel says it in words for those who want to look. Bars are an option.

## Model
- **Sight** (the tone-mapping pass, before tone mapping):
  - exhaustion, parching thirst, starvation, blood loss and dimmed sight (`Effects.vision`)
    drain the colour;
  - fainting dims the light;
  - pain and blood loss close in the edges of sight, darker and redder as blood is lost,
    pulsing with the heartbeat;
  - cold greys the world toward blue (chilly to freezing);
  - heat makes it waver.
  - Dead, the world drains and darkens.
  - Asleep or fainting, the eyes close slowly; on waking they open slowly and say what woke
    the player.
- **Hearing** (`audio.md`): the heart pounds and the breath labours with exertion, blood loss,
  fever and cold water. An empty stomach rumbles every 40–120 s (25–60 s very hungry) and a dry
  throat swallows every 45–120 s when thirsty. Very weak (blood loss, dimmed sight), the
  world's sounds go far away while the heart stays close.
- **The body**:
  - shivering hugs the arms to the chest and shakes the view a little;
  - below 7 °C the breath fogs in front of the eyes as it goes out (thicker colder and damper,
    not under water or asleep);
  - lying down, the view is from the ground; asleep it is dark.
- **The Body panel** (B): the body drawn region by region. Injuries are marked by kind and
  severity, bandages and splints are drawn on. Beside it, the states in words: hunger, thirst,
  warmth, tiredness, breath, wetness, bleeding, pain, illness, and each injury with how far it
  has healed and what is being done for it.
- **The Guided HUD** (Accessibility, off by default): compact bars for food, water, warmth
  (full when comfortable, blue cold, red hot), rest, breath (stamina) and blood.
- **Reduce motion** (Accessibility) stops the pulse at the edges, the shiver of the view and
  the heat's wavering.

## Interactions
The body (`hearth_body` through `BodyView`): states, effects, blood, pain, shivering, vision.
The weather: the breath's fog. Hearing: the heart's rate sets the pulse at the edges and the
breath's rhythm the fog.

## Known simplifications
- Breath fog is drawn before the eyes in first person, not in the world (other people's
  breath comes with them).
- No sweat on the skin, no frost on the eyelashes, no blur.
