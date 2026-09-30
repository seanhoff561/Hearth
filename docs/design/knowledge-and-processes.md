# Processes and the knowledge graph

*Status: partial. Data and lint in V2-0 (`processes/`, `knowledge/`, `workstations/`); the
process engine, journal and discovery in V2-5.*

## Model
- **Processes** declare inputs (items by id, form + material filter, bulk material by mass, or
  tag), tool properties (`hard_hammer ≥ 0.5`, `sharp_edge ≥ 0.3`), workstation, conditions
  (heat, water, dry, shelter, near a feature), duration on a time scale, knowledge and skill,
  outputs with quality rules, by-products and failure modes. The same data drives the player,
  hominins and future simulated humans.
- **Knowledge nodes** declare era, prerequisites, needs (materials, items, stations,
  environments), enabled processes, discovery routes (experiment / observation / inference /
  evidence events with insight amounts and journal hints), skill track and real history
  (summary, approximate date). Planned nodes are data only, hidden in game.
- **Graph**: 176 nodes across Eras 0–8 (V2-0); Eras 0–2 have discovery routes and 21 seed
  processes; Eras 3–5 are filled in by V2-12…V2-14; Eras 6–8 stay planned.
- **Effort parity**: the lint's rollup reports steps, real hours, play minutes and gathered
  mass from scratch per era; it must rise steeply by era once processes exist for later eras.

## Known simplifications
Tool properties come from the item form × material (a flint flake's `sharp_edge` is its
knapping quality); composite tools and wear are V2-5.
