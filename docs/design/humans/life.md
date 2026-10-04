# The life course (V2.1 §7, §14.3; H3)

How the simulated people pair, conceive, are born, grow, age and die, and how their numbers hold
over generations. Written with H3 and extended part by part.

## The demography engine (H3 (a))

A people's **life table** is data (`humans/life/tables.ron`, schema `LifeTable`):

- **Deaths by age** — a Siler hazard `a1·e^(−b1·x) + a2 + a3·e^(b3·x)` a year at age `x`: the
  infant and child deaths falling away, a constant adult risk, and ageing's rising one. The
  foragers' is Gurven and Kaplan's (2007) composite hunter-gatherer fit (e0 about 31, 57 % alive
  at fifteen, the modal adult death about seventy).
- **Fertility** — a paired woman's chance in a month of a conception carried to term, by age
  (linear between the ages listed): at its height in her twenties, falling steeply after
  thirty-five and gone by forty-six; while she nurses a child younger than `nursing_years` it is
  multiplied by `nursing_factor` (lactational amenorrhoea spaces births). Gestation is the
  species profile's. Twins and a mother's death in childbirth are chances per birth.
- **Pairing** — the ages women and men first pair.
- **The land** — the persons a square kilometre feeds well (`density`), how crowding past that
  tells (`crowding`: children's deaths multiplied by `1 + children·(c − 1)`, conceptions divided
  by `c^conception`, where `c` is how many times more of its kind live within 15 km of a band's
  home than that land feeds, never below 1), and the size at which a band splits.

`People::live_course` lives every band lived in full of a species with a life table a
fifty-second of its calendar year at a time, the band furthest behind first so neighbours keep
pace. Each step:

1. **Deaths** by each member's age (children's raised by crowding). The dead come off the band's
   list; a partner is free again; kin mourn (kinship reckoned six generations back, which is
   exact for anyone closer than second cousins).
2. **Pairing**, about monthly: the band's unpaired women from the first pairing age to 45, eldest
   first, each with the unpaired man of at least the men's pairing age nearest her in age (at most
   15 years apart) who is not close kin — sharing no parent or grandparent, nor being one — first
   in her band, then in the bands of her kind within 40 km, nearest first. Where they are of two
   bands, the one of the dispersing sex goes to the other's, with his or her children not yet
   grown. Pair bonds are abstracted (V2.1 ground rule 3): a `Paired` event, a partner's id.
3. **Conception** for a paired woman whose partner lives in her band, by her age, nursing and the
   crowding; **birth** when it is due: a newborn (or twins) of her band beside her, its father
   her partner, its genome the meiosis of theirs (`endow`) and its psyche formed; `Born` and
   `Bore` in the life histories; a mother's death in it now and then.
4. A band past its split size **splits**: its households (a pair with their unpaired children,
   one alone) go one by one, every other, to a new band set 8–16 km off, which keeps its culture,
   its knowledge of places and its ease with the players.

A dormant band's numbers live on in the ecological cells (as before); one met again is reckoned
from when it is met. Life courses come to the bands of *Homo sapiens* with Wild Earth's families
later in H3; until then our species is data, and the test adds it to its own species set.

### Calibration (`hearth_people/tests/demography.rs`)

Six bands of thirty foragers twenty kilometres apart, lived 200 years (under a second):

| Measure (cohorts born years 30–100) | Run | V2.1 §14.3 target |
|---|---|---|
| Life expectancy at birth | 30.5 | low-to-mid thirties |
| Alive at fifteen | 0.57 | high infant and child mortality |
| Of those past fifteen, to sixty | 0.38 | many live into their sixties and seventies |
| Children to a woman who lives to 45 | 5.1 | four to six |
| Years between births | 4.1 | about three to four |
| Growth over the last sixty years | ~0.1 % a year | near zero |
| Band size | ~33 | twenty-five to fifty |

The numbers fill the country in about a century and then hold, about a fifth above what the land
feeds as each band feels it (bands' 15 km neighbourhoods overlap). Crowding works mostly through
fewer conceptions and a little through children's deaths, so life expectancy stays near the
table's own; fertility responds weakly to fecundability (nursing sets most of the interval), so
the conception exponent is strong.

## To come in H3

Child bodies and life stages with their animations; development (nutrition and stature,
attachment); ageing; inheritance; Wild Earth's families and the player born into one, growing up
at the childhood pace (Addendum A, `player-birth.md`); death as an event and its choices
(Addendum B, `life-after-death.md`).
