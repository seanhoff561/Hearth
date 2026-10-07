# The Neolithic (V2-12, v2 Era 3)

The first farmers' and herders' crafts: pottery fired in its range, fields sown, tended and
reaped, wild grasses and wild sheep changed by the choices people make among them generation
after generation, and what herds give alive. This document covers what is built; the
society that grows up around it is H11.

## Pottery and kilns (V2-12 (a))

A pot is coiled from a clay lump and set to dry: unfired, it is *greenware*, and rain slumps
it back to clay (`Workshop::slake`). Fired, it becomes pottery only if it got hot enough for
long enough and not so hot that it slumped: `hearth_craft::firing::fired` takes the clay's
firing range (°C, from its material), the peak the batch reached, the hours it spent near the
peak and the hold the process asks for, and returns under-fired (still clay, said so),
over-fired (slumped) or fired with a quality from heat and soak. The heat is the station's own
fire as the fire model runs it, so a campfire kept fed brings earthenware to its range and
never kaolin; an updraft kiln does. Open firing cracks a share of pots in its uneven heat.

## Fields (V2-12 (b))

A plot is one tilled block of soil (`fields::Plot`): its nitrogen, its weeds, the crop
growing on it (`fields::Growing`) with the lot of seed sown. Crops are content: a plant with a
`crop` (`schema::flora::Crop`) names its grain material, the block drawn as it grows, when it
ripens sown in autumn or spring, its wild and domestic grain weights and yields, the water it
needs and what it draws from the soil.

- **When.** Winter cereals sown in autumn ripen the next early summer; sown in early spring
  they ripen later that summer for less. Sown in summer or deep winter, the seed is refused
  with the reason. The calendar is the world's, local to the hemisphere.
- **How much.** A ripe square metre bears the crop's yield between wild and domestic by how
  plump its grain has become, times the stand that came up (dormant seed lies in the ground),
  the climate's fit, water (the season's rain, or irrigation from water within four metres),
  the soil's nitrogen (drawn down by each crop, restored by a year's fallow and by dung),
  the weeds left standing, what birds and mice take of a crop left standing after it ripens,
  and a good or bad year.
- **Keeping.** Grain keeps in a storage pit; sheaves and grain in the open spoil a little each
  day.

### Domestication of grain

Seed carries its heritable makeup with it (`hearth_items::Lot`, on the stack): the share of
lines with a *tough rachis*, the mean grain weight and the share of seed that lies dormant.
Reaping, threshing and picking-over keep the lot; lots alike pour together, lots apart stay
apart.

- A wild stand's ripe ears shatter: reaped at ripeness some two thirds of their grain is still
  held, less each day after; tough ears hold theirs. The reaped grain is richer in tough lines
  than what was sown (Hillman & Davies 1990's sickle-harvest selection), and the richer the
  later it is reaped.
- Dormant seed is never reaped, so dormancy falls each generation it is sown and reaped.
- Picking out the plumpest third of a lot for seed raises its grain weight by the breeder's
  equation (selection intensity 1.09, a tenth's spread, half heritable); the rest is the worse.

The acceptance bot (`tests/acceptance_v2_12.rs`) tills eight square metres, sows wild einkorn,
manures, weeds, reaps with a flint sickle, threshes and keeps the plumpest third for twelve
harvests: its seed's ears come to hold their grain, it sprouts at once, the grain is plumper and
a square metre gives more.

## Herds (V2-12 (c))

Animals people can keep are those whose species has a `domestication` (its domestic form, and
what it gives alive: milk a day between a wild mother's surplus and a bred one's, fleece a year
between a wild coat's underwool and a bred fleece, the bred coat's colour). The first are the
mouflon (sheep) and the bezoar goat of the Fertile Crescent's hills, the aurochs (cattle) and
the wild boar (pig).

A kept animal (`hearth_fauna::herd::Kept`, on the live animal) belongs to no group or cell and
is saved apart from the populations' numbers. It has:

- **A makeup** (`Breed`): how docile, how woolly, how much milk, each 0 (the wild's) to 1 (the
  bred form at its fullest). A wild animal's are low and vary a little; a young one's lie
  between its parents' with a spread either way, so breeding only from the calmest and the
  woolliest moves a lineage along each generation.
- **How tame it is now**, learned: a young one handled from birth is tame whatever its makeup
  and grows into what its makeup lets it be — a wild-born lamb raised by hand turns wary as it
  grows, one of a docile line stays calm. Its flight from a person is its kind's times the
  square of what it is not tame; its readiness to turn on one, times the same.
- **A tether** (a stake and four metres of cord) or a keeper it follows, as a young one follows
  its mother.

Through the calendar (`Live::tend`) kept animals age into their stages (a docile line matures
sooner), settle toward the tameness their makeup allows, and breed in their kind's rut: a grown
female within sixty metres of a grown kept male of her kind is in young or not, the likelier
the more settled she is, and gives birth in her kind's birth season. Breeding is abstracted
(V2.1 ground rules): it is a chance and a due date, no more. Grown ones die of age at their
kind's lifespan. Births and deaths near the player are told.

What a person does with them (processes aimed at a live animal, `Target::Animal`):

- **Catch** a wild young one whose mother is gone (it stands and calls rather than runs) to
  raise by hand; a grown wild one twists free.
- **Tether** a kept one to a stake where it stands; **lead** it off on a halter (the stake is
  pulled up).
- **Milk** a mother with young at foot once a day into a vessel, if she is tame enough to stand
  for it: her kind's milk by her makeup, falling through her months in milk. Milk warmed by the
  fire curdles into **cheese**, which keeps for weeks where milk keeps a day or two.
- **Pluck** the fleece as it loosens: a year's growth by its makeup.
- **Slaughter** one for its meat: it lies as a carcass to butcher.

Predators leave a herd alone while its keeper is beside it; a tethered animal is watered.

The acceptance bot (`tests/acceptance_v2_12_herd.rs`) takes up eight orphaned mouflon lambs,
tethers them as they grow, plucks each grown sheep once a year and keeps to breed only the
calmest and woolliest eight ewes and ram, slaughtering the rest: the flock's grown sheep born in
its later years are calmer than the wild-born founders grew to be, and their fleeces heavier.

## Spinning and weaving (V2-12 (d))

Flax pulled with its roots (its stems the fibre's source) is laid in water to ret for ten days,
then broken, scutched and combed: a kilogram of stems gives some 150–200 g of fibre and its
shives. Fibre is spun on a spindle — a shaft through a clay whorl — into yarn, a skein of 150 g
in four hours (wool spins a little faster); a warp-weighted loom (two uprights and a beam, the
warp hung with stone weights) weaves a skein into a length of cloth, a metre by half a metre:
linen of flax, woollen cloth of wool. Two lengths sewn with yarn, a needle and a flint edge make
a tunic, three a cloak (`tunic`, `cloak` garments: cloth's warmth, lighter than hides).

## Timber and houses (V2-12 (e))

A tough stone pecked and ground for hours on wet sandstone and hafted makes a polished axe that
chops by its hardness rather than a flint's edge, and an adze across the haft hews and hollows.
Wedges driven along the grain split a log into four to six planks. Timber framing adds hewn
posts and beams (mortise and tenon, pegged) and plank walls and floors to the building pieces
(V2-8): with wattle and daub and thatch, the longhouses of the first farmers.

## Moving loads (V2-12 (f))

What is dragged is held back by the ground (`drag_friction`): sliding on the bare ground, less
on a sledge's runners (and very little on snow), rolling on a handcart's wheels (little on hard
ground, much in sand, mud and snow). A dugout, hollowed from a log with fire and the adze over
days, is dragged to the water; once it floats (the water knee deep) its owner sits in it and
paddles (`Ability::boat_m_s`, the mover's boat mode): dry, at about 1.8 m/s for a strong
paddler, until it grounds or the bank is stepped up onto. A potter's wheel (the slow wheel, a
disc on a pivot stone) throws a pot in a fraction of coiling's time, more evenly. Ox-drawn carts
wait for draft animals (V2-14).
