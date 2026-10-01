# Fire, cooking, preservation and food

*Status: implemented for Eras 0–2 (V2-5, D72). Code: `crates/hearth_craft/src/fire.rs` (the
thermal model), `food.rs` (nutrition, spoiling), `crates/hearth/src/workshop.rs` (fires in the
world, unattended work, eating and drinking).*

## Purpose
v2 §11.4–11.5: fire is hard to come by and must be kept; it lights, warms and cooks; food keeps
or goes off by what it is and how it is kept, and cooking changes what it gives the body.

## Model
### Fires
- A **fire** is its fuel (pieces with mass, heat of combustion, thickness and wetness), its
  coals, and whether it is flaming, banked or a lamp.
  - Flame eats into wood at 0.8 mm a minute from every side of the part of each piece in the
    hearth (40 %): dry grass flares for seconds, a 2.5 cm stick lasts most of an hour, a 40 cm log
    section half a day. A fire takes a few minutes to take hold of new fuel.
  - A seventh of the wood is left as coals, which glow on for an hour or two, light dry fuel laid
    on them, and banked under ash smoulder for half a day. A bed of coals (30 g or more) lights a
    stick; a few dying embers light only kindling (twigs, a centimetre thick or less), which
    then lights the rest.
  - Heat given off: what burns × its heat (18 MJ/kg dry wood, 29.5 charcoal, 39 fat). A handful of
    sticks gives 10–30 kW; coals alone about 1 kW. The hearth's temperature rises towards its
    build's limit (a campfire 800 °C) with the heat, following within a minute or two.
  - Rain soaks fuel; a fire giving less than twice the heat the rain takes to boil off (about
    a third of a kilowatt per mm an hour on a hearth) drowns, and its coals hiss out. Wind feeds
    it. Wet fuel must dry first (the fire dries what lies in it) and will not catch above 40 %.
  - About a quarter of the heat leaves as radiation: the body beside it gets 0.25 P / 4πd²
    (about 100 W/m² at a metre from a 5 kW fire).
  - A **lamp** burns fat on a wick at 6 g an hour (about 65 W of flame): 0.1 kg lasts about 17 h.
- **In the world**: hearths and lamps are workstations; their block shows the fire's state
  (out, banked, embers, low, high) in its shape and light (embers glow 5, flames 12–15).
- **Getting fire** (Eras 0–1): lightning in a storm now and then sets a tree burning
  (a natural fire of a hundred kilograms that burns for hours); a burning stick carried from it
  or an ember nursed in tinder fungus lights a laid fire. Friction comes later, by hand drill or
  bow drill, at a fire laid with tinder, and fails often in damp air or with a poor drill.
- **Keeping fire**: feed it (a piece of wood), bank it for the night, take an ember from it.

### Cooking and preserving
- Roasting at the fire (200 °C), cooking greens on a hot stone, stone-boiling with hot cobbles
  and carried water; fire-hardening a spear point; birch tar under the ash; resin glue.
- Drying meat on a rack (72 h of dry weather; rain stops it and spoils batches), leaching
  acorns in running water (24 h), pemmican from dried meat and fat. Unattended work lies where
  it was set up and goes on while its conditions hold.

### Food
- Materials carry their **nutrition** per kilogram (protein, fat, carbohydrate, water,
  fresh-food vitamins, a risk such as `raw_meat`), from food composition tables for wild foods.
  Eating one unit (a cut, a handful) gives its share to the body; the stomach must have room.
- **Spoiling**: a material keeps `keeps_days` at 20 °C (raw meat 2, cooked 3, dried 180,
  pemmican 365, berries 3, nuts months), 2.5 times faster for every ten degrees warmer and as
  much slower colder, nearly not at all frozen, faster wet; drying slows it to a quarter.
  Food more than half spoiled risks food poisoning, more the further it has gone; a carcass
  rotted through is gone to the scavengers.
- **Drinking** a quarter litre from water looked at (its salinity and pathogen risk from the
  hydrology), or from a carried water skin; a skin is filled at water.

### Kills (stand-in)
Until animals live in the world (V2-7), a predator's kill turns up 30–120 m away every one to
two and a half days (while fewer than two lie within 250 m); ravens circling say which way.

## Parameters
`REGRESSION_M_H` 0.048, `IN_FLAME` 0.4, `CHAR_SHARE` 0.15, `COAL_MJ_KG` 29.5,
`RADIANT_SHARE` 0.25; coal glow 0.6/h (0.3 flaming, 0.06 banked); hearth follows in 0.02 h;
lightning ignition 0.2 × thunder × (1 − min(rain/10, 0.8)) a minute.

## Interactions
Body (radiant heat, food, water, illness, burns), weather (rain, wind, humidity), light (block
emission), items (fuel from wood by size, embers that burn down), knowledge (fire keeping,
cooking, drying).

## Known simplifications
- Fires do not spread (wildfire comes with flora, V2-6); natural fires are single trees.
- Item temperatures are not tracked: hot stones are only hot at the fire.
- Smoke is not drawn yet; smoking meat is drying by the fire.
- Kills are a stand-in for predators and hunting.

## Future extensions
Earth ovens, kilns and furnaces with airflow and insulation (V2-12, V2-13); charcoal clamps;
hot items that glow and burn.
