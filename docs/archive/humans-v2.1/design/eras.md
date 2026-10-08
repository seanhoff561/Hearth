> **Superseded by Amendment E** (`docs/spec/amendments-e-q.md`, 2026-10-08): the simulated humans were removed in E0 and are planned anew as Phase F (`docs/design/future/humanity/`). Kept for reference; nothing here is built, tested or linted.

# Eras: the Paleolithic worlds (V2.1 §15.3; H8)

What a world of an era holds and how its peoples live through the year: the era profiles and the
era selector, the archaic peoples, camps, seasonal rounds and gatherings, the births offered, and
life after death in an era's world. Built with H8 (D191–D199); the deep past that peoples an era's
world is [history.md](history.md).

## Era profiles

An era is data (`eras/eras.ron`, v2 §17.1 grown into V2.1 §15.3's profiles): its date, its sea
level and cold against today's (acting in deep time, D195), the knowledge every one of its people
starts with (`knowledge_baseline`), and its **peoples** — for each, a species and how its bands
live then:

| | Who | Bands | Knowledge | Camps | The year |
|---|---|---|---|---|---|
| **Wild Earth** | sporadic *Australopithecus*; a few wandering families of our kind (D164), as many as the players the world expects | families of 4–10 | their country's needs (D164) | — | — |
| **Lower Paleolithic** (400,000 years ago) | *Homo erectus* | 15–40 | the species' repertoire: hand axes, wooden spears, fire kept and carried | a kept fire | where the food is |
| **Middle Paleolithic** (45,000 years ago, −70 m, −3.5 °C) | Neanderthals in the cold north; *H. sapiens* spreading out of its cradle in the south; where they meet, both | 10–30; 25–50 | fire made, prepared cores, hafted spears, a little ochre | a kept fire, beds of grass or furs | Neanderthals winter in sheltered valleys and follow the herds up in summer; ours winter in shelter and summer by the water |
| **Upper Paleolithic** (25,000 years ago, −121 m, −6 °C) | *H. sapiens* across the world | 25–50, in peoples of about 1,500 | every technique of the graph's eras to 2: blades, bone and antler, needles and sewn clothing, nets and snares, ornament | a kept fire, beds | four camps a year (shelter, water, uplands, water) and an autumn gathering of up to six bands for twenty days |

The **era selector** (Create World) lists the playable eras first with how their people live; the
later ones (Neolithic, Bronze Age, Iron Age, and the planned) show as "coming later" until their
technology lands (H11–H13).

## The archaic peoples

- ***Homo erectus*** (`humans/species/species.ron`): long-legged and narrow-hipped (its body plan,
  `Appearance::plan`), a faster life than ours (D198), a culture generator of small egalitarian
  bands, a day of its own (`erectus_day`), and a **proto-language** (D186's generator gated by
  `kinds`): a few sounds, syllables of a consonant and a vowel, words for things, kin, pronouns and
  a few particles — no grammar to speak of — so its talk is mostly gesture.
- **Neanderthals**: short, broad and barrel-chested, wintering 8 °C colder than we can bare
  (`cold_c`), dying young but weaning early (D198), a hearth-centred day (`neanderthal_day`), full
  languages drawn with fewer vowels and simpler syllables, burial and care of the injured in
  their culture's data.
- Where Neanderthals and our kind meet, deep time gives the newcomers a share of the others'
  ancestry, and the better equipped crowd the others out in the end (D191).

## Camps

A band of an era's people that keeps a fire (`hearth`) and knows fire keeping has a **camp**: its
fire laid at the camp and lit from the embers they carry, fed with dry wood whenever it burns low
while its people are about it, and — where the era has bedding (`bedding`) — a bed for each
household in a ring a few steps out: heaped grass (as at Border Cave, 200,000 years ago), or hides
over grass where they know fur bedding. These are the same campfire and beds the player builds
(workshop stations), so they burn, smoke, warm and give bedding as the player's do. Its people
sleep at the camp and come back to it from their day. A camp the band has left keeps its hearth
and beds where they were, its fire burning out.

## Seasonal rounds

Where the era gives a people a **round** (`round`: a season and where camp goes then — by the
**water** (a river, a lake or the shore), up on the open **uplands**, down in the **shelter** of the
woods), each band moves camp when the season turns (its hemisphere's) to the best such place
within its range: the world reads its terrain about the band's home for it (`Lands`, the
`Country` trait) — water within a few hundred metres, height over the country about and how open
it is, how wooded and low — and keeps each band's places, so that a band comes back to the same
camps year after year, as foragers do. A camp is made on dry ground — a block over any water and
two over the sea's level, as are the few steps about it — and at least 800 m from other bands'
camps (a band whose places are all taken stays where it is); a band split off makes its own camp
so, near its new home, and finds its own round's places from the next season. A band lived in full
walks there; a band of the household tier is there; a band coming into full about a player sets
its camp on standing ground at its place (or a few steps off where a tree's trunk stands there). A
camp just made is given ten days before the band's council may find it poor (H4). A season the
round does not name keeps the camp where it is.

## Gatherings

Where the era has one (`aggregation`: the Upper Paleolithic's autumn gathering of twenty days,
bands coming 60 km, six at most), when its season comes the nearest bands of one people (its
species and lineage) within reach, as many as the era says — a few hundred people, as at the
aggregation sites (Conkey 1980; Kelly 2013) — gather at one camp by the water near the largest
band's home, each band at its own fire, thirty metres from the next in a ring about the host's
and on dry ground, once a year. The era's days are a real year's, so a gathering takes as long a
part of the game's shorter year (twenty days of 365 are under two of the game's 32). At a
gathering each one grown meets a few of the other bands' people (ties begun or warmed, H4);
pairing looks first among those met there, so marriages are made across bands (H3's residence
rules then move one of the pair); and a band learns from the bands it gathered with that year as
from its nearest neighbours (H6). Then each band goes back to its round's camp for the season, or
the camp it left. A people with no other band within reach stays on its round.

## Births in an era's world (Addendum A)

A new life in an era's world begins where one of its peoples lives well (not a thin frontier, nor
an empty land), and is born into one of the households there (D197): the recent past about the
place is lived first (history.md), then two to four households are offered — a living mother of
bearing age with no other child born within a year and a half, and the father, her partner in her
band — spread over as many species, peoples and bands as there are, told by who they are (names,
ages, brothers and sisters, grandparents, the band and its country, what its people know), never
how they look; the player picks one and a daughter, a son or chance. Where none lives about the
place (a small world holds few), the households nearest anywhere in the world are offered, and the
life begins at that band's camp. The player's body is their species' (an *erectus* or Neanderthal
plan), as are their childhood, size and coming of age.

## After death (Addendum B §2)

- **Live on as another**: any living person of the world lived in full or as a household — a
  child among them, whose remaining childhood is then lived from its age through its moments, safe
  throughout — chosen by the death screen's filters (*your family*, *your group*, *those near*,
  *anyone*) and told by who they are to the dead, a stranger by their household; never another
  player's character, nor one fighting, fleeing, badly hurt and dying, or just giving birth. The
  world's **scope** setting (`InhabitScope`) says whom: anyone (single-player's default), kin, group
  and region (multiplayer's), kin only, or no one (permadeath).
- **Be born again** as a baby (Legacy's death rules): about where they died, or anywhere picked on
  the globe — in an era's world into one of the households offered there (the recent past lived
  first where no band of the era's peoples is yet), in Wild Earth into one of its families; a
  daughter, a son or chance; the childhood lived from birth.
- Watching, beginning again and permadeath stay as H3 built them (H9 brings the rest).

## Tests

- `hearth_people/tests/rounds.rs`: a band's camps follow its round season by season and it comes
  back to the same places; the bands of one people gather in their season, meet and marry across
  bands, and go home.
- `hearth/tests/eras.rs`: births are offered in each Paleolithic era's world among its own people
  (an *erectus* in the Lower Paleolithic, our kind in the Upper), and a band about the player keeps
  its fire at camp.
- `hearth/tests/afterlife.rs`: a dead player lives on as a grown kinsman, or as a child whose
  childhood goes on from its age, or is born again into a household.
