> **Superseded by Amendment E** (`docs/spec/amendments-e-q.md`, 2026-10-08): the simulated humans were removed in E0 and are planned anew as Phase F (`docs/design/future/humanity/`). Kept for reference; nothing here is built, tested or linted.

# Persons

*V2.1 §2–3, §17 (in part); milestone H0. Code: `crates/hearth_people`.*

Every hominin and human — and the player — is a **Person**: an id with components. A **species
profile** (data) sets the ranges each component may take. The people near the player are lived
in full by the same systems as the player; the rest wait in the **registry** as records and are
drawn out again as themselves.

## The Person

A person is a record of components, each its own type with its own fields defaulting on load, so
a component can grow without breaking saves:

| Component | Holds | From |
|---|---|---|
| `life` (LifeHistory) | sex, the day of birth (ages are reckoned from it), mother and father where known, birthplace, the events of its life (drawn out, met the player, died, …), death and its cause | H0; filled out in H3 |
| `body` | the player's physiology (`hearth_body::Body`) under its species' and its own size | H0 (v2 §9) |
| `mind` | what it is doing and how long it keeps at it, fear, its perch up a tree | H0; the layered mind in H2 |
| `knowledge` | the player's knowledge state (`KnowledgeState`): nodes, insight, skills | H0 (v2 §12) |
| `social` | its band, its ties (mother–child bonds now; relationships in H4) | H0; H4 |
| `possessions` | the player's carrying (`Carry`) | H0 (v2 §10) |
| `place` | where it is in the world: position, facing, speed, medium (ground, water, tree) | H0 |
| `rng` | its own random stream | H0 |
| `genome`, `phenotype` | inherited variation and what it unfolds into | H1 |
| `psyche` | personality, emotion, mood, values | H2 |
| `culture` | the culture it was raised in, its own conformity | H5 |

It is a composition rather than an archetype ECS: the people in full simulation are hundreds, the
household tier's records (H7) are compact records of the same components, and the player keeps
these component *types* in its own state (the same `Body`, `KnowledgeState` and `Carry`), so every
system written for a person's component works on the player's (D165).

Each **player** will have a person record too (its life history and genome from H1, its
relationships from H4); a player's body, knowledge and possessions are the server's player
state, of the same types. A player's mind is the person at the keyboard.

## Every player, never "the player"

The people never assume a single player (D166): they are lived in full about every player's
place, and they take the players about them as a list, each with its own identity. A band is
drawn out near any player and folded back only when far from all of them; a person weighs every
player it notices and fears most the one nearest its flight distance; a band's tolerance is kept
for each player by name, so coming to know one player teaches it nothing of another; a client is
sent the people about its own player. What the client is told travels as serializable messages
under the protocol's version (`hearth_protocol::PROTOCOL`).

## Species profiles

`data/hearth/humans/species/*.ron` (schema `Species`). A profile gives:

- **Body:** height and mass ranges by sex, the body plan (an australopith's long arms and short
  legs, a modern or a robust human's), the coat (hair, or bare skin), whether it climbs.
- **Cognition:** language (calls, proto-language, full), the planner's depth, symbolic thought,
  teaching (none, minimal, real), fire (none, keeping, making), the highest knowledge era it can
  reach.
- **Life history:** gestation, weaning, maturity (first birth) and adulthood, the span of adult
  life, the birth interval.
- **Social defaults:** group sizes, which sex leaves its birth group.
- What it knows and does, where it lives, its population in the ecological cells, when it lived.

*Australopithecus* is implemented; *Homo erectus*, *H. neanderthalensis* and *H. sapiens* are
data until their eras (H3's families and H8). The same code runs all four.

## Determinism

- Each person draws from its **own random stream**, seeded from the world's seed and its id, and
  saved with it; a band's own stream places those drawn out.
- A step has two phases. **Deciding** — each person senses what is about it and chooses — reads
  only the state as the step found it (a snapshot of everyone's place and doing) and writes only
  that person, so it runs in parallel and no person's choice depends on who was decided first.
  **Acting** — moving, taking things, working, calling, nesting — touches the shared world and
  runs in the order of the persons' ids. Same seed and inputs give the same people, on one thread
  or many (`tests/determinism.rs`).

## The registry, tiers and saves

- The **registry** holds every person ever drawn out, with an index by id, and the bands.
- **Full:** within reach of a player, lived every tick. **Dormant:** a record waiting; its
  band's numbers live on in the ecological cells (D161), which feed, breed and kill them as a
  population. When a player comes near again the band is drawn out *as the same persons*: the
  numbers the cells now hold are reconciled with the records — those the cells lost have died
  while no player was near (the oldest grown ones and a share of the young), those they gained
  are born to the band's mothers or join it — and everyone ages by the days between. H7 replaces
  this with the household tier and conserving promotion and demotion.
- Persons are saved with the world (`people.json.zst`): a format version and every record, with a
  migration step per version, as `level.json` has. Saving folds the bands' numbers into the cells
  first (in a copy), so a world reloaded draws them out again as themselves.

## The inspector

In the debug screen (F3), looking at a person within 40 m shows its record beside the debug
lines: species, sex, age and stage, tier; its life history; its body's state (energy, water,
core temperature, injuries); what it is doing and why (needs, fear), how long it keeps at it;
what it knows and its skills; its band and ties; what it carries. Each milestone adds its
component (the genome and phenotype chain in H1, the psyche in H2, …).
