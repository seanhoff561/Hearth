# Saves, versioning and migrations

*Status: implemented (V2-0). Code: `crates/hearth_save`.*

## Purpose
Worlds must survive constant content and format changes (v2 §3.5).

## Model
```
saves/<world>/
  level.json      WorldMeta: format, name, versions, times, settings, clock,
                  block state palette, enabled data packs
  player.json     the player: format (1), body (hearth_body) and mover (hearth_physics),
                  carried things and knowledge
  items.json      the things lying in the world (V2-4)
  crafts.json     stations and their fires, wildfires, harvest counts (V2-5)
  blocks.json     the blocks the player has changed, by position and state name (V2-5, D73)
  region/         r.<x>.<y>.<z>.hrg, 8×8×8 cubes per file
```
- **Format version** (`meta::FORMAT`, currently 7). Every world before format 6 had people in
  it (V2.1) and is refused with a clear message (Amendment E §2.4: "This world was made with an
  earlier version that had people in it; start a new world"); newer formats are refused. Later
  formats migrate step by step on the raw JSON value (`migrate.rs`): 6 → 7 (E3) takes out the
  day's and the season's lengths, the starting season and the tilt and sets the clock to start
  on a spring morning (`time.md`).
- **Region files**: 8-byte header, 512 index entries (sector offset, byte length), 4 KiB
  sectors. Records are zstd-compressed `[cube format][Cube::write_bytes]`. Rewrites reuse
  sectors when they fit, else the first free run.
- **Registry mapping**: `level.json` stores the block-state names indexed by the ids used in the
  region files. On load every name is resolved against the running content.
- **Unknown content degrades gracefully**: saved states the content no longer defines become
  placeholder blocks with exactly the same name and properties — solid, drawn with the
  missing texture, and written back unchanged, so removing and restoring a mod loses nothing.
- **World settings** (`settings.rs`): planet (world-gen settings), life & time (when the clock
  starts — a spring morning or the moment the world was made —, realism preset + overrides,
  predator behavior, knowledge mode, what death means) and the era.

## Tests
Fixture worlds in `crates/hearth_save/tests/fixtures`: a format-6 world (with `coal_ore`, no
longer defined) walked forward to format 7, that keeps the ore as a placeholder through a
save/reload cycle; older worlds that are refused.

- **JSON files of a world** (`WorldDir::write_json`, `read_json`): written atomically with a
  backup of the previous one; the server saves `level.json`, `player.json` and the rest every
  five minutes and when it stops (V2-3, `world-loop.md`).
- **The player's changes to the terrain** (`hearth::edits`, D73): terrain is generated afresh
  whenever a cube loads, and the blocks the player changed are laid back over it (before the
  finite water, the season's cover and the light). They are saved as positions and block
  states by name; states the content no longer has are kept as saved and not shown.

## Future
Entity, ecology-cell and structure files as those systems arrive, each versioned; the
player's changes into the region files as a per-cube overlay once building makes them many
(V2-8); region compaction; world backups before migrations.
