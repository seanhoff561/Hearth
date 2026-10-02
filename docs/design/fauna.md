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

## Animals in the world (`hearth_fauna::live`, the server's `fauna.rs`)
The server keeps the regions about the player (the 3 × 3 about the one the player is in, made
one at a time as the player comes near, or restored from the save where they were simulated
before) and advances their populations with the calendar, every few days of game time, in steps
of at most a thirty-second of a year. Near the player the populations become animals:
- A **group** whose home is within 112 m comes into the world as its members — the young of the
  year, the older young, the females and males — placed about its home on ground they can stand
  on (the loaded blocks; the generated heights where the blocks are not loaded yet).
- The **small species** that walk (hares, squirrels, voles, mice, hedgehogs, snakes, frogs) are
  drawn from the numbers of the cells within 80 m, the share of each cell within reach, at most
  three to a cell, the same way each day.
- When all its animals are beyond 150 m a group **folds** back: the living into its numbers by
  age and sex, the dead (killed by the player, so far) not at all. Small animals go back to
  their cells the same way.

What they do in the world is the plainest life for now: grazing and wandering about the group,
resting through the hours their kind sleeps (diurnal, nocturnal, crepuscular or about the clock),
fleeing a person who comes within their species' flight distance, standing alert after. They walk
and run at their species' speeds, step up and down what their legs allow and turn back from
water and drops; their minds (senses, scent on the wind, needs, herds and packs), their bodies
and their gaits replace this in parts (d)–(f). The server tells the client of the animals near
the player ten times a second (`ToClient::Animals`: each one's species, stage, sex, place,
facing, speed, what it does and its stride's phase); the client eases them between and draws
them within 160 m. A census of the groups about the player (`ToServer::Census`) serves tools and
tests. The populations are saved with the world (`fauna.json.zst`: the regions' cells, groups
and weather, the animals in the world folded into them).

## Bodies (`hearth_fauna::rig`, `anim`, `skin`; `hearth_texgen::coats`)
Each species is a skeleton of slots with boxes on them, at its real dimensions, built from its
body plan and the proportions of its `shape` (neck, head and snout, tail and its thickness,
ears and how they stand, legs, a hump, and what grows on the head: antlers with their beam and
tines, horns with their curve, tusks), every one of which defaults by plan, so that a new
species needs only data. Four-legged bodies have a torso of two halves (bending at the middle
of the back), legs of two segments and a foot, a neck, a head with a snout and a jaw, two ears
and a tail of up to three segments; the torso is as long as the length leaves after the head
and the neck, as deep as the plan's share of the shoulder height and as wide as holds the
animal's mass (a male's toward the top of the species' range). Birds have a body and breast,
a short neck, a head and beak, a fanned tail, two wings of an arm and a hand, and two legs;
snakes and fish are chains of segments; frogs sit on folded legs; insects are a head, thorax,
abdomen and wings. A young animal is its parents' body, smaller.

The bodies move by gait data rather than clips: the gait follows the Froude number of the
speed against the hip's height (walking, trotting, galloping; hares and small rodents bound;
bears walk and gallop), the stride's length after Alexander's relation (λ = 2.3 h Fr^0.3),
each foot down for its share of the stride and lifted through the rest, and the leg bent to
reach the ground under it (two segments: the foreleg's middle joint bends forward, the hind's
hock back). The body pitches to the slope between the fore and hind feet and comes down where a
leg cannot reach. The head nods with the walk, turns to what the animal watches, goes down to
the ground to graze or to water (the forelegs splaying); the jaw chews; the ears stand forward
when alert, lie back in flight and flick; the tail sways with the gait, flicks, and rises in
alarm (a squirrel's curls over its back); the chest breathes, slower in bigger bodies. The
poses of doing things blend in and out: lying with the legs folded under, asleep with the head
turned back along the flank, grooming, rearing on the hind legs, lunging or butting. Birds hop
or walk, peck, crouch, tuck the head asleep and fly with their wings beating (fewer beats in
bigger birds); snakes wind along their path or lie coiled, the head raised in alarm; fish swim
with a wave down the body; frogs sit and hop. A stag carries his antlers in the seasons of his
antler yield and grows them through the season before; a boar has tusks; horns are on both
sexes or the males as the data says.

Coats are textures. Every species' boxes are unwrapped side by side into one atlas (each box
six faces in a cross, at sixteen pixels a metre for a deer and finer for small animals so that
a vole has a face), and `hearth_texgen::coats` paints its coats there from the recipe — the
female's, the winter coat's, the male's where his colour differs, and the young's where they
wear a pattern of their own — every pixel from where it lies on the body at rest: the back and
the paler belly with the countershading line up the flanks, the throat, the points (muzzle,
ear rims, tail tip), the legs, the rump patch and the tail's underside, the face, eyes and
nose, hooves or paws, and the pattern (spots, stripes, grizzled hair, a badger's mask, speckled,
pied or barred feathers, a snake's zig-zag or bands, a perch's bars, a trout's spots). The
figure shader reads each box's faces from the atlas by its unwrap's place, so the bodies stay a
few hundred boxes for a herd.
