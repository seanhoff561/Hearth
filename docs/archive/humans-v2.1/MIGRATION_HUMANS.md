> **Superseded by Amendment E** (`docs/spec/amendments-e-q.md`, 2026-10-08): the simulated humans were removed in E0 and are planned anew as Phase F (`docs/design/future/humanity/`). Kept for reference; nothing here is built, tested or linted.

# Migration: the V2-11 agent framework → V2.1 people

V2.1 (`docs/spec/v2.1-realistic-humans.md`, with its Addendum A: the player is born) replaces
v2 §8.4 and §17 and milestone V2-11 with a simulation of people: a **Person** is a composition
of components (genome, phenotype, body, psyche, mind, knowledge, social, culture, life history,
possessions) under a **species profile**, and everything from *Australopithecus* to farmers runs
on it. V2-11 had built four of its five parts when V2.1 arrived (2026-10-03). This inventory
says, for each piece, what becomes of it, and in which milestone (`PLAN.md`, H0–H13).

**Keep** = moves onto the new framework as it is (renamed at most). **Refactor** = the purpose
and most of the code stay; the shape changes. **Replace** = the purpose stays; the
implementation is redone. Nothing V2-11 built is dropped.

## What V2-11 built

V2-11 (a)–(d), committed together with this file:
- (a) *Australopithecus* as a population of the ecological cells (`data/hearth/fauna/hominins.ron`,
  D161), its hunters, the single-cradle setting; fifty-year runs in three realms' savannas.
- (b) The `hearth_agent` crate: kinds of agent from the content's hominins, agents with the
  player's body (scaled to their size, in a coat of hair), knowledge, carrying and a utility
  mind; groups with their places, ties and techniques; work through the player's process
  engine.
- (c) Hominins in the world: groups drawn out as agents near the player and folded back,
  their days (feeding at fruit and nut trees and on the open ground, drinking, cracking marula
  stones on anvils, knapping), climbing to the crown at dusk and bending leaf nests, the alarm
  call, mobbing and flight to the trees, wariness of a person; drawn as small long-armed hairy
  bipeds; screenshots (`tools/shots/v211_hominins.shots`).
- (d) Traces, habituation and learning by watching: a group's site laid under a nut tree the
  first time it is drawn out (anvil, hammers, cobbles of the place's rock, the flakes and cores
  of earlier work); habituation to a calm person, unlearned by running at them; a crouched or
  crawling person noticed less far off; the watch triggers of the knowledge a process rests on
  (`watch:hominin_*`) heard by a player watching within 40 m, `study:tool_scatter` by one
  standing at a scatter; a scripted observer gains insight toward knapping (the V2-11
  acceptance test, `hearth_agent/tests/days.rs`).

## Inventory

### Content and data

| Piece | Where | Decision | Note |
|---|---|---|---|
| `Hominin` schema (name, height, mass, group size, habitat, knowledge, behaviours, population) | `hearth_content/src/schema/era.rs`, `data/hearth/hominins/` | **Refactor** (H0) | Becomes the **species profile** (`data/hearth/humans/species/`): body ranges, life history, cognition caps (language, planning depth, symbolic thought, teaching), social defaults, attainable eras, the knowledge it is born into, its population. The behaviours list becomes the profile's capabilities and, in H2, its routines. |
| `Behavior` enum (Forage, DigTubers, CrackNuts, FishTermites, Scavenge, KnapFlakes, TreeNest, AlarmCall, MobThreat, FleeToTrees, Habituate) | `era.rs` | **Refactor** (H0 → H2) | Kept as the profile's capability flags in H0; in H2 the routines and HTN methods (data) take over what each names, gated by the flags. |
| *Australopithecus* population (diet, life, density, range, realms) and the hunters' prey lists | `data/hearth/fauna/hominins.ron`, predator entries | **Keep** (until H7) | The ecology's group model stays the demographic tier of hominins until H7's household and demographic tiers take over people's numbers. |
| Hominin populations drawn as agents, not animals (`Species.hominin`, `drawn()`, `materialize` skipping them) | `hearth_fauna/src/{species,live}.rs` | **Keep** | — |
| Single cradle (`hominins_in_cradle`, `HomininRange::SingleCradleRegion`) | `species.rs`, `crates/hearth/src/fauna.rs` | **Keep** → extended (H8, done) | Deep time's *H. erectus* appears in the cradle's best country and spreads from it (V2.1 §15.1, D191). |
| A group's tolerance of people kept with its numbers (`Group.tolerance`) | `hearth_fauna/src/ecology.rs` | **Refactor** (H4) | Becomes the group's and each person's relationship to the player (familiarity, trust, fear; §8.2, §8.7). |
| Marula stones, `crack_marula_stones`, the nut-cracking node's processes | `data/hearth/{materials,flora,processes,knowledge}` | **Keep** | Agents keep using the player's processes. |
| `leaf_nest` block and texture | `data/hearth/blocks/craft.json`, `hearth_texgen/src/craft.rs` | **Keep** | — |
| `watch:hominin_*`, `study:tool_scatter` routes | `data/hearth/knowledge/paleolithic.ron` | **Keep** → generalized (H6) | H6 generalizes observation to watching any skilled person (`watch:<process>` from the same graph walk). |

### `hearth_agent` crate

| Piece | Decision | Note |
|---|---|---|
| `Kind`, `Kinds::from_content` (a kind from a hominin entry: size, knowledge, techniques, body, coat) | **Refactor** (H0) | Becomes `Species` from the species profile; its body and coat stay; in H1 the person's phenotype sets its stature and looks; its physiology by its own size (`body_of`: DuBois area, Kleiber BMR, gait by leg length) follows with the growing bodies of H3. |
| `Agent` (body, knowledge, carry, mind, position, sex, stage) | **Refactor** (H0) | Becomes **`Person`**, a composition: `Body` (Keep), `Knowledge` (Keep: the player's `KnowledgeState`), `Possessions` (Keep: the player's `Carry`), `Mind` (Refactor), `Social` (from the group's ties), `LifeHistory` (new: birth, parents, events), and the place in the world. `Genome`/`Phenotype` (H1) and `Psyche` (H2) join as their milestones land; saves carry a component version. |
| Ages as three stages (young, half-grown, grown) | **Replace** (H3) | Real ages on the calendar with V2.1's life stages, child bodies and growth. |
| `Mind`, `Doing`, `Intent`, `Needs`, `Situation`, `choose` (a utility choice from needs, fear and what is about) | **Refactor** (H2) | Becomes the third layer (utility selection) of V2.1 §6's architecture, under routines and over the HTN planner; reflexes (flight, alarm) become layer 1. |
| One random stream for all agents | **Replace** (H0) | One stream per person, from its id and the world's seed; decisions read the state of the step before, so the order (and the thread count) cannot change the outcome. |
| `SocialGroup` (members, home, range, ties, habituation), `Places` (water, sleeping trees, anvils, food) | **Refactor** (H0 → H4) | The group becomes a band in the persons' `Social` memberships (H0); places become each person's semantic memory, a mental map that can be wrong (H2); ties become relationships (H4). |
| `Culture` (techniques, traditions) | **Refactor** (H5) | A reference to a generated culture (§9). |
| `Hominins` (the agents near the player: draw out, fold back, step, act, work, move, climb) | **Refactor** (H0, H7) | Becomes the **full tier** of the persons registry. H0: persons persist in the registry and the world's save (folding keeps their records); H7: promotion and demotion between the tiers conserving state. |
| `AgentWorld` (ground, things, food, exposure, surroundings, hunters, calls, nests, sites) | **Keep** | The world as people meet it; grows with the systems (construction in H2/H12, speech in H5). |
| `Things`, `Pile`, `plan_work`, `finish_work` (work through the player's engine, knowledge first) | **Keep** | The action layer under the H2 planner; the anachronism guard (H6) checks it. |
| `watched`, `seen`, `scatter_near` (what watching teaches) | **Keep** → generalized (H6) | — |
| Habituation and stealth (`Person.plain`, tolerance, heed distance) | **Refactor** (H4) | The stranger handling of §8.7: familiarity and trust grow from calm contact, fear from threat. |
| Tests: `agents.rs` (kinds, knowledge, the engine), `days.rs` (days, nests, alarm, flight, watching, habituation, the observer) | **Keep** | Ported to the new API in H0; they are the measure of "*Australopithecus* behave at least as well as before". |

### The game

| Piece | Where | Decision | Note |
|---|---|---|---|
| The agents' world adapter (map ground, things lying, food trees, hunters, sites, nests, crown perches) | `crates/hearth/src/hominins.rs` | **Refactor** (H0) | Becomes `people.rs`, the game's side of the persons simulation. |
| Looks and animation of a hominin (`looks`, `drive`) | `hominins.rs` | **Replace** (H1, H2) | Looks from the phenotype (H1; families look related); animation from actions and emotions (H2). |
| `Proportions::hominin`, `Rig::hominin`, `Figure::hominin` | `hearth_character` | **Refactor** (H1) | The species' body plan in its profile, then the person's phenotype within it. |
| `ToClient::Hominins(Vec<AgentView>)`, the client's `ShownHominin` | protocol, `client.rs` | **Refactor** (H0) | `ToClient::People(Vec<PersonView>)`. |
| Server wiring (the tick, calls heard with the animals', what the player sees done heard an hour apart) | `server.rs` | **Keep** | Renamed with the module. |
| Screenshot keys `hominin=` and `seek=` for a hominin population | `screenshot.rs` | **Keep** | `family=` (H1) sets three generations of the human pool before the camera; `era=` an era's people about a birth among them, living and keeping their camps (H8). |
| The character creator's appearance editor | `crates/hearth/src/menus.rs`, `profiles.rs` (v2 §9.1) | **Replace** (H1, Addendum A) | The player is born: no appearance is chosen; the genome is a child's of two parents. |

### Docs

| Piece | Decision | Note |
|---|---|---|
| `docs/design/agents.md` | **Replace** (H0) | By `docs/design/humans/` (one document per V2.1 section, written with its milestone). |
| `docs/design/future-humanity.md` (v2 §17) | **Replace** (H8, done) | By `docs/design/humans/history.md` and `eras.md`; kept for what it says of the later eras until H11–H13. |
