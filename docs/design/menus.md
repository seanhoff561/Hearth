# Menus and world management

*Status: implemented (Amendment P P1; the mode on Create World and the world's Edit with P2,
`modes-creative.md`); the Field Guide on the pause menu comes with P6.*

## Purpose
Screens that always fit the window, worlds that can be looked after, and a short way into a new
world that ends with choosing where to be born (Amendment P §4).

## Model
- **Layout that fits** (`hearth_ui::widgets`). A page is a title, its rows in a column that
  scrolls inside its area when the window is too short for them, and a footer row (Done, Back)
  held at the window's bottom (`menus::page`). The scrolled area cuts what passes its edges (the
  draw list clips), takes the pointer only within it, scrolls by the wheel, by dragging its bar,
  and by the keys and the controller moving the focus to a row out of sight. Long screens are
  split into tabs (Video: Display, Quality, Distance).
- **The layout test** (`tests/layout.rs`, part of `cargo test` and so of `scripts/check.sh`)
  lays every screen out on the CPU at 1280×720, 1600×900, 1920×1080, 2560×1440, 3840×2160,
  2560×1080 and 1024×768, at the interface scales Auto and 1–4, full screen and in a window (its
  frame and the task bar taken off), and fails if an interactive widget is out of reach (off the
  window and not in a scrolled area that is on it), overlaps another, or cuts its text. The
  widgets note where they were laid out when `UiState::layout` is on.
- **Worlds** (`worlds.rs`). Each world is told by its name, era, the player's name and age (from
  `player.json`: their age when their life began and the days lived since, by the world's
  calendar), play time and when last played. Play, New world, Rename, Duplicate (a copy beside
  it, named as one), Back up (a dated copy in `saves/backups/`), Open folder, Delete (asked first,
  naming the world) and Trash. A deleted world moves to `saves/trash/<folder>.<unix time>/`; the
  Trash screen restores it (beside a world that has taken its folder since, if one has) or
  empties the trash, as does Options; a world deleted more than thirty days ago may go.
- **Create World**: a name, a seed (empty for a random one), the player's name (or none), the era
  with how its people live, and More options for the world's shape: the planet's size
  (Standard recommended), the height of the land, the day's length, days in a season and the
  season it begins in (`server::WorldShape`, saved in the world's settings). The death and
  knowledge rules and the loincloth are no longer asked (P2's modes set them).
- **Making the planet**: the app builds it on its own thread (caching it where the world will
  find it), its stages shown with a bar ("Raising mountains…"); then its globe opens.
- **Where to be born**: the globe (`globe::GlobePicker`) behind the screen, turned by dragging
  and zoomed by the wheel; the place under the pointer described in plain words; a click
  chooses, Recommended takes the place the world finds best for a first life, Surprise me
  anywhere on land. The world is then opened with the birthplace (`WorldSpec::birthplace`, kept
  as `settings.birthplace`, save format 5): the first life is born on land near it, among the
  era's people there, and the world's calendar starts by it. The households to be born into
  follow as before (H8).
- **Opening a world**: the server tells how far it has come (`ToClient::Progress`: the planet,
  the deep past, the recent past) and the client shows it with a tip.
- **Pause**: the time of day and the year in words ("Late afternoon, the third day of autumn"),
  Back to the game, Options, Watch the world, Save, and Save and quit at the bottom.

## Parameters
`menus::ROW`, `PAGE_TOP`, `W`; `worlds::TRASH_KEPT_S` (thirty days); the sizes, heights, day
lengths and season lengths Create World offers (`menus::SIZES` and the lists in its screen).

## Interactions
Saves (`hearth_save`: `WorldSettings::birthplace`, migration 4→5); the server's world start
(`server::run`, `calendar_of`, `calendar_in`); the globe (`globe.rs`, `hearth_render::globe`).

## Known simplifications
- No globe thumbnail in the world list yet, nor the mode (P2).
- The hover description is the globe's own (climate, biome, height, temperature, rain); who
  lives there in the era and the dangers and resources come with P2/P6's plain-language pages.
- A world opened from the list skips the globe (it keeps its birthplace).
