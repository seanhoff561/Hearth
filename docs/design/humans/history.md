# History: deep time and the recent past (V2.1 §15.1–15.2; H8)

How a world's peoples come to be where the player finds them: a fast abstract simulation of deep
time over the planet's own geography, run once when the world is made, and a century of the
recent past lived household by household about the place a life begins. Built with H8
(D191–D198); `hearth history` runs it from the command line and prints its chronicle.

## The deep-time layer (§15.1)

### The grid

The deep past is lived on a **history grid** laid over the planet's map (`hearth_people::history`):
square cells about 8 km across, never fewer than 32 nor more than 128 along the circumference (a
Standard planet's 65 km make 32 cells of 2 km; an Earth-sized one's are 300 km). Each cell reads
the planet at 4 × 4 points (`Geography::of_terrain`): the ground's height at each (so its share of
land follows the sea's level, and glacial shelves and land bridges come out of the sea), the
climate (the year's and the coldest month's temperature, the rain), the biomes' worth to foragers
(data), its realm and landmass, whether a river runs through it, and its sunlight.

### The clock and the climate

Years count back from the present. A run begins with the first of the era's peoples to appear
(*Homo erectus* 1.9 million years ago for the Paleolithic eras) and ends at the era's date. Steps
are 250 years before 200,000 years ago, a century to 60,000, and a generation (25 years) after;
slower things — splits, losses, the pools' drift — are reckoned every 500 years. The ice ages are a
stylised sea-level curve (data, `humans/history/settings.ron`): 41,000-year cycles before 900,000
years ago, then 100,000-year cycles whose glacials lower the sea about 120 m, and for the last
150,000 years the record's own outline (the last interglacial, the build-up of the glacial, the
Last Glacial Maximum 26–19 thousand years ago at about −125 m, the melt). Temperature follows the
sea (0.05 °C a metre: −6 °C at the glacials' depth) and so does rain (4 % a degree). The world the
player walks in is generated at today's climate and sea level: the era's ice age acts in deep time
only (D195).

### Peoples as demes

Every cell holds, for each species living there, a **deme**: a number of people (continuous, not
persons), a lineage (a culture with its language), a gene pool and what it knows. A deme lives by
its cell's **capacity** for its species:

- The species' base density (`deep.density`, a km² of open savanna–woodland to people knowing
  nothing that wins more food), times the biome's worth to foragers (data: open savanna,
  Mediterranean scrub, wetlands and oases richest; temperate forest and grassland less;
  rainforest, boreal forest and tundra poorer; deserts and ice nothing), times the share of the
  cell that is land at the step's sea level.
- Water's foods: a coast or a river adds by what is there to gather (shellfish, stranded fish,
  eggs) and more by the fishing gear the deme knows (harpoons, hooks, nets, weirs).
- What its techniques win from the land (data `food`: digging sticks, cooking, butchery, spears,
  snares, nets, bows, drying...): a Middle Paleolithic people about 0.06 a km² of open country, an
  Upper Paleolithic one about 0.12.
- The cold: a species winters where the coldest month is no colder than its body (`cold_c`: a
  Neanderthal bears a winter 8 °C colder than ours) and what it knows allow — bare and fireless to
  about 10 °C, with fire kept, made, hide wraps, sewn clothing and needles each lower. The ice
  ages push peoples out of the north and let them back.

Each step a deme grows logistically toward its share of the land and sends people to its eight
neighbours by how much room they have — a Fisher wave of the species' speed, twice as fast along
coasts and rivers, at most a cell a step where cells are small. Species in one cell compete for
it: each counts everyone against what the land would feed of each, so the better equipped need
less land a head and in the end crowd the others out (the Neanderthals' fate where *H. sapiens*
comes with the Upper Paleolithic's techniques, not before). Across water a deme reaches cells
within its **sea reach**: 12 km drifting on a log, 120 km with rafts (the knowledge graph's
`raft`); land bridges open and close with the sea.

On a planet smaller than a people's mating network's country (some 25,000 km² of land: the few
hundred among whom its bands find partners, Wobst 1974) its peoples live denser by as much, at
most thirty times (`denser`, D196, D198): a liberty, as Wild Earth's families are, so that a
Standard world holds such a network of each of its era's peoples and no band is left alone to die
out. Numbers for invention, keeping and drift are reckoned as
Earth's: a deme's people as its share of its species' on the planet, times the species' numbers at
their height on Earth (`deep.people`) — the planet stands for Earth.

### Where peoples begin

Each species appears at its time (`first_appearance_ya`) by its origin (`origin` in its profile):
*H. erectus* in the cradle — the best savanna–woodland of the Afrotropical realm — and the later
species out of the populations already living in a realm: Neanderthals from the *erectus* demes of
the Palearctic 400,000 years ago, *H. sapiens* from those of the cradle 300,000 years ago. A
species past its `extinction_ya` is gone.

### Gene pools (§4.5)

A deme's pool (D192) is the sunlight its physical frequencies are adapted to (moving toward its
cell's own over about ten thousand years), a drift for each place-shifted locus or trait (a logit
offset; a trait's loci share one) that wanders by how few the deme is and is undone over a hundred
thousand years, and a share of another species' ancestry gained where two live together. Migrants
mix pools by their numbers; founders of a new cell add their own drift. A band drawn from a deme
draws its founders from its species' pool at the deme's adapted sunlight, moved by its drift
(`Genetics::founder_in`). Behavioural loci have no place here (ground rule 1).

### Cultures and languages (§9, §10.1)

Each deme carries a **lineage**. The lineages make a tree (D193): a lineage splits when its
connected people pass what one people holds (data: about 1,500 for our kind, after Birdsell's
dialect tribe; fewer for the others), the half farthest from where it began becoming a daughter,
and at once where a part is cut off by land (an island, a deserted gap) with enough people.
Neighbouring lineages build up contact. Deep time keeps only the tree: a lineage's culture and
language are made when a band of it is first met — drawn at its root on the root's stream, a
daughter at each split, a year's drift for each year of the branch (at most twenty thousand:
nothing older would tell), customs and words taken from its closest contacts. Bands of one lineage
share its culture; recent splits are visibly kin, old ones are strangers.

### Knowledge geography (§11.4)

A deme knows a set of techniques of its species' repertoire (D194: its profile's, or the era's
`repertoire` or `reach` of graph eras, within its cognitive ceiling). A technique is invented by a
lineage no earlier than the record's first date for it (`years_bp`), at a rate by its people;
spreads between neighbouring demes at about half a kilometre a year (less across lineages); and is
lost where those joined to the deme by land are too few to keep it, the deeper the more easily
(Henrich 2004): islands cut off lose their deepest techniques, continents keep them. A band drawn
from a deme knows its deme's techniques; the era's floor (`knowledge_baseline`) is every people's.

### The chronicle

Notable events are kept with their year and place: a species appearing; a people first reaching
each realm or landmass; a crossing of the sea or of a land bridge; the first invention of each
technique and its loss by a whole people; lineages splitting; two species meeting; a species
leaving a realm to the cold; a species gone. A census every ten thousand years keeps each
species' numbers and the sea's level. The Observer (H9) shows it; `hearth history` prints it.

### Running and keeping it

The deep layer runs when the world is made, after the planet, on the world's seed and era; the
result (grid, demes at the era's date, lineage tree, chronicle, census) is kept in the world's
folder (`history.json.zst`) and read after. It is deterministic (streams of seed, step and cell;
two-phase updates) and parallel: a Standard planet takes seconds, an Earth-sized one about a
minute.

## The recent-history layer (§15.2)

About the place a life begins, the bands of the era's peoples in the ecological cells within
reach (40 km, H7's household region) are founded at the era's date less a century — their founders
drawn from their demes' pools, their cultures their lineages', their knowledge their demes' — and
**lived as households** (H7's tier: births, deaths, pairing across bands, households, ties,
culture, language, learning, their seasonal rounds and gatherings, `eras.md`) for four
generations, a century, a year at a time (D196). When the player arrives the people already have
lives: genealogies four generations deep, the dead remembered as stubs, kin in the next band,
reputations, skills. A band so founded reckons its crowding by deep time's own density where it was
founded (D198). The ecological cells' groups of the era's peoples about the place are taken up by
these bands, so nothing is counted twice; beyond the region the era's peoples are numbers in the
ecological cells at their demes' densities (`Ecology::peopling`).

## Tests

- `hearth_people/tests/history.rs`: on a Standard planet *H. erectus* spreads from the cradle over
  its continent, and *H. sapiens* after it; on a made-up world of a mainland and an island across a
  strait the island is reached only within sea reach or over a land bridge the ice opens, and the
  cut-off few lose what the many keep; the same seed makes the same history; the recent past gives
  the people lives (the dead, the born, grandparents known) and a player is born among them.
- `hearth/tests/eras.rs`: births are offered in each Paleolithic era's world, among its own
  people.
