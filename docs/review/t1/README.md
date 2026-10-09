# T1 review pictures (Amendment T §2)

Rendered on the cloud machine's software device (`hearth --software --screenshot-list
tools/shots/t1_birds.shots`); how they look on a real GPU is the owner's check.

## T1.1 Birds over remains
- `ravens_near.jpg` — five ravens circling 18–54 m over a dead hind 25 m off, from a rise
  (85° lens, looking up): dark crosses against the sky, wings spread, soaring.
- `ravens_risen.jpg` — the same with someone near: the rings wider by seven tenths and 30 m
  higher.
- `ravens_300m.jpg`, `ravens_300m_detail.jpg` (×4, nearest neighbour) — the flock from 300 m:
  five dark specks a few degrees over the horizon, one still drawn as a bird (3 px), the rest
  as specks.
- `ravens_1500m.jpg`, `ravens_1500m_detail.jpg` — from 1.5 km: one-pixel specks just above the
  hazy horizon, as hard to pick out as real ravens there. A raven's 1.2 m span is 3′ at 1.5 km,
  under a pixel's 6.7′ here (720 lines over 70°); drawn as its figure it fell between the pixels
  and vanished, so far birds are drawn as specks no smaller than a pixel and a half across
  (`fauna::speck`).

## T1.2 Skin
- `skin_lights_before.jpg`, `skin_lights_after.jpg` — the creator's preview: a face from the
  front and three-quarters, a darker face, a hand and a wet face (columns) in daylight,
  overcast, at dusk and by firelight (rows); `cargo test -p hearth_render --test body_preview
  skin_under_four_lights` makes it (`bench-out/skin_lights.png`). Before: a vinyl sheen over
  every face (plainest on the darker one), pale, greyish colours, wet skin a few hard glints.
  After: matte skin with soft highlights, the colours of measured skin (warm on the fair face,
  a deep warm brown on the darker), the hand's creases and knuckles, wet skin under a film that
  mirrors the light.
- `skin_world_sun.jpg`, `skin_world_evening.jpg`, `skin_world_wet.jpg` — people in the world
  (`tools/shots/e7_people.shots`' views).
- Not judged here: photographs of real skin under the same lights (they are the owner's to put
  side by side, and stay out of the repository); how a real GPU draws the pores up close.

## T1.3 The thing itself
`tools/shots/t1_aim.shots` (summer, temperate plains, 60° lens, eye height; `look=` turns the
camera onto the part named and highlights what the middle of the view meets, as in play):
- `aim_berries.jpg`, `aim_berries_detail.jpg` (×2, nearest neighbour) — a bilberry, the eyes on
  its berries: each berry outlined on its own, the leaves not.
- `aim_bush.jpg` — the same bilberry, the eyes on its leaves: the whole sprite outlined along
  its own pixels, gaps and berries included.
- `aim_raspberries.jpg` — a raspberry two blocks tall, the eyes on its fruit at chest height:
  the red fruit outlined, nothing else.
- `aim_branch.jpg` — a young hazel's branch (a limb block): its own crossed limbs outlined.
- `aim_stone.jpg`, `aim_stick.jpg` — a granite cobble and an oak stick lying in the grass, each
  outlined by the box it is drawn as (the other lying unlit beside it).
- `aim_dig.jpg` — the grass ground: the patch a dig there would take, a soft wash with a
  brighter rim lying on the ground (and on the plant standing in it).
- `aim_water.jpg` — the shallows: a ring on the water's surface about the point looked at.

## T1.4 The globe in relief
`tools/shots/t1_globe.shots` (seed 7, the Earth). Before: `docs/review/e4.1/globe.jpg` (flat biome
colours, a slope shade).
- `globe_biomes.jpg`, `globe_climate.jpg`, `globe_relief.jpg` — the whole Earth by biome, by
  climate and as plain relief: ranges standing as ridges, the glacier range white, lakes and
  rivers, the shelves pale along the coasts and the deep sea dark.
- `globe_range.jpg` (×6) — nearer over the highest range: its relief lit from the north-west.
- `globe_range_near.jpg` (×16, plain relief) — the 2.4 km level's relief over the view: ridges,
  valleys and cirques through the ice.
- `globe_coast_near.jpg` (×12) — a coast: the shelf, rivers to the sea, the hills behind it;
  the biomes still the grid's 20 km cells.
