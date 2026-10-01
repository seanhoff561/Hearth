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

## Populations (`hearth_fauna`)
The land is divided into **ecological cells** of 256 m, gathered in **regions** of 64 × 64
cells (16 km) that are simulated near the player, saved, and caught up when visited again. A
cell knows its **habitat** (`habitat.rs`): its shares of land, fresh water and sea, its
ecosystems, its realm and the realm of the animals that live there (its own, or the stand-in's),
its climate, its cover, and the usable production of each kind of forage — graze, browse, mast,
fruit, seeds, invertebrates, fungi, nectar and the invertebrates of fresh water — from the Miami
model of net primary production shared out by the vegetation (`expected_canopy`): grass where
the canopy lets light through, browse where young trees grow back after a clearing or a fire,
mast under nut trees old enough to bear (their species' yields), fruit at the edges. The amounts
are anchored to what the reference wood's community eats at its usual densities, so that food is
just enough at what the habitat holds. Each kind has a season (grass and twigs standing through
the winter thinner, nuts falling in autumn and lasting into spring, fruit in its weeks), snow
buries what lies on the ground, every year has its weather (how well plants grew, how hard the
winter was), and nut trees mast heavily one year in three over a wide area.

Large animals live in **groups** that keep their members — young of the year, older young,
females and males, a condition, a home and where they are today: a herd of red deer, a family of
roe, a sow's sounder, a wolf pack, a lynx with her kittens. Small animals (hares, squirrels,
voles, birds, frogs, fish, snakes, bee colonies) are **numbers per cell**: young and adults with
a shared condition. A step of the simulation (an eleventh of a month by default):
- **Hunting**: predators take prey by a functional response whose attack rate is calibrated so
  that each meets its need at its prey's usual numbers (a specialist — a snake — at a fifth of
  them). It is of type III for generalists (they turn from a prey grown scarce to others), and
  prey that keeps to cover is partly hidden. The weak and the young are taken first; fish take
  frogs only as tadpoles. A large kill's remains are carrion.
- **Feeding**: everyone eats what the hunt did not give them from the forage and carrion in its
  range, shared out cell by cell when there is not enough. An omnivore short of meat eats more
  plants; a hunter can only partly (an owl in a vole-poor year lives on worms, thinly).
- **Condition** moves toward what the food allows (fat on all of it, thin on nine tenths,
  starving on half); hibernators live on their fat; the cold-blooded burn little.
- **Deaths** from what is not simulated (the species' background survival), from hunger, from
  winter, and from disease and stress where a kind crowds past what the land about it holds
  (judged over its home range). The dead feed the scavengers.
- **Births** in the species' season, fewer from mothers in poor condition, and only to the
  holders of a territory where the species keeps one; the young grow up, fewer where the place
  is already full.
- **Dispersal**: the young of the dispersing sex leave to settle within their species' distance
  where there is room (territorial ones away from others' homes; pack animals pair up), in this
  region or the next; crowded herds send some mothers and young to nearby land with room (which
  is how the deer come back to land emptied by hunting); the edges of the simulated land take in
  animals as if the land beyond held its usual numbers.
