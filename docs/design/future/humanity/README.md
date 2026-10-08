# Simulated humanity (Phase F): the design

*Design only (Amendment E §10, written in E1). Nothing here is built before Phase F, which
begins after R10; no code exists for it now. The simulated humans of V2.1 are archived in
`docs/archive/humans-v2.1/` (and on the branch `archive/humans-v2.1`); Phase F restores and
adapts parts of them (`12-foundations.md`).*

## The goal
People who behave like real people of their era, alone and as societies, across millions of
lives and thousands of years of history, in a world that runs at Earth's real time and size.
No person has a human-level mind; the design gives **human behaviour wherever anyone is
looking**, thought scaled to attention, every part replaceable as models improve.

## Three loops, one ground truth
1. **Society** (always on, the whole planet, coarse): demography, economy, polities, culture,
   technology and conflict as numeric and agent-based models. No language model per person.
2. **People** (individuals near players and notable people): needs, plans, relationships and
   memories, procedural minds with occasional language-model reflection.
3. **Presence** (moment to moment about players): a fast action model, the body's motor
   control and real-time voice for the people face to face with players.

**The simulation is the ground truth.** Models propose what someone wants, says, believes and
decides; deterministic systems decide what can physically happen, what is true and what is
recorded. Words never change world facts.

## The documents
| § | Document | What it settles |
|---|---|---|
| 10.1 | [01-principle.md](01-principle.md) | the three loops, who decides what, determinism |
| 10.2 | [02-worldbuilding.md](02-worldbuilding.md) | the History Engine, the Historian, the World Bible, lazy zoom |
| 10.3 | [03-people.md](03-people.md) | one Person record; cognitive levels C0–C4; the attention allocator |
| 10.4 | [04-mind.md](04-mind.md) | the body as Actor, System 1, System 2, the planner, memory, emotion |
| 10.5 | [05-conversation.md](05-conversation.md) | turn-taking, what to say, voice, languages |
| 10.6 | [06-societies.md](06-societies.md) | households, cooperation, institutions, economy, conflict, pathfinding |
| 10.7 | [07-lives.md](07-lives.md) | births in any household, childhood as the past, living among people |
| 10.8 | [08-compute.md](08-compute.md) | the Minds setting, efficiency, multiplayer, the decision journal |
| 10.9 | [09-eras.md](09-eras.md) | eras as starting conditions; their order |
| 10.10 | [10-safety.md](10-safety.md) | the hard content rules and how each is enforced |
| 10.11 | [11-evaluation.md](11-evaluation.md) | believability, plausibility, consistency, stability, cost, safety |
| 10.12 | [12-foundations.md](12-foundations.md) | what is built now that this uses; what returns from the archive |
| 10.13 | [roadmap.md](roadmap.md) | F0–F12, their order and acceptance outlines |
| — | [research.md](research.md) | the prior work, verified, and what each lends the design |

## Rules for the work now
- No code for Phase F before R10 (Amendment E §0.3). What is built meanwhile is built general:
  the Actor rule (E §2.3), the multiplayer-ready rule (R §0.3), the technology graph and the
  process engine, the ecology's lazy tiers.
- These documents are refined as research and the game change; each stays concise and current
  (Q §5.4).
