# The interface's look

*Status: implemented (Q1, Amendment Q §6); item icons from meshes wait for S6's item meshes.*

## Purpose
An interface that suits a photoreal world: real typefaces, crisp at any resolution and scale,
quiet panels, and a journal that reads as a field notebook, with the layout test and the
accessibility settings kept.

## Model
- **Typefaces** (`hearth_ui::font`): Source Sans 3 for the interface and Source Serif 4 for
  the journal, both SIL Open Font License 1.1 (`data/hearth/fonts/`, `ASSETS_LICENSES.md`),
  built into the binary. At start their glyphs (ASCII, Latin-1 and the punctuation, arrows and
  signs the words use) are drawn four texels per interface pixel, turned into signed distance
  fields (Felzenszwalb's transform, ±5 texels) and packed into one 1024² single-channel atlas
  with a solid block for rectangles. A character a face lacks is drawn as `?`; a test checks
  every character of the language files is in both faces.
- **Metrics**: text is laid out in interface pixels as before, capitals 6.5 tall on a baseline
  7 below the top of a 9-pixel line box, lines 11 apart; widths are the faces' advances,
  rounded up for layout.
- **Drawing** (`hearth_render::ui`, `ui.wgsl`): the atlas is sampled linearly and the field
  stepped at 0.5 over about a screen pixel (`fwidth`), so text is sharp at every interface
  scale; rectangles sample the solid block.
- **Colours** (`hearth_ui::widgets::theme`): umber panels, warm grey buttons, a warm off-white
  for words (about 15:1 on a panel, dim words about 7:1), ochre focus, moss progress, rust
  warnings.
- **The journal** (`journal_ui.rs`): ruled paper, the serif face, inks for what is known,
  history and legends, discoveries and hunches (each at least 4.5:1 on the paper).
- **Scrolled areas** keep room for their bar whether it shows or not, so text never rewraps
  when it appears.
- **A specimen** for screenshots: `hearth --screenshot …,ui=3` draws a panel of the widgets and
  a journal page over the shot at interface scale 3 (`interface::specimen`).

## Checks
`hearth_ui::font` tests (both faces hold the language's characters; capitals sit on the
baseline at their height; the field is half on the outline), `tests/layout.rs` (every screen at
every resolution and scale), the specimen shot (`docs/review/q1/`).

## Known
Item icons rendered from the items' meshes are built with S6, when items have meshes of their
own (today a thing is drawn as one coloured box, D236). Kerning is not applied (`ab_glyph` reads
no GPOS); the faces' own spacing is good at interface sizes.
