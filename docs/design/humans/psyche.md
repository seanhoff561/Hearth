# The psyche

*V2.1 §5; milestone H2; D174. Data: `data/hearth/humans/psyche/` (tendencies, feelings, values).
Code: `crates/hearth_people/src/psyche.rs`; its use in `mind.rs` and `sim.rs`.*

**In short:** every person has a psyche: **tendencies** (personality as it shows in what they
choose), **feelings** that rise as events are appraised, fade each at its own pace, spread to
those who see them and show on the body, a **mood** that follows the feelings and the body
through the day, **stress** that builds from what threatens or wears on them for long, and
**values** that weigh what they care about. Temperament comes from the genes (H1); development
(H3) and culture (H5) shift how it is expressed.

## Tendencies (§5.1)

Thirteen behaviour parameters: risk tolerance, curiosity, patience, cooperativeness, trust in
strangers, conformity, copying the successful (prestige bias), emotional reactivity, the threshold
for aggression, forgiveness, diligence at work, sociability and dominance seeking. Each is a
**curve in the data**: a middle for one of middling temperament, a range, and how far it moves
per standard deviation of the heritable traits it leans on (the HEXACO factors and the narrower
dimensions of `genetics.md`). They are reckoned when a person's psyche is formed — from their
phenotype, when their band is first drawn out — and kept.

**Effect sizes are modest.** A tendency tilts a choice, never makes it: the bold let a threat come
a little nearer before running (flight distance × 1.3 − 0.6 × risk tolerance), the patient keep at
a thing a little longer, the sociable groom a little more, the diligent take up work a little more
often, the conforming stray a little less far from the others. The situation, the body's needs
and (from H4) relationships and norms usually decide. The tests find trait–behaviour correlations
of about 0.1–0.4, as studies of personality and everyday behaviour do.

## Feelings (§5.2)

Ten feelings: fear, anger, grief, shame, guilt, indignation, joy, pride, disgust, affection. Each
has in the data its valence (its weight in the mood), how fast it fades — a moment's in seconds
of play (fear's half-life 40 s), a lasting one's in days of the world (grief's 20) — how much of
another's is caught, how it shows on the body, and the traits that make it felt more strongly
(a person's **gain** for it, 0.4–1.8).

- **Appraisal.** Events stir feelings by what they mean to the person: a threat near, fear (the
  nearer the more); an alarm heard, fear; a new hurt, fear and anger; a kin's death, grief by how
  close the kin was (a mother, a child, a brother or sister most; a grandparent or half-sibling
  half as much, by the coefficient of kinship); food when hungry, joy; work done, joy and pride;
  company and grooming, affection. One's own and others' wrongs (shame, guilt, indignation) come
  with the norms of H4; disgust with spoiled food and the dead.
- **Contagion.** A feeling shown (above a fifth of its full strength) is caught by those of the
  band who see it within 25 m, the nearer the more, at the feeling's own rate — but **at most half
  as much as is seen**, so a fright passed on weakens with each passing and dies away without its
  cause (an early version without the halving let a band frighten itself for good, D174).
- **Showing.** The strongest feeling shown is the one the body shows (`Display`: cowering in
  fear, bristling in anger or indignation, slumped in grief, hanging the head in shame or guilt,
  bright in joy or pride, recoiling in disgust, warm in affection); the people's views carry it to
  the figures.

## Mood and stress

The **mood** (−1…1) follows the feelings (weighted by valence) and the body's strain (hunger,
thirst, weariness, pain) with a time constant of about a third of a day; a low mood weighs toward
rest. **Stress** (0–1) builds toward the load of fear, grief and strain over about two days and
eases over about five; it will weigh on health (H3) and on choices.

## Values (§5.3)

Kin loyalty, generosity, honour, autonomy, deference to elders, piety, fairness and courage: a
default weight where no culture speaks (H5 gives cultures theirs), tilted by personality. In H2
courage sets how afraid one may be and still face a hunter with the others; the social systems
(H4) weigh the rest.

## Saved and shown

The psyche is a component of the person record (people format 3); kinds are saved by name, so
kinds added later read as nothing from older saves. The inspector shows the mood, stress and the
feelings now, the tendencies and the values.
