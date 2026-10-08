# Art direction: stylized realism (Amendment S)

*The look of the smooth voxel world (`docs/spec/amendment-s-smooth-world.md` §1.1). Every texture,
mesh, shader and screenshot from S0 on is judged against this page.*

## The target

The world should read as **real places**: a valley someone could walk into, a beach that shelves
into the sea, a scree slope under a cliff, a riverbank cut by the water, a beech wood in October.
It is not a toy and not a photograph. Forms are clean and silhouettes readable; color is chosen,
not scanned; detail is where the eye goes (the ground at one's feet, a face across a fire, the
bark of the tree one is felling) and resolves into calm, legible masses with distance.

**Nothing looks like a cube unless it was built or cut.** Natural ground, rock, snow, water,
trees, plants, animals and people have no corners the land or life would not give them. Things
people make out of discrete pieces — logs laid in a wall, planks, posts and beams, bricks, dressed
stone, thatch bundles — keep the grid's crispness, because real buildings are made that way, and
show the material's own detail (bevels, grain, bark, mortar, tool marks).

## References, by description

No other game's assets or looks are used or imitated; the references are places and things.

- **Soft ground:** grassy downland after rain; the wind-rippled backs of dunes; snow drifted
  against a hedge; a ploughed field's crumbling furrows; a cattle-trodden riverbank of wet mud.
- **Crisp ground:** a limestone scar with its bedding planes; a basalt sea cliff in columns and
  blocks; granite tors with rounded corners and sharp joints; a slate quarry's cleaved faces.
- **Loose ground:** scree fans under cliffs, settled at their angle of rest; a gravel bar in a
  braided river; a spoil heap beside a dug pit, slumping where it is steepest.
- **Woods:** an oak's grey ridged bark and its crown of lobed leaves lit from behind at evening;
  a pine's straight red-brown bole; birch's white bark and fine twigs bare in winter; the dappled
  light on a woodland floor; a rainforest's buttresses and layered canopy.
- **Living things:** a red deer's mass carried on thin legs; a wolf's narrow chest; a person's
  shoulders, hands and face legible at conversation distance — body proportions as measured, not
  heroic.

## Palette principles

- Colors come from the materials' real appearance (`appearance` in `data/hearth/materials/`),
  then are held within a shared range: saturation never above what sunlit real surfaces reach,
  greens kept earthy and varied by species and season, no pure black or pure white in diffuse
  albedo (charcoal and fresh snow included).
- Value carries form: light and shadow read before hue. Each material family has a narrow value
  band so a scene groups into masses (dark woods, mid-value grass, light rock or sand).
- Weather and season shift the palette through the shader, not the textures: wet ground darker
  and glossier, dry grass toward straw, autumn crowns by species, snow and frost by exposure.
- Fire and lamps are the only strongly saturated light; the sky sets the mood of everything else.

## Material families

| Family | Members (examples) | Sharpness | Surface character |
|---|---|---|---|
| Soft sediments | sand, silt, loess, peat | 0.05–0.15 | Rounded, rippled, slumping; fine grain; wet darkening strong |
| Soils | loam, clay soils, podzol, chernozem | 0.15–0.3 | Soft, crumbly, rooty; grass and moss over them by moisture |
| Clays and muds | clay, mud | 0.2–0.35 | Smooth, slick when wet, cracked when dry |
| Gravels and scree | gravel, talus, till | 0.35–0.5 | Lumpy at the decimetre scale, settled at the angle of rest |
| Snow and firn | fresh snow, packed snow, firn | 0.05–0.2 | Smooth blankets, drifts and cornices; sparkle |
| Ice | lake ice, glacier ice | 0.5–0.7 | Smooth sheets; glaciers crevassed and banded |
| Layered rocks | sandstone, shale, limestone, chalk | 0.7–0.9 | Crisp ledges; strata banding finer than a metre from the geology |
| Massive rocks | granite, basalt, gneiss, quartzite | 0.8–1.0 | Jointed blocks, sharp-but-irregular edges, lichen on old faces |
| Ores and minerals in rock | copper, iron, tin minerals | as their host | Stains and veins blended into the host rock (prospecting cues) |
| Worked materials | logs, planks, bricks, dressed stone, thatch | crisp pieces | Bevels, grain, tool marks, mortar lines; weathering by age |

Sharpness is data (S1): 0 is perfectly soft (the surface follows the smoothed field), 1 keeps
creases where the field turns sharply (the mesher's feature solve).

## Scale rules

- 1 block = 1 m, always. A person is 1.5–1.9 m tall, a doorway about 1.9 m, a step 0.15–0.25 m,
  a sheep's shoulder 0.65–0.75 m.
- Texture detail is sized in world units: grains of sand and gravel at their true scale up close;
  strata bands of 5–50 cm; cracks and joints of decimetres to metres.
- Smoothing never moves the surface by more than a voxel from where the simulation has it:
  collision, editing and the eye agree (the developer-only blocky view exists to check this).

## Detail and distance

- Near (≲ 30 m): full material detail, height-blended transitions, parallax on rock and gravel
  where the preset allows, individual leaves and grass blades.
- Mid (30–200 m): simplified meshes, merged leaf clusters, macro color variation doing the work
  of the detail.
- Far: material averages (the same values the near field fades to), canopy shapes per species,
  impostors; no color or brightness jump at any transition.

## Checks

A screenshot passes when: no cube shows in natural things; soft materials look soft and rock
crisp; nothing is blobby (no melted cliffs, no pillowed edges); no tiling pattern repeats at a
glance; cube seams and LOD transitions are invisible; people and animals sit in the same world
as the ground they stand on.
