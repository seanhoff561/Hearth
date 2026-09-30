# Future humanity: eras and simulated humans

*Design only (v2 §17). Hooks are built as their milestones arrive; nothing here may be blocked
by current work.*

## Eras as data
`eras/` already defines Wild Earth (playable) and seven later eras shown as "coming later".
Every species has `first_appearance_ya` / `extinction_ya`, so an era picks its flora and fauna
by date; eras also carry sea-level and temperature offsets (a glacial maximum lowers sea level
~120 m, scaled), hominin species and knowledge baselines.

## Hooks to build (and where)
- **Agent framework** (V2-11): `Body` (the player's physiology parameterised per species),
  `Mind` (needs, perception, memory, goals, utility/GOAP planning), `KnowledgeState` (known nodes
  and skill levels), `Inventory` (the player's carrying model), `SocialGroup`, `Culture`.
  Agents run the same `processes` data as the player.
- **Knowledge transfer** (V2-11): observation now (hominin → player), later teaching and records
  between any agents.
- **`HistorySimulator`** (world-creation stage, a no-op for Wild Earth): will simulate
  thousands of years of spread and settlement over the planet's rivers, soils, climate and
  resources before play.
- **Settlements** (V2-8 onward): blueprints made of the same construction pieces and obeying the
  same structural rules as the player's buildings.
- **Ownership, territory, claims**: fields on world data, unused until societies exist.
- **Simulation layers**: human populations live in the abstract layer like animal populations
  and materialise near the player (v2 §3.7).
