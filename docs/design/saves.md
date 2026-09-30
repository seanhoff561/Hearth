# Saves, versioning and migrations

*Status: implemented (V2-0). Code: `crates/hearth_save`.*

## Purpose
Worlds must survive constant content and format changes (v2 §3.5).

## Model
```
saves/<world>/
  level.json      WorldMeta: format, name, versions, times, settings, clock,
                  block state palette, enabled data packs
  region/         r.<x>.<y>.<z>.hrg, 8×8×8 cubes per file
```
- **Format version** (`meta::FORMAT`, currently 3). Format 1 (v1) is refused with a clear
  message; newer formats are refused; older supported formats are migrated step by step on the
  raw JSON value (`migrate.rs`) and written in the current format on the next save.
- **Region files**: 8-byte header, 512 index entries (sector offset, byte length), 4 KiB
  sectors. Records are zstd-compressed `[cube format][Cube::write_bytes]`. Rewrites reuse
  sectors when they fit, else the first free run.
- **Registry mapping**: `level.json` stores the block-state names indexed by the ids used in the
  region files. On load every name is resolved against the running content.
- **Unknown content degrades gracefully**: saved states the content no longer defines become
  placeholder blocks with exactly the same name and properties — solid, drawn with the
  missing texture, and written back unchanged, so removing and restoring a mod loses nothing.
- **World settings** (`settings.rs`): planet (world-gen settings), life & time (day length,
  days per season, starting season, axial tilt, realism preset + overrides, predator behavior,
  knowledge mode, hominin range, death rules, full map knowledge) and the era.

## Tests
Fixture worlds in `crates/hearth_save/tests/fixtures`: a format-2 world (with `coal_ore`,
removed in V2-0) that migrates to format 3 and keeps the ore as a placeholder through a
save/reload cycle; a format-1 world that is refused.

## Future
Player, entity, ecology-cell and structure files as those systems arrive, each versioned;
region compaction; world backups before migrations.
