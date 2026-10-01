# Flora

*Status: in progress (V2-6). Code: `crates/hearth_flora` (the growth model), the placement in
`hearth_worldgen`, the server's vegetation state and fire in `crates/hearth`. Data:
`data/hearth/flora`, `data/hearth/materials/wood.ron`, `data/hearth/blocks/trees.json`.*

## Purpose
Plants are the world's living surface: food, fibre, medicine, poison, fuel, timber and
shelter, the cover animals hide in and graze, and what fire burns (v2 §6). Trees are real
species with real heights and girths and recognisable silhouettes; vegetation grows back
after it is cleared or burned, slowly, as it does on Earth.

## Model

### Species (`flora/`)
Each species has its growth form, climate envelope, soil and light preferences, lifecycle and
phenology, dispersal, growth rate, maximum height and trunk diameter, lifespan, wood, usable
parts by season with edibility, flammability and appearance (v2 §6.1). Trees and woody shrubs
add a **form** for the growth model:
- crown: spreading, ovoid, conical, columnar, weeping, umbrella, multi-stemmed;
- live crown ratio (crown length ÷ height) and crown width ratio (crown width ÷ height);
- apical dominance (0 a broad decurrent crown of forking limbs, 1 one straight leader);
- branch angle from the stem, droop, whorled branching (conifers), trunk taper, root flare,
  number of stems;
- leaf type (broad, needle, scale) and foliage density;
- blocks for the trunk (`log`), limbs (`branch`) and foliage (`leaves`).

Herbs, shrubs, ferns and fungi add how they are drawn and found: their block, the light they
grow in (under canopy or in the open), the ground (wet, rich, poor), how thickly, and a
look-alike group when they can be mistaken for another (below).

### The growth model (`hearth_flora`)
A tree is a pure function of its species, its age and a variant number:
1. **Allometry.** Height follows a Chapman–Richards curve, H(a) = Hmax · (1 − e^(−k·a))^1.4,
   with k set by the species' early growth rate; trunk diameter grows on after height
   levels off, D(a) = Dmax · (1 − e^(−a/τ)), τ = lifespan ÷ 4. A 60-year oak is about 20 m
   tall and 0.5 m through; a 300-year oak 30 m and 1.6 m.
2. **Stages.** Seedling (under 0.6 m), sapling (to 3 m), pole, young, mature, old, ancient,
   and dead standing (snag). A tree is grown at a representative age for its stage, so its
   blocks change only when it passes from one stage to the next.
3. **Skeleton** (after Weber and Penn's parametric trees): a stem (several for multi-stemmed
   species) that leans and tapers; decurrent species fork into a few limbs at the crown base;
   first-order branches leave the stem at the species' angle (in whorls for conifers), as long
   as the crown's shape allows at that height (conical: longest at the bottom; ovoid: at the
   middle; umbrella: at the top); second- and third-order branches fork from them, shorter and
   thinner, drooping as the species droops; foliage clusters at the twig ends.
4. **Voxels.** Wood thicker than about 0.9 m fills whole log blocks across its section (logs
   lie along the stem's axis); thinner wood becomes branch blocks, round bars 12, 8, 4 or 2
   pixels thick that join their neighbours along the branch. The flare at the foot spreads
   into the ground as roots. Foliage fills its clusters at the species' density.
5. **Templates.** Each (species, stage, variant) is grown once into a template (its blocks
   relative to the foot, and a per-column summary of the crown for the distant terrain) and
   cached; a tree in the world is a template turned by a quarter turn and mirrored as its
   position's hash says. Eight variants per stage make each forest of one species look varied.

### Blocks
Per wood: a log (axis), a branch (thickness 2, 4, 8 or 12 px; joined north, south, east,
west, up, down). Per species: leaves. Thick branches (8 and 12 px) and logs are solid and can
be climbed; thin branches and foliage are not solid. **Foliage is passable**: moving through
it is slow (a dense crown slows to half pace), it rustles, and it shades the ground below a
little per block (sky light falls by one per leaf block). Deciduous foliage colours with the
season (green, its autumn colour, bare), as the shaders already do by phenology.

### Placement (`hearth_worldgen`)
Trees stand on the generator's grid cells where the tree density passes. A column's
candidate species are those whose climate envelope fits its mean temperature, coldest and
warmest months, precipitation and Köppen class, weighted by how centrally they fit and by
the biome's forest type; one is drawn by hash. Stand age varies smoothly across the land
(young stands to old growth); a tree's age is the stand's ± its own, capped by its species'
lifespan. The understory is placed per column from the species that fit the climate and the
light under the canopy (shade-tolerant under trees, pioneers in clearings and on edges) and the
ground. The distant terrain grows the same trees from their templates' crown summaries.

### Useful, edible, medicinal and poisonous plants
Plants yield their parts by season through gathering processes. Some foods need processing
(acorns are leached, nettles cooked). Medicinal plants have modest effects (willow bark eases
pain, yarrow stems bleeding). Poisonous plants have look-alikes (hemlock among the edible
umbellifers, death cap among white mushrooms, deadly nightshade among dark berries): until the
player has learned to tell a group apart, its plants are seen by their group's name, and
eating one may poison. Identification is knowledge, learned by its routes.

### Felling, limbing and bucking
Cutting through a trunk with a chopping tool takes time by its diameter, the wood's hardness
and the tool. The tree then **falls** away from the cut: the server takes the tree's blocks
(its template, as changed), removes them, and lays the trunk with its crown along the ground,
turned about its foot, resting where it first meets the ground; what stood in its way is
broken, and anyone under it is hurt by its weight and speed. The client shows the fall as the
whole tree turning about its foot under gravity over a few seconds. Felled trees stay felled.
Limbing takes the branches off a fallen trunk (poles, sticks, twigs; the foliage becomes
litter); bucking cuts the trunk into log sections (a metre of 40 cm oak is over 100 kg).

### Vegetation state and succession
The land is divided into ecological cells (256 × 256 m on the wrapped grid). Undisturbed
cells are what the generator makes; a cell that is cleared, burned or otherwise disturbed
keeps when, how, and how much, and the generator grows it back from that date: grasses and
herbs at once, then brambles and shrubs, then pioneer trees (birch, aspen, pine) a few years
on, then the climax species as the pioneers age. The player's felled trees are kept by place,
and new trees take their spots after a few years. Trees age with the calendar: when a tree
passes into its next stage, the cubes holding it are rebuilt. The state is saved
(`vegetation.json`), sent to the client and drawn in the distant terrain.

### Wildfire
Lightning in the dry season sets the vegetation alight, and so can the player. Near the player
fire spreads block by block through litter, grass, foliage and wood, faster downwind and
upslope and in dry weather, slower in damp; burned ground is left bare, trunks stand charred,
and the area enters succession. Far away a fire spreads over ecological cells by their fuel,
dryness, wind and slope (a surface fire runs metres a minute), and its smoke rises high enough
to be seen across the land. Fires burn out against water, rock, bare ground and rain.

### Distant terrain (engine)
LOD tiles are cached on disk by seed, content and the vegetation and edits that touch them.
The edits and vegetation state reach the distant terrain (felled stands, burned land, cleared
fields). Tiles stay within a VRAM budget, far tiles coarsening first. TAA is an option.

## Parameters
- Stages by height: seedling < 0.6 m, sapling < 3 m, pole < 0.4 Hmax, young < 0.75 Hmax,
  mature, old > ½ lifespan, ancient > ⅘ lifespan.
- Logs from 0.9 m of wood; branch thickness 12/8/4/2 px from 0.5/0.3/0.15/0 m.
- Templates: eight variants per stage; four turns and a mirror per tree.
- Ecological cells: 256 m; succession: herbs at once, shrubs from 2 years, pioneer trees
  from 3–8, climax species from ⅓ of the pioneers' lifespan.

## Interactions
Seasons (phenology tints and leaf cover, snow on crowns), light (shade under foliage), body
(movement through foliage, falls from trees, poisoning, medicine), items (wood, bark, parts),
processes (gathering, felling, limbing, bucking), knowledge (identification), weather (fire
danger, rain), fauna (cover and forage, V2-7), the distant terrain.

## Known simplifications
- Templates are grown per stage, not continuously; a tree's shape does not respond to its
  neighbours' shade.
- A felled tree falls as one rigid body; it does not break up or roll.
- Wildfire near the player is a block automaton; far away a cell model.

## Future extensions
Tier 2 species and biomes (V2-10); crops and domestication (V2-12); coppicing and pollarding;
fungi on dead wood; seeds as items and planting.
