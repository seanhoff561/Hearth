# Phase F roadmap (after R10)

*Amendment E §10.13. Each milestone's acceptance tests are detailed when Phase F begins; an
audit (Amendment Q §8.2) follows every three. The same list is at the end of `PLAN.md`.*

| | Milestone | Builds | Depends on | Acceptance outline |
|---|---|---|---|---|
| **F0** | Research and prototypes | model latency, throughput and quality on the reference hardware for System 1 and System 2; a five-person believability prototype in an isolated scene; a voice-latency prototype | R10 | budgets for `08-compute.md` measured; the prototype judged believable in blind comparison; first audio under 0.8 s |
| **F1** | People foundation | the Person record; levels C0–C2 on the Actor framework; genetics, life course and demography restored from the archive, in real time with childhood as the past | F0 | a region of C1 households lives a decade at real rates within budget; genetics lint (rule 1) |
| **F2** | The History Engine | region graph, settlements, polities, economy, conflict, culture, snapshots, event store | F1 | demography, economy and conflict within the sanity ranges of `11-evaluation.md`; centuries without unexplained collapse; deterministic |
| **F3** | The World Bible and the Historian | the knowledge graph, the chronicle with fact validation, lazy zoom | F2 | every chronicle claim cites events; contradiction rate under the set bound; zoom consistent with the level above |
| **F4** | Procedural minds (C2) | needs, emotions, the HTN planner, routines, social rules, pathfinding at every scale | F1, S3 | a village's day at C2 plays without a model, every plan physically valid |
| **F5** | The System 1 action model | training pipeline, batched runtime, fallback rules | F4 | 300 people at 5–20 decisions a second within the GPU budget; fallback on every failure |
| **F6** | Reflective minds (C3) | memory streams, reflection, structured outputs, validators, budgets; the AI Bridge providers (R4) | F4 | reflections change later behaviour sensibly; no invalid output reaches the world |
| **F7** | Conversation and voice (C4) | turn-taking, streaming speech in and out, voices, languages and translation, multiplayer (R6) | F6 | conversations with first audio under 0.8 s; Authentic and Understood both playable |
| **F8** | Societies | households, cooperation, institutions as collective agents, deliberation, markets, conflict and war | F6, F2 | institutions decide by their rules; markets clear; wars follow the route graph |
| **F9** | Settlements | people building with the real construction system; growth and roads | F8, S6 | a settlement grows over years with structurally sound buildings and worn roads |
| **F10** | Births and childhood | births in any household; the vignette engine with the real family's minds; coming of age | F7, F8 | every safety rule's red-team suite passes; a childhood plays through to adulthood |
| **F11** | Eras | the Upper Paleolithic vertical slice first, then the others, each with an authenticity review | F1–F10 | each era passes its review before it is playable |
| **F12** | Scale, cost and safety hardening | million-person regions; cost and latency budgets; red-team suites; multiplayer determinism through the decision journal | all | budgets met on the reference machine; replays and multiplayer agree by state hash |
