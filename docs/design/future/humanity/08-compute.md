# 8. Compute, cost and hardware

*Amendment E §10.8.*

## The Minds setting
| Setting | Levels | Needs |
|---|---|---|
| *Procedural* | C0–C2 only | nothing more; always available; deterministic from the seed |
| *Local small* | C3 on a small local model | a few GB of VRAM or a fast CPU |
| *Local full* | C3 and C4 locally | a strong GPU |
| *Hosted* | C3 and C4 through a provider the player configures | an account; costs shown as they accrue |

## Efficiency
- **Batching** System 1 for hundreds of people per GPU step; System 2 requests queued and
  batched by model.
- **Prefix caching** of each person's and culture's standing context (who they are, the style
  guide), so a request pays only for what changed.
- **Small models fine-tuned for the game's schemas**; **distillation** of large-model behaviour
  into small models and into the System 1 action model.
- **Quantisation** and **speculative decoding** where the runtime offers them.
- **Background queues** that never take frame time: inference has its own threads and budget.
- **Graceful degradation:** when compute is short the allocator assigns more C2 and fewer C3/C4,
  and deliberations fall back to the numeric models.

## Budgets (to be set in F0 from measurements on the reference machine)
System 1 under 1 ms of GPU per tick for 300 people; a C3 reflection within its queue's minutes;
C4 first audio 0.5–0.8 s; the society loop within the tick budget of `docs/design/budgets.md`;
cost per hour of hosted play shown and bounded by the player's cap.

## Multiplayer and determinism
- The server host provides the minds' compute; clients render and synthesise voice locally.
- The **decision journal** records every model decision that affects the world (who, when,
  input hashes, output). Saves and replays read it; multiplayer syncs from it with state
  hashes, so clients never need a model to agree.

## World creation
The Historian runs in batch with a progress bar and a time estimate; the *Procedural* setting
skips it.
