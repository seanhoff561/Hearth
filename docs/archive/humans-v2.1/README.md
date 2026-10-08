# The simulated humans of V2.1 (archived)

> **Superseded by Amendment E** (`docs/spec/amendments-e-q.md`, 2026-10-08). Nothing here is
> compiled, tested or linted. Phase F (`docs/design/future/humanity/`, after Phase R) plans
> simulated humanity anew and may restore parts of this work.

## What existed

Built in milestones H0–H10 (2026-10-03 to 2026-10-07), removed in E0:

- **Crates:** `hearth_people` (about 25,000 lines: genetics, psyche and minds, life course and
  demography, social systems, culture and language generators, social learning, simulation
  tiers, deep-time history and the Paleolithic eras, the Observer's chronicle) and `hearth_ai`
  (about 2,300 lines: the optional conversation backend, its closed-vocabulary filter,
  providers and prompts).
- **Game modules** (`crates/hearth/src/`): `people.rs` (persons near the player, their tick and
  views), `born.rs` (births and households), `childhood.rs` (childhood moments),
  `conversation.rs` (the backend's queue), `history_cli.rs` (`hearth history`), `preview.rs`
  (the birth screen's figures), and the human parts of `eras.rs`, `observer.rs`,
  `observer_ui.rs`, `server.rs`, `client.rs`, `menus.rs` and `screenshot.rs`.
- **Protocol:** persons, births, childhood, speech acts, typed words, the conversation
  backend's messages, the Observer's chronicle, overlays and followed lives.
- **Data:** `data/hearth/humans/` (species, genetics, psyche, mind, life tables, childhood
  moments, social norms, culture and language generators, learning, history), `data/hearth/ai/`
  (cues, prompts, word lists), `data/hearth/fauna/hominins.ron`, the peoples of
  `data/hearth/eras/eras.ron`, and the hominin observation triggers of the knowledge graph.
- **Content schemas:** `ai`, `culture`, `history`, `humans`, `language`, `learning`, `life`,
  `mind`, `psyche`, `social`, and the era's peoples.
- **Tests:** `acceptance_h9`, `afterlife`, `childhood`, `conversation`, `era_week`, `eras`,
  `family`, `observer`, `hominins`, `genetics_lint`, and the crates' own suites.
- **Docs:** `design/` (this folder's copy of `docs/design/humans/`), the spec
  `v2.1-realistic-humans.md` with Addenda A and B, and `MIGRATION_HUMANS.md`; the screenshot
  lists of the people and eras (`shots/`, no longer runnable) and the era reviews (`review/`).
- **Smaller pieces:** the australopith figure (`Figure::hominin`), predators' taste for
  australopiths, the `watch:hominin_*` discovery triggers, the people's options and keys.

## How to look at it

The last commit with all of it is kept as the branch and tag `archive/humans-v2.1` (commit
`f244710`): `git checkout archive/humans-v2.1`.

## What Phase F may restore and adapt

- **The genetics engine** (`hearth_people::genetics`): diploid genomes, meiosis, polygenic
  traits, pigmentation by latitude with no behavioural trait varying by population (ground
  rule 1, enforced by lint).
- **Life tables and demography** (`hearth_people::life`): mortality and fertility by way of
  life, households, birth intervals; to be moved to real time and childhood-as-the-past.
- **The language and culture generators** (`hearth_people::language`, `culture`): phonologies,
  sound change and word replacement; trait-based culture with drift and diffusion.
- **Kinship** (`hearth_people::kin`): relatedness, households, inheritance.
- **The deep-history simulation** (`hearth_people::history`): bands spreading through the
  planet's realms over deep time, knowledge diffusing on the graph, a chronicle of events.
