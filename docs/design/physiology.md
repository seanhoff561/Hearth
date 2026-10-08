# Body and physiology

*Status: implemented (V2-3: the physiology in `hearth_body`, D66; sleep and death with the
player in `hearth_player` and the server; what it tells the player in `hud.md`). Parameters
in `data/hearth/body/` (human, injuries, illnesses) and `data/hearth/clothing/`.*

## Purpose
The player's body as v2 §9 describes it: needs, heat and cold, sleep, stamina, injuries and
illness with real causes and real time scales, so survival is about food, water, shelter,
clothing and care rather than a health bar.

## Time
Needs, heat and short illnesses run on the **day scale** (v2 §4.2): a game day stands for a
real day, so a body needs a day's food and water each game day, a naked body in cold rain
becomes hypothermic in the real hour or two (2–3 minutes of play at the default 48-minute day)
and a night's sleep takes eight real hours (16 minutes of play). Each injury and illness heals
or runs on the scale its data declares (a sprain on the day scale, a fracture on the year
scale). Short-term **stamina** is measured in seconds of play, because movement is not
compressed. All conversion goes through `TimeScales`.

## Model
`Body::step(config, play_seconds, exposure, worn, activity)` advances everything together.

- **Reference body.** One adult whatever the character looks like (height and build are
  cosmetic, v2 §9.1): the middle of `human.ron`'s ranges — 70 kg, 1.73 m, 1.83 m² of skin, a
  basal rate of 24 kcal/kg/day (81 W).
- **Heat balance** (`thermal.rs`): a core and a shell of eleven skin regions, after Gagge's
  two-node model with the shell split by region. The core makes the metabolic heat (METs from
  `activity_met`, plus shivering) and holds the body's heat store; each region's skin settles
  where the heat reaching it from the core (tissue conductance 10 W/m²K at full
  vasoconstriction plus the skin's blood flow) meets what it loses: convection and radiation
  through what covers it (local clo, reduced by wind for permeable covers and by soaking), the
  evaporation of sweat, of rain that got through and of water diffusing through dry skin (skin
  vapour resistance 0.6 m²kPa/W, about 0.35 l a day), the rain it warms, water when immersed
  (150 W/m²K), the ground when lying (through bedding), and the sun's and fires' radiation
  reaching it through the cover. Warm and cold signals drive the skin's blood flow, sweating
  (Gagge's 170 g/m²h per °C) and shivering (19.4 W/m² per °C², under one ceiling of 4.5 METs
  shared with work, less on low glycogen, failing below 32 °C core); the hands and feet are shut
  off most in the cold unless the body is warm, so they grow cold while the trunk stays warm.
  Working muscles thin the shell's insulation (swimming in cold water cools faster than floating
  still). Fever raises the set point.
- **Food energy** (`energy.rs`): the stomach (2 l) empties with a 1.5-hour time constant
  (drink in 20 minutes); energy fills glycogen (28 kcal/kg, about a day's worth) then fat (at
  85 %); burning draws on glycogen in proportion to how full it is, then fat alone. The body
  starts with 20 % fat (≈ 108,000 kcal) and dies of starvation at 1.5 %. Protein's share of
  the energy absorbed over three days above 45 % weakens ("rabbit starvation"); a reserve of
  fresh-food vitamins (60 days at the start, up to 120) runs down by a day a day and its
  absence slows healing and weakens on the Authentic preset.
- **Body water** (`water.rs`): losses are sweat, insensible evaporation (skin and breath, from
  the heat balance), urine (1.5 l/day well watered, down to 0.5 l/day at a 3 % deficit, more to
  shed extra), faeces (0.1 l) and illness; gains are what is absorbed from the stomach and the
  water of burning food (0.13 ml/kcal). Salt costs the water to excrete it (23.3 g per litre
  of urine: seawater loses half a litre per litre drunk). Death at a 15 % deficit.
- **Sleep** (`sleep.rs`): Borbély's two processes — pressure building while awake (18.2 h
  time constant, faster with work) and draining asleep (4.2 h, slower when the sleep is poor:
  cold, wet, hard ground, pain, noise) — and the body clock's swing. `wakes` says what would
  wake a sleeper (cold, heat, wet, pain, hunger, thirst, disturbance, or rested: after four
  hours, once pressure and the clock together fall below 0.2 — about eight hours after a long
  day, toward morning).
- **Death and a new life** (Amendment E §6.6, the world's mode): a new adult begins near where
  the last life ended, or elsewhere, carrying only a loincloth; the body's belongings stay where
  they fell. What the new person knows is the mode's (`AfterDeath`): in Realistic only their
  own, the old journal readable as notes from a past life; in Easy what earlier lives
  discovered, at a beginner's skill. Creative cannot die.
- **Sleeping** (`hearth_player::Player::rest`, the server): the player lies down (Z); a body
  sleepy enough (0.3) and at ease drops off after 5 s of play; asleep, the world eases up to
  90× as fast (v2 §9.5: accelerated, not skipped: the weather, water and body go on), and
  slows back on waking. What wakes the player is said as they open their eyes; the eyes
  close slowly into sleep and open slowly out of it.
- **Stamina**: drains with effort above a third (an all-out sprint empties it in 15 s of play),
  recovers in 30 s at rest, slower when tired, weak or out of glycogen.
- **Injuries** (`harm.rs`): on a region and side with a severity; bleeding from the data's
  range by severity, clotting over minutes for small wounds and barely for deep ones, a tenth
  under pressure or a bandage; pain; infection rolled six hours after the wound unless it was
  cleaned (clean or boiled water, honey, resin); healing over the data's time × (0.5 +
  severity) on its scale, slower when starving, dry, cold, without fresh food, infected or
  unsplinted, faster asleep. Frostbite comes from skin frozen (below −0.5 °C) for ten minutes
  on the hands, feet or face.
- **Illness**: caught from causes (`bad_water` when drinking, `raw_meat`, `wound_infection`…)
  at the data's chance; onset and course on their scales; effects act on the body (fever,
  diarrhoea and vomiting as water loss, weakness); a course rolled fatal kills at its end unless
  any of its treatments was given.
- **Death**: core ≤ 26 °C or ≥ 43 °C, a 15 % water deficit, 40 % of the blood lost, fat gone,
  a fatal illness, or what the world reports (drowning, a fatal fall).
- **Effects** for movement and actions: walking speed, sprint, jump, two hands, grip, strength,
  sight, consciousness, shivering and sweating (for animation), pain.
- **Status** for the Body panel and the diegetic cues: hunger, thirst, warmth and tiredness as
  words (localisation keys `body.*`), core and skin temperature, blood lost, bleeding, sickness,
  stamina, wetness.

## Parameters
`body/human.ron` (body size ranges, basal rate, METs, stomach, water, temperatures, blood,
loads, speeds, stamina), `body/injuries.ron`, `body/illnesses.ron`, garments in `clothing/`
(clo where they cover), and the balance keys `hunger_rate`, `thirst_rate`, `fatigue_rate`,
`cold_stress`, `heat_stress`, `injury_severity`, `healing_rate`, `illness_chance` (D46).
Physical constants of heat transfer are in `thermal.rs`.

## Interactions
Weather and seasons (air, wind, rain, sun, sky) → heat balance; water sources (`Quality`:
salinity, germs) → drinking; food items and cooking (V2-5) → `Food`; clothing (V2-4) →
`Worn`; movement (V2-3) → activity, falls → injuries, swimming → immersion and drowning; fire
(V2-5) → radiant heat; shelter (V2-8) → rain and wind kept off; sleep → time acceleration (the
world loop); death → respawn rules (V2-3).

## Acceptance (V2-3)
`crates/hearth_body/tests/acceptance.rs`:
- Naked in 5 °C rain with a breeze: mild hypothermia (core below 35 °C) after 1.0 h of body time
  — 2.0 minutes of play; in furs (parka, leggings, mittens, moccasins) by a fire in the same
  rain the core holds at 36.4 °C.
- Without water: dead after 3.3 days of hot, active days (32 °C, walking in the sun), and
  after about 11 days resting in mild shade — as with people, heat and work decide.
- A moderate sprain heals in about 4 real days (3.7 game days, day scale); a splinted fracture
  in about 6 weeks (3.4 game days, year scale).

`tests/realism.rs` checks the rest against human data: a naked body at rest holds its core in
29 °C still air without shivering or sweating; ordinary clothes are comfortable indoors; 5 °C
water makes a body hypothermic in 42 minutes and kills in 3.6 hours; a resting day costs about
2,000 kcal, an active one about 3,400; starving with water lasts 46 days; walking three hours at
38 °C in the sun sweats 2.2 l; an untreated deep wound bleeds out in 13 minutes, pressed and
bound it costs a tenth of the blood; uncleaned punctures get infected at their rate and cleaned
ones never; seawater deepens thirst; a day awake tires and a warm night restores; bare hands
freeze at −20 °C in wind while fur mittens save them.

Movement and the body together (`crates/hearth_player/tests/living.rs`): a 4 m fall hurts
most people and 30 m kills; a sprint lasts 16 s and drops to a jog; a broken leg allows 0.4 m/s
and no jump; a diver who stays down drowns after 105 s; floating in 8 °C water chills the body
until it faints and drowns. Falls by height (`tests/realism.rs`): 2 m hurts 39 %, 3 m breaks a
bone in 24 %, 6 m in 78 %, 10 m kills 20 %, 15 m 68 %, 25 m all.

## Known simplifications
- One skin temperature per region (no left and right, no front and back); the radiant heat of
  a fire or the sun is spread over the whole body rather than its facing side.
- Clothing soaks as one: a body's wetness is one store of water on skin and clothing.
- No acclimatisation to heat or cold, no fitness, no age; one reference body for everyone.
- Shivering fatigue only through glycogen; no hypoglycaemia of its own.
- Infection is a single roll at six hours; illnesses of one kind do not stack.
- Bleeding is per wound with no shock physiology beyond blood volume.
