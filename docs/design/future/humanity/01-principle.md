# 1. Simulate everything, think where it matters, narrate what is seen

*Amendment E §10.1.*

## The three loops
| Loop | Runs | Who | Rate | Models |
|---|---|---|---|---|
| **Society** | always, the whole planet | populations, polities, routes, markets | event-driven; coarse steps of days to years | numeric and agent-based (no language model) |
| **People** | near players; notable people anywhere | instantiated persons (C1–C3) | needs and routines each tick; plans hourly; reflection nightly | procedural minds; small language model for reflection |
| **Presence** | about each player | the people face to face (C4) and their bodies | 5–20 decisions a second; speech in real time | System 1 action model; conversation model; voice |

The loops nest: the society loop gives the people loop its households, prices, laws and news;
the people loop gives the presence loop its people's goals and moods; what happens in presence
and people flows back as events the society loop folds into its totals.

## Ground truth
- **The simulation is the record.** The world store (cubes, things, bodies, the ecology) and
  the event store of the History Engine hold what is true.
- **Models propose; deterministic systems dispose.** A mind's output is a structured
  proposal (an intent, a goal, a line of speech, a belief update) checked against what the
  person can do and know, executed by the same rules that bind players (the process engine,
  physics, construction), and recorded only when it happens.
- **Words never change world facts.** A person may say a false thing, believe it, or be lied
  to; the World Bible records who said what, not that it became true.

## Determinism
Every decision a model makes that affects the world goes into a **decision journal** (who,
when, the inputs' hashes, the output). Saves and replays read it instead of asking again;
multiplayer clients sync from it with state hashes (`08-compute.md`). Without models (the
*Procedural* Minds setting) the game is deterministic from its seed alone.

## What this rules out
- No per-person language model in the society loop: billions of past lives are statistics.
- No narration that invents facts: the Historian writes about events the engine produced.
- No model touching physics, inventories or the calendar directly: it asks, the systems act.
