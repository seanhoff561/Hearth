# Assets and their licences

Every third-party file the game ships, with where it came from (Amendment Q §4). `scripts/lean-check.sh`
fails a shipped asset with no entry here. Code dependencies are licensed through Cargo and are
not listed.

| File | Source | Author | Licence | Retrieved | SHA-256 | Used for | Processing |
|---|---|---|---|---|---|---|---|
| `data/hearth/fonts/SourceSans3-Regular.ttf` | https://raw.githubusercontent.com/adobe-fonts/source-sans/release/TTF/SourceSans3-Regular.ttf | Adobe (Paul D. Hunt et al.) | SIL Open Font License 1.1, Reserved Font Name "Source" (`data/hearth/fonts/OFL-SourceSans.md`) | 2026-10-08 | `4644c81b86ec9caaa76b634889968ed3c4f4f52f054855933acc7c2b21e53b0f` | the interface's typeface | none; drawn into a distance-field atlas when the game starts |
| `data/hearth/fonts/SourceSerif4-Regular.ttf` | https://raw.githubusercontent.com/adobe-fonts/source-serif/release/TTF/SourceSerif4-Regular.ttf | Adobe (Frank Grießhammer et al.) | SIL Open Font License 1.1, Reserved Font Name "Source" (`data/hearth/fonts/OFL-SourceSerif.md`) | 2026-10-08 | `e5a4ee6a3d87bb9024796be390c6771e2a0eb1883dae25effaf57ca01668e24b` | the journal's typeface | none; drawn into the same atlas |
