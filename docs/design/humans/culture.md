# Culture (V2.1 §9; H5)

How the simulated people come to have ways of their own — how they organise their families and
their work, what they hold dear, how they lay their dead to rest, what they will not eat, how
they greet and treat strangers — how those ways pass from one generation to the next, change,
spread between neighbours, and part when a people parts. Written with H5 and extended part by
part.

## The model (H5 (a))

Every band carries its **culture** (`band.rs`, `culture.rs`): what H2–H4 gave it — the knowledge
its people hold, the techniques they practise, their traditions — and now its ways, drawn from
its people's **generator** (`humans/culture/generators.ron`, schema `CultureGenerator`):

- **Lineage**: its id (the band that first carried it), the culture it came from, and the day it
  began. A band that splits off, leaves after a feud or is cast out takes a **daughter** of its
  culture — the same ways, its own lineage — and from then on the two can drift apart (H5 (c)).
- **Values** along four of V2.1 §9.1's dimensions, each 0–1: *egalitarian ↔ hierarchical*, *kin
  first ↔ wider cooperation*, *conciliation ↔ honour* in conflict, and *loose ↔ tight* norms.
- **Social organisation**: where a new pair lives (with either's people as suits them, with his,
  with hers, or apart), how descent is reckoned (through both parents, the father, the mother),
  the share of men who may have a second wife.
- **Division of labour**: for each kind of work (each process skill, and gathering, hunting and
  child care), the share of it its women do — the only place a person's sex shapes what it does
  (ground rule 6).
- **Rites**: how the dead are laid to rest (buried; buried with what they carried; laid out away
  from camp; burned where fire is kept).
- **Food customs**: the foods it forbids.
- **Etiquette**: how it greets (an embrace, hands, a call from afar, a gift held out), and its own
  **ways with strangers and quarrels** — H4's, tilted by its values and a little at random: the
  more widely it cooperates, the more hospitable; the more it holds to honour, the sooner it
  warns strangers off; the tighter its norms, the slower it takes a stranger in.
- **Style**: the seed of its motif (the art comes with H8's eras).

The generator gives each trait its options with how often the world's peoples of that way of
life have them — foragers' now (farmers' and herders' with H11): residence mostly with either
spouse's people, else more often with the husband's (Marlowe 2004; Alvarez 2004); descent mostly
bilateral; polygyny allowed in most and practised by few men (Marlowe 2005); the division of
labour within Murdock and Provost's (1973) cross-cultural ratings (big game almost wholly men's
work, gathering, cooking and sewing mostly women's, fire-making and hide work mixed); values
about the egalitarian end (Boehm 1999); most burying their dead; a few forbidding fish (the
Tasmanians, the Hadza). A culture is drawn when its band is founded, on a stream of its own from
the world's seed and the band — the same each time, and moving nothing else drawn for the band;
bands met again from before cultures are given theirs.

## In what people do (H5 (b))

- Its **ways** replace its people's in everything H4 does with strangers and quarrels: how near
  a stranger is watched and met, the trust a greeting gives, hospitality, when strangers are
  warned off, when a guest is taken in, how long a quarrel's rungs last and when a feud parts the
  band.
- **Tightness** moves when the norms' sanctions begin: a name brings mockery, keeping away,
  food withheld and casting out the sooner the tighter the norms (the thresholds times
  `1.15 − 0.3 × tight`, the data's own at a middling half).
- **Honour** makes a quarrel likelier to rise (its chance times `0.65 + honour`) and its weaker
  side slower to back down (times `1.35 − honour`).
- **Residence** decides where a new pair of two bands lives — she goes to his band, he to hers,
  or either as suits them — instead of the species' dispersing sex.
- **Division of labour** weighs each work by whether it is the chooser's sex's: a work worth `w`
  to anyone is worth `w × (0.3 + 1.4 × share)` to one whose sex does that share of it.
- **Rites**: the dead are laid to rest as their culture has it (`LaidToRest` in their record); a
  people that buries its dead with what they carried passes nothing on to an heir.
- **Taboos**: a food its culture forbids is not eaten, unless starving; a place that offers
  only it is let go.

## Transmission and evolution (H5 (c))

- **Taking it in**: each month of the life course a band's people take its culture in — each
  one's values move toward what its temperament and its culture make of them together (six parts
  its own, four its culture's): honour from its culture's honour, deference and autonomy from its
  hierarchy, kin loyalty and generosity from how widely it cooperates. The young (under fifteen)
  move a tenth of the way a month, the grown a hundredth, the conforming the more; individuals keep
  their own bent, and nonconformists most of it.
- **Drift**: each year a culture's values wander a little (a hundredth, as a standard step, by
  data), the shares of its work too within what its people's way of life allows, and now and then
  a custom changes — where a pair lives, descent, how the dead are laid to rest, the greeting, a
  taboo taken up or let go — and its motif.
- **Contact**: each year a culture meets those of its people's other bands within twenty
  kilometres, the nearer the more: their values draw toward each other, and now and then one takes
  up a custom of the other's (a burial, a greeting, a motif).
- **Splits**: a daughter culture begins as its parent's and from then on drifts on its own, drawn
  back only by contact; two daughters that never meet drift apart, two that meet stay alike — the
  peoples part where their bands part.

## To come in H5

Languages and names; speech acts and gestures; subtitles translated as far as the player knows
the language; the acceptance.
