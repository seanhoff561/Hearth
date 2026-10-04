# Tiers and persistence (V2.1 §17; H7)

How the simulated people are kept as the player moves: lived in full near the player, kept whole
as households within reach of the player's life, counted as numbers everywhere else — and how
they move between these without losing who they are. Written with H7.

## The tiers (§17.1)

| Tier | Who | How they live |
|---|---|---|
| **Full** | bands about a player (drawn out within 112 m, kept until all are beyond 150 m) | moment to moment: bodies, minds, work, speech |
| **Household** | bands whose home is within 40 km of a player, or that hold someone a player knows (one who has met a player, or a player's kin) | their records kept whole; their lives lived by the life course — births, deaths, pairing, households, ties, culture, language, learning — week by week |
| **Dormant** (demographic) | everyone else | their numbers in the ecological cells' groups, their records waiting |

- **Full to households**: a band whose persons are all beyond 150 m of every player goes on as
  households when its home is within 40 km of one or it holds someone a player knows; otherwise
  it is folded back into the cells as before. Its quarrels end; its persons keep everything.
- **Households to full**: a band lived as households whose camp (or home) comes within 112 m of
  a player is lifted back into full about it — the same persons as their lives have gone while
  away, set down about the place, knowing what lies there, their bodies refreshed to their sizes
  if they were away more than a day. No reconciling with numbers is needed: their course was
  lived.
- **Households to dormant**: beyond 40 km and holding no one a player knows, a band's numbers go
  back to the cells.
- **Dormant to full**: as before H7 — a group drawn out near a player is either woken (its old
  records reconciled with the cells' numbers) or **instantiated**: founded with coherent families
  — each young one given a mother of the band old enough to have borne it and not past bearing
  then, her births a year and a half apart at least — and its grown founders given **forebears**:
  by ones, twos and threes close in age, a mother who died before the band was met, kept as a
  genealogy stub, so a band met for the first time has brothers and sisters among its grown.
  Fathers are found as the genomes are drawn.
- A save keeps every band lived in full as households, whole for the player's return.

## Budgets and pruning (§17.1–17.2)

- At most **300 persons lived in full**: past it the bands farthest from every player go on as
  households (never a player's own), and none is drawn out or lifted while it would pass it.
- At most **20,000 as households**: past it the farthest that hold no one a player knows are
  folded back into the cells.
- **Pruning**: about yearly, the records of those dead more than forty years (not a player's)
  are cut to genealogy stubs — id, parents, sex, dates, name and the outline of their lives
  (born, paired, bore, died) — their genomes, minds, knowledge, memories, ties and things let go.

## The acceptance (H7)

`tests/persist.rs`: a band left as households for five years and come back to is the same
persons, five years older or dead in their time, with those born meanwhile to its mothers, its
ties kept, back about the place; a band met for the first time has coherent families and
brothers and sisters among its grown; past the budget, the farthest band goes on as households
and the nearest stays in full; the long dead are pruned to stubs that keep their dates.
