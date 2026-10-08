# Fauna and ecosystems

*Status: implemented (V2-7). Species and ecosystems started in V2-0 (`fauna/`, `ecosystems/`).*

## Model
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
mix of two realms. The understory's plants, whose odds are absolute (a plant on a column or
none), are the stand-in's in full where the place's own would cover less than half what they
would (D115). A species of neither the place's realm nor its stand-in does not grow there
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
  spotted, striped, grizzled, masked, speckled, pied, zig-zag, banded, a flank band) and its
  colour, the winter coat, the male's colour where it differs, the young's pattern;
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

Freshwater species live in the `temperate_freshwater` and `tropical_freshwater` ecosystems
(rivers, lakes, wetlands, D145); fish densities are per km² of water, and an animal whose lands
are the waters' alone (a beaver, an otter, a heron, a hippopotamus) lives on the share of a
cell's land the water gives it, all of it where a seventh of the cell is water (D148). A colony
species' density counts colonies. A species of the warm lands can name the coldest month it
bears (`min_coldest_month_c`, D146).

The expansion waves (V2-10) bring each biome's own ecosystems, with species of both northern
realms (a realm without its own takes the stand-in realm's, D115): the boreal forest and the
tundra (moose, reindeer, musk ox, wolverine, Canada lynx, the hares, lemmings and ptarmigans,
sable, marten, ermine, the great grey and snowy owls, the arctic fox); the temperate grassland,
steppe and prairie (American and European bison, wild horse, saiga, pronghorn, the prairie dog,
bobak marmot and souslik, the common and meadow voles, coyote, corsac and swift foxes, the
American badger, steppe polecat and black-footed ferret, great bustard, sage-grouse, burrowing
owl, golden eagle, meadowlark and skylark, the prairie rattlesnake); the cold and hot deserts
(Bactrian camel, dromedary, onager, addax, dorcas gazelle, the jackrabbit, kangaroo rat and
jerboa, the fennec, kit fox and sand cat, the roadrunner, the western diamondback and horned
viper, the Gila monster and the desert tortoise); the savanna and the tropical rainforest of
three realms and the Mediterranean scrub (wave c). Africa's savanna: the bush elephant, giraffe,
zebra, wildebeest, buffalo, kudu, hartebeest, eland, impala, Thomson's gazelle, warthog and white
rhinoceros; the lion, leopard, cheetah, spotted hyena, wild dog and black-backed jackal; the
ostrich, guineafowl, secretary bird and white-backed vulture; the scrub hare, multimammate mouse,
banded mongoose, olive baboon, pangolin, puff adder, black mamba and leopard tortoise. Africa's
rainforest: the forest elephant, okapi, bongo, red river hog and blue duiker; the chimpanzee,
gorilla and guereza; the golden cat and crowned eagle; the grey parrot, pouched rat,
soft-furred mouse and Gaboon viper. The Americas': the tapir, peccary, red brocket, sloth,
agouti, paca, opossum and spiny rat; the howler monkey; the jaguar, ocelot, coati and harpy
eagle; the macaw and toucan; the iguana and boa; and in the cerrado the giant anteater, maned
wolf, rhea and bolo mouse. Tropical Asia's: the Asian elephant, gaur, sambar, chital and
muntjac; the rhesus macaque and lar gibbon; the tiger, dhole, clouded leopard and sun bear; the
hornbill, junglefowl and peafowl; the ricefield and spiny rats; the king cobra, python and rat
snake. The Mediterranean's: the rabbit, red-legged partridge, Iberian lynx, wood mouse,
Montpellier snake and Hermann's tortoise, with the boar, red deer, fox, badger, wolf and golden
eagle of the woods and steppes; the chaparral's mule deer and California quail, with the coyote,
bobcat, jackrabbit and deer mouse. The mountains (wave d), whose alpine lies above every
mountain's tree line (D134), a realm's ranges one community (D140): the Old World's ibex,
chamois, marmot, snow vole, bearded vulture, chough, nutcracker, snow leopard, bharal, tahr,
argali, wild yak, chiru, kiang, plateau pika, snowcock, Tibetan fox, musk deer and monal; North
America's mountain goat, bighorn, hoary marmot, pika, golden-mantled ground squirrel,
white-tailed ptarmigan, Clark's nutcracker and cougar; the Andes' vicuña, guanaco, taruca,
mountain tapir, pudú, mountain viscacha, leaf-eared mouse, culpeo, spectacled bear and condor;
Ethiopia's gelada, Ethiopian wolf, giant mole rat, walia ibex, mountain nyala, klipspringer, rock
hyrax and Verreaux's eagle; with the golden eagle, foxes, wolves, bears, wolverine, ermine,
hares, ptarmigan, deer, lynxes, capercaillie, squirrels, raven, moose and leopard of the lands
below. A pack's home range is its share of the land, its size over the species' density (D139).
The waters (wave e): the north's beavers, muskrat, otters, mink and water vole, mallard, geese
and swan, herons, stork, crane, osprey and kingfisher, pike, carp, wels, eel and catfish,
snapping and pond turtles, bullfrog and toad; Africa's hippopotamus, Nile crocodile, sitatunga,
fish eagle, shoebill, tilapia and catfish; South America's capybara, spectacled caiman,
anaconda, giant otter, marsh deer, jacana, piranha and arapaima; tropical Asia's water buffalo,
mugger, gharial, smooth-coated otter, fishing cat, sarus crane and snakehead.

## Populations (`hearth_fauna`)
The land is divided into **ecological cells** of 256 m, gathered in **regions** of 64 × 64
cells (16 km) that are simulated near the player, saved, and caught up when visited again. A
cell knows its **habitat** (`habitat.rs`): its shares of land, fresh water and sea, its
ecosystems, its realm and the realm of the animals that live there (its own, or the stand-in's),
its climate, its cover, and the usable production of each kind of forage — graze, browse, mast,
fruit, seeds, invertebrates, fungi, nectar and the invertebrates of fresh water — from the Miami
model of net primary production shared out by the vegetation (`expected_canopy`), growing with
the warmth and, where the climate has a dry season (the savanna's winter, the Mediterranean's
summer), with the rains (D124): grass where
the canopy lets light through, browse where young trees grow back after a clearing or a fire,
mast under nut trees old enough to bear and under conifers their cone crops (their species'
yields), fruit at the edges; where the summers are cool, part of the open ground's growth is in
dwarf shrubs, browse rather than grass (up to half on the low-arctic tundra, D108), and where
the rain is under 400 mm in desert shrubs (D117). The amounts are anchored to what the reference
lands' communities eat at their usual densities, each kind by the land whose animals eat the
most of it (the steppe's grass, the desert's seed and shrubs, the wood's mast), so that food is
just enough at what the habitat holds (D117). Each kind has a season (grass and twigs standing
through the winter thinner, nuts falling in autumn and lasting into spring, fruit in its weeks),
snow buries what lies on the ground, every year has its weather (how well plants grew, how hard the
winter was), and nut trees mast heavily one year in three over a wide area.

Each ecosystem names its **reference land** — the climate, canopy, young growth and the mast and
fruit of its trees that its animals' densities describe, and the realms whose animals live on it
(`reference` in `ecosystems.ron`; the temperate wood for an ecosystem without one) — and a
species' density is that of the richest of the reference lands of the ecosystems it lives in
(`habitat`, D116); it may live in others besides at what their land gives it against those
(`also_in`: the wild boar of the oak woods in the scrub and the monsoon forests, D128). Each
realm's community on a reference land anchors the forage by its own needs (D127). How well a cell **suits** a species
(its quality, against that land, which sets the numbers it holds): for a plant-eater its plants,
for a hunter the meat its prey of the realm offer about the cell at their usual numbers there,
with its plants for their share of its food, less where it lacks the cover it keeps to, and for
a cold-blooded animal of the land less again where few months are warm (a frog needs months
above 8 °C a tenth of the year, a snake or a lizard above 10 °C a sixth, a third of the year for
full numbers, D109, D119).

Large animals live in **groups** that keep their members — young of the year, older young,
females and males, a condition, a home and where they are today: a herd of red deer, a family of
roe, a sow's sounder, a wolf pack, a lynx with her kittens. Small animals (hares, squirrels,
voles, birds, frogs, fish, snakes, bee colonies) are **numbers per cell**: young and adults with
a shared condition, the adults crowded against what the cell holds and the young among
themselves by what they eat. So too the large that lay their young by the hundred and lose all
but a few, the sea turtles: counted as a group's members, a clutch's hatchlings crowded out the
grown. A step of the simulation (an eleventh of a month by default):
- **Hunting**: predators take prey by a functional response whose attack rate is calibrated so
  that each meets its need at its prey's usual numbers (a specialist at a fifth of them: a
  cold-blooded hunter, which lies in wait, or one with a prey or two of its realm and no plants,
  as the great grey owl with its voles, D119). It is of type III for generalists (they turn from
  a prey grown scarce to others, against the prey's usual numbers there), and prey that keeps to
  cover is partly hidden. Where the land holds fewer of a hunter's prey than their usual numbers
  (the tundra, the taiga), the hunter ranges the wider (its attack rate rises with the shortfall)
  while the land holds the fewer of it, so that a wolf of the tundra meets its needs on reindeer
  as a wolf of the oak woods does on deer, and there are fewer of it (D109, D116). A hunter eats
  at most half as much again as its need in its hungriest season (a snake its year's food in the
  warm months). A generalist's catches go by its want in grown prey (a young one, caught the
  more readily, is only part of a meal), the rest found among its other foods; a specialist takes
  as many young as its fill needs (D120). The weak and the young are taken first; fish take frogs
  only as tadpoles. A large kill's remains are carrion.
- **Feeding**: everyone eats what the hunt did not give them from the forage and carrion in its
  range, shared out cell by cell when there is not enough. An omnivore short of meat eats more
  plants; a hunter can only partly (an owl in a vole-poor year lives on worms, thinly).
- **Condition** moves toward what the food allows (fat on all of it, thin on nine tenths,
  starving on half); hibernators live on their fat; the cold-blooded burn little.
- **Deaths** from what is not simulated (the species' background survival), from hunger, from
  winter, and from disease and stress where a kind crowds past what the land about it holds
  (judged over its home range; a herd alone where the land holds less than one ordinary herd is
  not crowded by itself, and lives or starves by its food). The dead feed the scavengers.
- **Away**: migrants winter elsewhere — away, neither eating nor eaten, while their place lies
  frozen hard (the month below −5 °C), coming back with the thaw (D110).
- **Births** in the species' season, fewer from mothers in poor condition, and only to the
  holders of a territory where the species keeps one; the young grow up, fewer where the place
  is already full.
- **Dispersal**: the young of the dispersing sex leave to settle within their species' distance
  where there is room and the land is a fifth as good as the best about (territorial ones away
  from others' homes; pack animals pair up), in this region or the next (D118); crowded herds
  send some mothers and young to nearby land with room (which is how the deer come back to land
  emptied by hunting); the edges of the simulated land take in animals as if the land beyond
  held its usual numbers.
- **The sea** (V2-10 (f), D151): a species whose ecosystems are all the sea's lives on a cell's
  sea and is counted per km² of it. The sea's small life (plankton, krill, shellfish, weed and
  seagrass) is its aquatic forage, richest on the cold shelves, a third as rich over the deep,
  poorer under the ice; the sea's realms are its coasts', the polar seas one about each pole
  (D152); a mangrove's place is half sea, its animals living on its mud (D159).

## Animals in the world (`hearth_fauna::live`, the server's `fauna.rs`)
The server keeps the regions about the player (the 3 × 3 about the one the player is in, made as
the player comes near on worker threads, three at a time, the player's own first, each with a
copy of the populations' tables and ids of its own, and taken in when made; or restored from the
save where they were simulated before) and advances their populations with the calendar, every few days of game time, in steps
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
- Animals asleep for the winter (a bear in its den) or away (a snow bunting in the south) are
  not met about.

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
tines, horns with their curve and the way they sweep — out and forward as a cow's, up and back
as an antelope's or a wild goat's, round the ear in a ram's curl or up in a kudu's corkscrew,
D141 — tusks), every one of which defaults by plan, so that a new
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

The sea's bodies (V2-10 (f), D153): a seal's and a sea turtle's limbs are flippers (short legs
hidden in the body, broad paddles, the fore pair splayed, a seal's hind pair trailing); a
penguin is the bird's body pitched up eighty degrees on short legs, waddling, lying on its belly
and swimming flat with its flippers beating; a whale is round with a fluke across its tail that
beats up and down, its back fin's height its `hump` (of its length) and its flippers' length its
`legs`; tusks hang down from a walrus's jaw and run ahead from a narwhal male's; a crab stands on
four legs a side with its claws before it, a male fiddler's one great claw folded across.
Swimmers keep their depths (D154): a river's fish halfway down, the sea's within its sunlit
top, a whale with its back just under the surface.

## Finding the way (`hearth_fauna::nav`)
Animals go where they are going along ways searched on the loaded blocks a column at a time
(A*): a body steps to a neighbouring column if the ground there is within the step it climbs or
drops (its species' climb), and not in water deeper than it wades (three fifths of its shoulder
height) unless it swims; steps between two diagonal columns only where both sides are open, so
that it does not cut between trunks; water costs more than dry ground, swimming more again. A
search is bounded (1500 columns to walk somewhere, 700 to run away; six searches a step at the
most, the others going straight until their turn), and goes as near as it found when it cannot
reach; the way is then drawn straight wherever the ground between allows. A step it cannot take
after all (the ground changed) sends it looking again a second later.

- **Walkers** run from a person within their flight distance along such a way, and swim where
  the water is deep — the back just out of it, the legs paddling, the head up.
- **Climbers** (squirrels, the black bear) make for the nearest trunk within twelve metres and
  climb it to a good part of its height, watch from there, and come down when the danger is
  long gone; trunks, limbs and foliage are known by their blocks' names.
- **Birds** big enough to see as they go about (crows, ravens, owls, capercaillie and turkey)
  forage on the ground, perch on the tops of trees (owls roosting there by day), and take
  wing — from a person, to roost, down to forage: a flight goes up as steeply as the trees
  near its start ask, level a few metres over the highest trees on its way, and down to a
  perch on a crown or to the ground.
- **Fish** (trout, perch) are drawn from the stream cells into water deep enough, keep to it,
  wander and dart away from a person at the edge.

The client draws each one in its medium: swimming, climbing (the body upright against the
trunk, the feet gripping it), perched or on the wing.

## Minds (`hearth_fauna::mind`)
An animal senses a person by sight, hearing and scent, each raising its suspicion at a rate
that grows as the person comes nearer within the sense's reach; with nothing sensed the
suspicion fades over a quarter of a minute or so.
- **Sight** reaches its species' distance in daylight and its night vision's share of it in the
  dark, within the field its eyes watch (all about but behind it for prey, ahead for hunters),
  as far as the person is plain: upright and moving in the open plainest, still, crouched or
  crawling less, among plants less again; the ground and trunks between hide them, foliage and
  tall plants half hide them, every half metre along the line of sight.
- **Hearing** reaches the species' distance for a person running over dry leaves, and as the
  square of the noise for quieter going: a walk on grass a third as loud is heard a ninth as
  far, a crouched step on moss hardly at all. The noise comes from the person's gait and the
  ground underfoot, as the player hears their own steps.
- **Scent** is carried by the wind: downwind it reaches far (the farther as the wind is brisker,
  up to the species' distance), upwind not at all, all about but near in a calm.
- Very close, an animal is startled whatever it sensed.

A little suspicion makes an animal stop, look up and watch toward where it believes the person
is; full suspicion makes it aware, and an aware animal runs when the person is within its flight
distance (shorter for the bold), along a way, up a tree or on the wing as it goes. One that
hides (the roe deer) keeps still instead until the person is within half that distance. The
first of a herd to run warns the rest within a hundred and twenty metres, which run with it.

At its ease, an animal chooses what to do next by how much each suits it now (a utility AI), a
little at random: grazing; looking up from it now and then (the less as its herd is bigger);
grooming; wandering about its group's middle (the more as it has strayed from it); going to the
nearest water within a hundred and twenty metres when thirsty (about once a day) and drinking at
the bank; a young one following its mother when it has fallen behind. The weights are the
species' `habits` (vigilance, grooming, roaming, sociability), defaulting by its diet and how it
lives; out of its hours it sleeps. So from downwind a stalker gets within a hundred metres of a
grazing red deer that would scent them at three hundred from upwind, and crouched and slow
within a hundred where running is heard at three hundred.

## Danger (`hearth_fauna::danger`)
Animals turn on a person only for a reason a naturalist would give, weighed once an encounter
(again after fifteen seconds) by an animal aware of the person, startled, or a snake underfoot:
- **Defending young**: a mother whose young (within 25 m of her) the person comes within her
  guarding distance of (30 m for a bear, less for smaller beasts).
- **Defending a kill**: a hunter feeding at its kill, approached within 25 m.
- **Surprise**: a big animal (25 kg and more) startled close by, too close to get away first.
- **Cornered**: one running that finds no way on, the person within five metres.
- **Rut**: a grown male in the season of his rut, within twenty metres.
- **Hunger**: a hunter of people (wolves, bears) in a lean season (leanest at winter's end),
  the person seeming small — crouched or crawling, in the dark, hurt — and not by a fire.
- **Stepped near**: a venomous snake the person steps within a metre of.

How likely each is goes with the species' aggression, scaled by the world's Predator Behavior
setting (Authentic; Wild two and a half times; Tranquil a seventh) and lessened by what the
animal has learned to fear of people. Most turn with a charge: a hunter stalks low and slow
until within twelve metres; then the rush. A defender's charge closes on the person only some
of the time (a bear's less than a third; a boar's more than half); otherwise it stops short, a
bluff, stands a moment and goes. Closing, it strikes as its body does — a bear knocks the person
down and mauls them (a deep wound), a boar slashes the legs with its tusks, a stag or a bull
drives in head down (a puncture), a hind strikes with her forefeet (a bruise), a wolf, fox or
cat bites, a snake bites with its venom (envenomation), a bird flies at the head — and then a
defender goes, its point made, while a hungry hunter comes again.

The person can turn it. Fire keeps hunters off (light at the person's feet, from a fire or a
torch); facing an animal upright and loud (a shout, H) turns back a hunter at once and a
defender before long, and makes a defender likelier to stop short; backing off from what it
defends ends its charge; running from a hunter of people sets it after them. An animal turned
back fears people more after, for a good while. Every charge and blow comes with its reason in
words ("The brown bear charges and stops short: you came too near its young.") and in the log.

Hunters hunt their prey in the world too: a hunter at its ease may go after the nearest of its
prey within 150 m, stalking low and slow (quiet, half hidden), then rushing — a cat from seven
metres, briefly; wolves from thirty-five and long, the pack joining the first of it to hunt.
Prey sense hunters as they sense people (a stalker in cover downwind is hard to sense, a rush is
heard at once), and bodies get up to speed as they do (six metres a second every second, more
for a hunter's spring or a small body's dart), so an ambush from close in cover succeeds and a
long chase of a faster deer seldom does, unless it is a calf (the young of the year run at
seven tenths of their kind's speed). The prey killed lies dead and the hunter feeds at it.

## Carcasses and butchering (`hearth_content::butchery`, the server's `fauna.rs`)
Carcasses are generated from the species' data when the content loads, not listed. Every species
whose butchering yields are known and whose grown female weighs a quarter of a kilogram or more
(a red squirrel; songbirds, mice, frogs and insects are eaten whole or not at all) has a carcass
of a grown animal; a grown male's too where the sexes differ in size (the species'
`dimorphism`, how much heavier a male is: a red deer stag half again a hind, a capercaillie cock
twice a hen, owls' males smaller) or where the males carry what the females do not (antlers,
tusks); and a young one's, at two fifths of a grown female, where that is still worth working
(mammals and birds). The species' mass range is of both sexes about its middle: a hind of 132 kg
and a stag of 198, a calf of 53. Bodies are built at the same masses.

Each carcass has two ways of working it. Butchering proper — skinning and jointing a mammal,
plucking and drawing a bird, gutting and filleting a fish, skinning a snake — needs the
butchery knowledge and a sharp edge, practises the butchery skill, and gives the species'
yields as fractions of the carcass's mass: the meat (or fish), the hide (rawhide or a fur pelt),
the bone (not a fish's or a snake's), the edible organs (offal), the sinew, the fat by the season
(fattest in autumn, about three times what it is at winter's end) and what the animal carries:
antlers from a stag in the seasons he has them (cast in spring), a boar's tusks, an aurochs'
horns, a bird's feathers. Hacking at it without knowing how needs only an edge, wastes half the
meat, the hide and the sinew, and teaches butchery when done. The work grows with the animal
(0.15 h + 0.045 h·kg^0.8: a hare in a quarter of an hour, a roe deer in forty minutes, a red deer
in two and a half hours, an aurochs in a day; birds four fifths of that, fish half), heavier work
over 50 kg, and dulls an edge more the bigger the animal. The outputs of a carcass are what is
left of it (its `condition`: a kill the hunters ate from gives less), and what goes off comes
out as far gone as it was (meat from a carcass a few warm days dead is as near spoiled).

Carcasses come from deaths. In the world: a hunter's kill lies where it fell, the hunter (and the
pack about it) having made a meal of it — twice a day's need each — and the dead are taken from
the animals at once to lie as carcasses. In the populations: where a large animal dies in the
abstract step — taken by a hunter, or of age, hunger, winter or crowding — its remains are kept in
its region (`Remains`: species, age and sex, where and when, how much was left, why), each lasting
as its size allows (half of it gone in a day or two for a piglet, a week for a red deer, more than
a fortnight for an aurochs; gone after a month at most). Remains within 90 m of the player come
into the world as carcasses, gone off as their days in the air's warmth make them; by day, ravens
circling over remains less than four days old within 2 km tell of them ("Ravens are circling to
the north-east."), again each hour. A uniform wood holds about 150 remains in a region of 256 km²
at a time, most of them wild boar (whose numbers turn over fast). The client draws a carcass as
the animal lying dead on a flank, legs straight, in its coat (`dead` on the screenshot `animal=`
key). Tests may force a natural death (`ToServer::Die`).

## Hunting and wounds (`hearth_fauna::wound`, the server's throws and thrusts)
A thrown thing flies as before (no drag; points every 20 ms), now strays from the aim as the
thrower's practice allows (about three degrees for a novice, half a degree with practice: the
`throwing` skill, practised by every throw), and strikes the first animal its flight passes
through. A blow (the primary action aimed at an animal, or at nothing; E2,
`docs/design/controls.md`) lands at the end of its wind-up along its path (a spear's thrust
along the look within the arm and the spear's 2.2 m, a swing's arc across the body, a kick from
the hip) and strikes the same way; edges cut (shallower than a point, long, the throat and a
leg's vessels and tendons first), and its momentum shoves the body struck. A body is its
rig's torso (a box), the neck and head ahead of and above the chest, and the legs under it;
where the blow strikes is the part: the chest (the front of the torso, where the heart and
lungs are), the belly (the middle), the haunch (the hind part), a leg, the neck, the head.

A blow carries its energy (half the mass by the speed squared for a throw: a stone-tipped
spear thrown hard about 190 J; a thrust with a spear 150 J, a blow with something in the hand
40–130 J) and the weapon's sharpness (`piercing`). A sharp one goes in 0.25 m × piercing ×
√(E / 100 J) and reaches the vitals if that is more than three tenths of the body's width (a
hare's 3 cm, a red deer's 9, an aurochs' 22: a wooden point does not reach an aurochs' heart
where a stone one does). Reached, the heart and lungs bleed out within about twenty seconds (a
red deer runs some 140 m first); the neck sooner; the belly slowly (it runs, lies up and dies
within a quarter hour or so); the haunch now and then cuts a great vessel and otherwise lames
and bleeds; a leg lames; the head kills. Short of the vitals, a flesh wound bleeds a while and
clots (never to death alone). A blunt blow (a stone) kills a small animal struck hard on the head
or body and stuns or bruises a large one. An animal that has lost two fifths of its blood falls
dead; before that it goes slower the more it has lost and lies down when it can go no further.
Hurt, it knows the person (aware, running), fears people more after, and a boar, bear or other
defender hurt and come close upon turns on them (`Provoked`). An animal killed by a person lies
as a carcass where it fell, whole, and the person is told it falls if they are within sight.

## Tracks and signs (`hearth_fauna::live`, the client's `signs.rs`)
Animals within 80 m of the person leave signs: a print at each stride where the ground takes
one (fresh snow plainly, sand and bare earth well; grass, moss and leaf litter not), either side
of their line as their feet fall; drops of blood where the wounded go (the faster they bleed, the
closer the drops); droppings now and then (a plant-eater's about a dozen times a day, a
hunter's a couple). Prints last a day and a half (in snow twice that), blood a day, droppings
ten days; the most recent 6,000 are kept. The client draws them on the ground — a print shaped by
the foot in the species' `track` data and as long as its print, dark in earth and a blue-grey
hollow in snow; blood bright when fresh; pellets or scat — and, where the eyes rest on one, says
what it is: tracks, blood, droppings; and to one who knows tracking (learnt by noticing fresh
prints underfoot, `see:tracks`), whose, how old and which way they went ("Red deer tracks, a
few hours old, going north-east"). Screenshot key: `trail=species:n:prints|blood|droppings@ahead:right:yaw`.

## Calls and ambient life (`hearth_fauna::voices`, `hearth_audio`'s `Sound::Call` and crickets)
Each species' `calls` data (a kind, an occasion, a loudness at a metre, a pitch range and a
length) is what it says and when. In the world an animal calls: the alarm of one that starts to
run (half of them: a red deer's bark, a white-tailed deer's huff), the distress of one struck or
caught (a hare's scream, a boar's squeal), the threat of one that turns on a person (a bear's
huff, a snake's rattle or hiss), and those of habit while their occasion holds, at rates as the
animal lives them second by second whatever the calendar's pace: a stag in the rut about once a
minute at dusk, by night and at dawn; a herd's contact calls; a wolf pack's howl a few times an
hour by night, taken up by every wolf of the pack within 200 m; an owl's hoot; a fox's bark. The
animals not in the world are heard too: the small birds of the cells within 200 m sing, the males
of each cell's numbers, every ten seconds or so at dawn (a fifth as often through the day) in
spring, less in summer, hardly in autumn and winter, never in the dark — the dawn chorus; owls
hoot by night; woodpeckers drum on spring mornings; and from as far as 3 km the packs howl by
night and the stags roar in the rut, where their groups are (only calls of 95 dB or more carry
so far).

The client hears each call as loud as its species makes it, 6 dB fainter each time the distance
doubles and half a decibel fainter every hundred metres for the air, from the way it came, and
not at all below the wood's own quiet (−66 dB of full scale). The sound is made from its kind:
a roar a long rough bellow of low harmonics and breathy noise falling away, a bellow a lower
held note, a bark a sharp yelp, a huff a blast of air, grunts low and short, a squeal a rising
high note, a howl rising, held and falling, a growl a low rumble, a hiss high noise, a hoot a soft
note and a quavering run, a song quick notes up and down its range (the same notes for its kind),
a caw two harsh calls, drumming a roll of seventeen blows a second, a croak a pulsed low note, a
scream a falling cry, a gobble a pulsed note, chatter quick clicks, a rattle a dry whirr, a buzz a
hum. Captions name what is heard ("Birdsong", "Howling", "An owl hoots").

On warm nights from midsummer into autumn crickets sing (a bed of six, each chirp three pulses of
a 4 kHz note), chirping as often as the warmth makes them — Dolbear's law for the tree crickets,
seven chirps a minute for every degree less thirty — and not in the rain, the cold or a cave.
Birds and insects seen about (flocks crossing the sky, insects over flowers) are not yet drawn.

## Acceptance (V2-7)
`crates/hearth_fauna/tests/acceptance_v2_7.rs`: the 3 × 3 regions about the spawn of seed 7 (a
patchy land on the borders of four realms: woods, open plains, sea and fresh water) run for fifty
years in quarter-year looks, every species the land holds ten of or more stays between a
twentieth and four times what its habitat holds and is never gone for long (most hold near
their capacity; the red fox about half, the timber rattlesnake about a tenth, food-limited in
this land); a roe deer population hunted by half every year for six years falls to a sixth and
comes back within twelve years of the hunting's end; and in seven kinds of encounter run a dozen
times each — a sow with her cub come upon, wolves at the end of winter with the person crouched
alone in the dark, a boar surprised, an adder walked over, a boar wounded and come up on, a stag
in the rut walked up to, a hind and calf walked past — every attack says its cause in words, and
each cause only where it can hold (hunger from hunters, defence of young from mothers with
young, a snake's bite when stepped near, provocation when hurt, the rut from males).

Two flaws of the populations showed in the fifty years and were mended: the small species' young
died at the first year's rate however many years they took to grow (a timber rattlesnake takes
seven), so slow-growing species dwindled — now the first year's rate and then the half-grown's of
the groups, the mean over the years they are young; and crowding killed a territory's grown
holders as readily as its young, so a pair on a patch too small for its litter died with it —
now crowding falls on the young and the half-grown, as density does in the wild. A predator's
prey is reckoned over the prey's own habitat in its reach (not the land of another realm beside
it), and a predator whose home is on another realm's border hunts as it does in its own.
