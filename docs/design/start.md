# The Wild Earth start

*Amendment E §6 (E5). Status: implemented. Code: `hearth_character::appearance` (how a person
looks, Randomize), `crates/hearth/src/character_ui.rs` (the creator), `profiles.rs` (saved
people), `places.rs` (suggested places), `birthplace_ui.rs` (the Birthplace screen),
`menus.rs` (the death screen), `server.rs` (waking, new lives), `hearth_env::calendar`
(`spring_dawn`).*

## Purpose
A new life in Wild Earth begins as an adult the player designs, at a place worth beginning in,
at dawn. There are no other people. Death leads to a new life or a new world.

## Who you are
- **Appearance** (`Appearance`) is stored as appearance and changes nothing a person can do:
  - body, height (1.45–1.95 m) and build;
  - skin tone (a continuous scale between ten swatches of the human range), undertone, freckles;
  - the face: six presets, then seven features (jaw, cheekbones, brow, nose, eyes, lips, ears),
    each −1–1 about the average;
  - eyes (seven natural colours);
  - hair: eleven styles, length, eleven natural colours and a fine picker (hue, saturation,
    lightness), eyebrows (four weights), facial hair for any body;
  - a name, and the loincloth's material.
- **On the current body model** (until E7): the face's features size and place its boxes (a
  broad jaw widens the lower face, high cheekbones stand out, a heavy brow lowers and juts),
  freckles are small dark marks over the nose and cheeks (their colour the skin's own, darker
  and warmer), and length lengthens the styles that hang.
- **Randomize** draws someone from adults' natural variation, keeping the name and body:
  - heights about the body's mean (1.63 or 1.76 m, sd 7 cm), builds about the average, each
    feature about the average face;
  - skin across the whole range;
  - lighter eyes (blue, green, grey) and fair hair only with the lightest skins, more often the
    lighter the skin, as the pigmentation genes act together;
  - red hair with freckles (MC1R) and freckles otherwise only with light skin;
  - few women bald, none bearded.
- **Profiles** are kept in `characters.json` (the profiles of before load as they were; the
  `birth.json` of E0–E4 gives a first profile its name, body and loincloth once).
- **The creator** is the last step of making a world (name and mode, the place, then who: E
  §9.1; its Done reads Begin) and opens from the death screen. It lists the profiles (New,
  Delete, Randomize, Done) and puts their looks in a scrolling column. Between them the person
  turns (drag) under daylight, overcast, dusk, firelight or moonlight.

## Where you start
After the planet is made, its making thread builds the world generator and suggests three to
five places (`Finder::suggest`).

- **Candidates:** 6,000 random land cells of the grid, 3–2,500 m up, not polar.
  - A cell is survivable at the starting date if its night before dawn stays above 2 °C and its
    afternoon below 40 °C (the climate normals for that day of the year: spring where the place
    is for a spring dawn, the real date for "today").
  - Each is scored for a mild day, a river and low ground. Coasts score no better than inland.
- **Variety:** the best place of each climate class first, at least 1,500 km apart, then the
  next best. A place whose verification fails is passed over.
- **Verification** (`Finder::verify`) reads the generated world about the spot:
  - the terrain at the spot and on 7 rings of 16 points out to 1.5 km (rivers, lakes, the sea, the land's
    relief and cliffs);
  - the springs of fresh water within some 750 m;
  - the surface cubes at 25 points out to 600 m, block by block: loose stones of a knappable
    rock, and the plants of the understory;
  - the trees standing at those points;
  - the fauna that the place's habitat cell supports (`ecology::lives_here`).
  - **Fresh water within 1.5 km is required.**
- **The card** shows:
  - **the name in plain words:** the land (coast, marsh, mountainside, valley, riverside,
    lakeshore, hills, plain), its cover (wooded, open, savanna), the rock where it shows, and
    the climate;
  - its climate and the season now, with the day's high and the low before dawn;
  - its biome and height, and a difficulty;
  - **what to look for**, each with its distance and direction: the water, the toolstone lying
    on the ground, the commonest trees, plants giving fibre for cord, food in season (edible
    parts whose seasons include it), tinder;
  - **what to watch out for:** predators (those at the water apart), venomous animals,
    large animals that defend themselves hard, cold nights (below 8 °C), heat (above 33 °C),
    thin air (above 2,500 m), salt water as the nearest.
- **Difficulty** adds up the cold or heat, the predators, the distance to water, the lack of food
  and the height:
  - Gentle: 0–1;
  - Challenging: 2–3;
  - Harsh: more.
- **The Birthplace screen** lists the places and shows the chosen card beside the globe.
  "Choose anywhere", or a click on the globe, picks any spot. Its card is then read from the
  world on the click; a spot with no fresh water near says so and may still be chosen.

## Eras
Every era is listed in the selector. The ones not yet playable read "Coming soon", are greyed
with their line, and cannot be created.

## Waking
- **The time:** a world's clock begins twenty minutes before sunrise on a spring day where the
  first life is (`Calendar::spring_dawn`), or today, now.
- **Lying awake:** the first life lies awake in the grass and the eyes open slowly. The spring
  dawn chorus of the birds about is the fauna's own.
- **Getting up:** setting off, or any act, gets up. New lives after death wake lying too.

## Death and a new life
- **The death screen** names who begins the next life and opens the creator. The new life
  begins at one of these:
  - near where the last one lived;
  - at one of the places suggested for the season it is then (found on the client's own thread
    once the player has died);
  - anywhere on the globe.
- **Restart the world** archives the world and makes it again.
- **The new person** looks as chosen (`NewLife` carries the appearance).
- **What a new life knows** is the mode's: Realistic its own only, Easy what was found.
- **The body left behind:** the dead body's things lie where it fell.
- **Creative** cannot die.

## Tests
- `appearance::tests`: an appearance round-trips, old ones load; Randomize correlates
  pigmentation as it is.
- `profiles::tests`: profiles save and load, and the wishes of before migrate.
- `tests/places.rs`:
  - the suggested places are 3–5, of 3+ climates, survivable, and every claim is found again in
    the world (about 3 s on a test planet);
  - the Birthplace screen draws them;
  - no human lives in the world;
  - by hand (`--ignored`), on an Earth-sized planet, 5 places in 7.8 s.
- `tests/server.rs`: the first life wakes lying and gets up when asked; a new life looks as chosen
  and wakes lying near where the last fell.
- `calendar::tests`: a spring dawn comes twenty minutes before the sun at four latitudes.
- `tests/menus.rs`, `tests/layout.rs`: the creator fits every window.

## Known simplifications
- The globe's hover shows the one-line description. A place's card is read on the click
  (some 0.3 s), not on hover.
- **Near a friend** waits for multiplayer (Phase R).
- The first-time hints of Easy are P6's.
- Rock shelters, tides and mudflats are not in the world, so no card claims them or any shelter.
- Fauna come from the habitat cell's suitability, not from a census of the animals placed.
- Plant names and the place's words are English (the language file has the keys; the content's
  names are not yet translated).
