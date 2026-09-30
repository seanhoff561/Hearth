# Content platform

*Status: implemented (V2-0). Code: `crates/hearth_content`, `crates/hearth/src/content_cli.rs`,
`crates/hearth/src/content_state.rs`. Data: `data/hearth/`.*

## Purpose
All game content is data so the design can keep changing without code changes (v2 §3.1).
Rust implements mechanisms; data supplies parameters and content.

## Model
- **Packs**: a pack is a directory of namespaces, `<pack>/<namespace>/<domain>/**/*.ron|json`
  (single-file domains may also be `<namespace>/<domain>.ron`). The base pack is `data/`; world
  and user packs load after it and override entries by id.
- **Domains** and their Rust schema types (`hearth_content::schema`): `materials`,
  `geology/{rocks,minerals,provinces,deposits,soils}`, `flora`, `fauna`, `ecosystems`,
  `hominins`, `items/{forms,items}`, `processes`, `knowledge`, `workstations`, `construction`,
  `clothing`, `body/{injuries,illnesses}`, `eras`, `balance/{keys,presets}`, plus singletons
  `units.ron`, `time.ron`, `body/human.ron`.
- **Files**: `(schema: N, entries: [ ... ])`. `schema` must match the domain's current version
  (`Entry::SCHEMA`). RON is preferred (implicit `Some`, unwrapped newtypes); JSON is accepted.
- **Entries** always have `id`, `status` (`Implemented` when a system uses it, `Planned` for data
  authored ahead of its system — hidden in game), `notes`, `realism_source`, `uncertain`.
- **Ids**: `namespace:path`; a bare path takes the namespace of the file's pack
  (`IdRef::qualify`, resolved while parsing).
- **Generate, don't enumerate**: every `items/forms` entry × every matching material is an
  item (`form/material`), mass = size × fill × density, properties read from material fields.
- **Unknown fields are errors** (via `serde_ignored`), with file and line.

## Lint (`hearth content lint`, part of `scripts/check.sh`)
1. Unit sanity: every numeric field against physical ranges for its unit (`validate.rs`).
2. Cross-references: every id resolves in the right table; biomes exist in the world
   generator; balance presets only set known keys within range.
3. Knowledge graph: no cycles; era order (warning); implemented nodes can't depend on planned
   ones; implemented nodes need a discovery route.
4. Reachability: a fixpoint from natural materials (rocks/minerals present in provinces or
   deposits, soils, plant parts, animal products, water) and gatherable items through
   workstations, knowledge and processes. Unreachable implemented content is an error,
   planned content a warning.
5. Habitats and food webs: animals have existing habitats; ecosystems have producers and
   every animal finds food where it lives.
6. Effort rollup (§12.5): distinct processes, real hours, play minutes and gathered mass needed
   to reach each node from nothing, averaged per era; a warning if an era is cheaper than
   the one before.

## Graphs (`hearth content graph`)
DOT and self-contained SVG (longest-path layering + barycentre ordering) for the knowledge
graph, the process graph (material flows) and one food web per ecosystem, with an HTML index
in `docs/generated/`.

## Hot reload
F3+T reloads every pack; a reload with errors keeps the previous content. Systems declare
whether they rebuild live (`content_state::RELOADABLE`); those that can't are logged.

## Known simplifications
- Blocks still come from the v1 block JSON (`data/hearth/blocks`); V2-2 generates them from
  materials × block forms.
- Composite items (a hafted spear of wood + flint + sinew) are not yet modelled; V2-5.
- Process outputs of a form without a material filter inherit the materials of the first
  input (a flake is struck from a core of the same stone).

## Future
Per-domain content migrations when a schema version changes; JSON-schema export for editors;
pack dependency declarations.
