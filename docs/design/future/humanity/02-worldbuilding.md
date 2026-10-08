# 2. Worldbuilding at the scale of a long saga

*Amendment E §10.2.*

## The History Engine
A numeric, event-driven model run from the first people (or an era's starting conditions) to
the chosen date, on a **hierarchical region graph** built from the planet: the Earth-sized
planet's grid (E4) aggregated into regions by watershed and biome; settlements as nodes, routes
as edges whose costs come from slope, rivers, coasts and ice (least-cost paths on the planet
grid). Every model reads the real world: soils, climate by year, the ecology's carrying
capacity, deposits, the knowledge graph.

| Model | State | Step | Grounded in |
|---|---|---|---|
| **Demography** | age-structured population per settlement and way of life | yearly | life tables by subsistence (the archive's foragers', erectus's, Neanderthals'; farmers' from the literature), nutrition and disease |
| **Subsistence** | food production per region | seasonal | the ecology's animals and plants (`hearth_fauna`, `hearth_flora`), fields and herds (V2-12) |
| **Technology** | known nodes per community | yearly | the knowledge graph; innovation and diffusion faster in large connected populations, losses in isolation (Henrich 2004) |
| **Settlements** | place, size, buildings, central places | yearly | water, soil, defence, resources; roads along least-cost paths |
| **Economy** | production, stores, routes' flows, prices | seasonal | gravity-model flows along routes (size × size / distance^k), specialisation, surplus, inequality |
| **Polities** | members, capital, rules, legitimacy | yearly | agent-based formation and collapse on the region graph (Turchin et al. 2013), alliances, succession, structural-demographic pressure (Turchin and Nefedov 2009) |
| **Conflict** | raids, wars, fronts | event-driven | land, water, routes and succession as causes; terrain, technology, numbers and logistics as outcomes |
| **Culture and religion** | trait vectors per community; language trees | yearly | drift, selection, diffusion and conversion; the archive's language generators for sound change and word replacement |
| **Shocks** | epidemics, droughts, volcanic winters | event-driven | density thresholds; the climate's own variability |

**Output:** an append-only **event store** (each notable birth, founding, war, migration,
invention: id, time span, place, participants, cause ids), **snapshots** every so many years
for lazy zoom, and **notable individuals** wherever the models produce leaders, founders,
prophets, generals or inventors (each a C0 Person record with a role, `03-people.md`).

**Budgets:** history from the era's start to its date in the world-creation budget (E4: about
45 s total at Earth size on the reference machine; a quick option for shorter histories);
deterministic from the seed.

## The Historian
Language models in batch at world creation, then lazily as players go places:
- **The World Chronicle:** names from the generated languages; annals of each polity; myths
  and origin stories; epics of the great wars; short laws and sacred texts; genealogies; songs
  and proverbs; customs, dress and architecture; labels on maps.
- **Perspectives:** each people's own account of a war beside the facts. Invention lives in
  perspective; facts are fixed.
- **Consistency:** every claim cites event ids; facts extracted from what was written (dates,
  places, participants, who was alive) are checked against the event store; a contradiction is
  rejected and the passage rewritten, or repaired.
- **Voice:** each culture has a style guide (register, imagery, what it values) so its texts
  stay distinct.

## The World Bible
Per world, versioned: a knowledge graph of entities (people, places, polities, things, ideas),
relations and events with time spans; the chronicle's documents; an embedding index over both.
Game systems and minds query it **through what their person could know**: a mind's queries are
filtered by the person's region, era, social reach and what they have been told.

## Lazy zoom
Detail on demand, coarse to fine, always consistent with the level above: world (at creation)
→ realm → polity → region → settlement → household → person. Approaching a region, or choosing
a birth there, synthesises its households, buildings, families and recent history, first as
numbers (from the snapshot and the events since), then as narrative (life stories, local
legends, gossip). Generation is deterministic from the seed and the snapshot, and cached; in
multiplayer the server generates and shares it.

## Scale
A macro chronicle of a few hundred thousand words at creation (configurable, budgeted, a quick
option), growing lazily to millions of words as the world is lived in. The numbers come first:
a world without the Historian (the *Procedural* setting) is complete, only less told.
