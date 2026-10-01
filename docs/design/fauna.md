# Fauna and ecosystems

*Status: in progress (V2-7). Species and ecosystems started in V2-0 (`fauna/`, `ecosystems/`).*

## Planned model
Real species on shared body plans with procedural animation; senses including wind-carried
scent; utility AI over behaviours (forage, flee, stalk, defend young, migrate…); herds and packs;
ecological cells (256 m) holding population densities and age/sex structure, advanced in
abstract time steps with predator–prey dynamics that stay stable through real mechanisms;
materialisation near the player and folding back; biogeographic realms; predators that attack
only for real reasons; tracking, hunting and butchering at real yields.

## Biogeographic realms
When a world is made its landmasses are found on the planet grid (8-connected land, wrapping
east–west). Those of at least 4 % of the land are continents, the rest islands. The continents
are ranked by area: the largest is the Old World (north of 23.5°, the Palearctic; between the
tropics, the Afrotropical), the next the New World (the Nearctic and the Neotropical), and so
on in turn (the Indomalayan tropics third). A continent's land south of 23.5° belongs to its
tropics' realm, or to the Australasian where it has none; south of 60° to the Antarctic. An
island within 1.5 % of the circumference of a continent belongs to that continent's realm, a
remote one is Oceanian; islands are poorer in species than continents. Every column of the
terrain knows its realm (`ColumnSample::realm`).

A species lists the realms it is native to (`realms`; none means everywhere). Where the
place's realm is its own it grows (or lives) as its climate and habitat allow. Where the realm
has no species of its own for the climate yet (a southern temperate forest, a Nearctic forest
on wet ground none of its three trees likes), the species of a stand-in realm fill the place:
the Palearctic's outside the tropics, the Afrotropical's in them. They are weighted down
(×0.02 against the natives) so that they show only where no native suits, and they are never a
mix of two realms. A species of neither the place's realm nor its stand-in does not grow there
(the Nearctic's sugar maple, white oak and paper birch only grow in the Nearctic).

## Species data (V2-7)
Each species carries, beside its size, speeds, senses, diet, society, activity, habitat,
seasons, danger and realms:
- **life**: age at first breeding, the greatest age commonly reached, the litter (or clutch,
  or eggs), births a year (below one, a birth every so many years), the usual birth day,
  yearly survival of adults and first-year survival of the young from what the simulation
  does not model (disease, accident, old age), and a newborn's mass;
- **ranging**: the area one group (or a solitary adult) lives in, whether it holds it as a
  territory, how far and which young disperse, how much it keeps to cover, and whether it is
  simulated as groups (large animals, by default from 5 kg) or as numbers per cell (small ones,
  birds, fish);
- **coat**: the recipe of its texture — back, belly, points, rump patch, pattern (plain,
  spotted, striped, grizzled, masked, speckled, pied, zig-zag, banded) and its colour, the
  winter coat, the male's colour where it differs, the young's pattern;
- **yields**: what butchering gives as fractions of the live mass (meat or fish, fat, organs,
  bone, hide or fur, sinew) and extras (antlers in their season, tusks, horn, feathers);
- **track**: the foot (cloven hoof, clawed or retracted paw, plantigrade sole, hopping feet,
  bird's toes, a snake's trail), a print's length and a walking stride;
- **calls**: kind (roar, bark, grunt, howl, hoot, song, drum, croak, rattle…), when (rut,
  alarm, contact, territory, distress, threat, dawn, night), loudness at 1 m and pitch;
- **temperament**: the distance at which a person makes it flee, boldness, and whether it
  freezes before fleeing.

The Tier 1 temperate forest set (Appendix B) has both northern realms:

| Role | Palearctic | Nearctic |
|---|---|---|
| Large deer | red deer | white-tailed deer |
| Small deer | roe deer | — |
| Wild cattle | aurochs | — |
| Pig | wild boar | — |
| Pack hunter | gray wolf | gray wolf |
| Bear | brown bear | American black bear, brown bear |
| Cat | Eurasian lynx | bobcat |
| Fox | red fox | red fox |
| Badger | European badger | — |
| Hare or rabbit | brown hare | eastern cottontail |
| Squirrel | red squirrel | eastern gray squirrel |
| Small insectivore | European hedgehog | — |
| Vole or mouse | bank vole | deer mouse |
| Woodland gamebird | western capercaillie | wild turkey |
| Owl | tawny owl | barred owl |
| Woodpecker | great spotted woodpecker | hairy woodpecker |
| Crows | common raven, carrion crow | common raven, American crow |
| Songbirds | European robin, common blackbird, common chaffinch | American robin, northern cardinal, black-capped chickadee |
| Trout | brown trout | brook trout |
| Perch | European perch | yellow perch |
| Frog | common frog | wood frog |
| Venomous snake | common European adder | timber rattlesnake |
| Honey bee | western honey bee | — (not native to the Americas) |

Freshwater species live in the `temperate_freshwater` ecosystem (rivers, lakes, wetlands);
their densities are per km² of water. A colony species' density counts colonies.
