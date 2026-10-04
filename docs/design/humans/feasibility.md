# What is simulated, and what stands in for it

*V2.1 §0.3. Where the ideal cannot be simulated literally, the best feasible model of it stands
in, said plainly here. Numbers are filled in as each milestone lands.*

| Ideal | Why not literally | What stands in | Where |
|---|---|---|---|
| A human genome | Memory, compute, no meaning in play | An abstract diploid genome: 23 chromosome pairs with real relative genetic lengths, a few hundred loci in functional groups, alleles as small integers; meiosis with crossovers, mutation, inbreeding; traits calibrated to realistic heritabilities | [genetics.md](genetics.md) (H1) |
| A human mind | Unsolved, and compute | Layers from cheap to dear: reflexes, routines, utility selection, hierarchical (HTN) planning, social reasoning; driven by personality, emotion, memory and belief | [mind.md](mind.md) (H2) |
| Everyone, individually, through deep time | Billions of person-years | Three tiers: deep-time populations (gene pools, cultures, languages, knowledge); households of individuals for the recent past and the wider region; full persons near the player | [tiers.md](tiers.md) (H7) |
| Natural conversation | No offline understanding at this quality; nondeterministic | Structured speech acts rendered through generated languages; an optional, off-by-default language-model backend that can only phrase and parse validated speech acts | [language.md](language.md) (H5, H10) |
| Real peoples and religions | Does not fit a random planet; invites stereotypes | Generated fictional cultures, languages and beliefs whose structures follow cross-cultural patterns | [culture.md](culture.md) (H5) |
| Endless variety of behaviour | Authoring cost | Behaviour generated from personality × culture × situation × memory, over routines and norms authored as data | [mind.md](mind.md) (H2) |
| A whole childhood lived through | Fifteen years of play | The player's childhood at a childhood pace: moments at normal speed, the years between at the household tier | [player-birth.md](player-birth.md) (H3) |
| Two million years of human history over Earth | Earth's size; billions of lives | Demes of each species on a history grid of the planet's own geography, a few thousand cells, stepped by centuries to generations; the planet stands for Earth (numbers for inventing and keeping knowledge reckoned as Earth's) and, smaller than a people's mating network's country, holds its peoples up to thirty times denser than real; a century of households lived about the place a life begins | [history.md](history.md) (H8) |
| An ice-age world | World generation is today's | The era's sea level and cold act in deep time (land bridges, glacial shelves, the cold's retreat); the land walked is today's | [history.md](history.md) (H8, D195) |

## Commitments carried by every substitute

- **Determinism.** Each person draws from its own random stream (seed and id); decisions read the
  state of the step before; parallel updates cannot change results (tested from H0).
- **Same systems as the player.** Agents make things through the process engine, build through
  the construction system, carry by the carrying rules, live in the player's body physiology.
  Only the reduced tiers approximate, and they conserve state on promotion and demotion.
- **Only what they know.** No agent performs a process outside its knowledge state (the
  anachronism guard, H6); no agent acts on what it has not perceived or remembered.
