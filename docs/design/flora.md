# Flora

*Status: in progress (V2-6). Code: `crates/hearth_flora` (the growth model), the placement,
succession and vegetation state in `hearth_worldgen` (`trees`, `cubegen::features`,
`cubegen::succession`, `vegetation`), felling and the regrowing of loaded terrain in
`crates/hearth` (`workshop`, `server`). Data: `data/hearth/flora`,
`data/hearth/materials/wood.ron` and `organic.ron`, `data/hearth/blocks/trees.json` and
`understory.json`. Decisions D75, D76.*

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
- crown: spreading, ovoid, conical, columnar, weeping, umbrella, multi-stemmed, and palm (one
  unbranched stem, leaning and curving up, crowned by a rosette of great arching fronds, V2-10);
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
   levels off, D(a) = Dmax · (1 − e^(−a/τ))^1.3, τ = lifespan ÷ 3. A 60-year oak is about
   20–26 m tall and under 0.6 m through; a 250-year oak over 30 m and over a metre.
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
the biome's forest type; one is drawn by hash. A realm's own species take its land; the
stand-in realm's grow where none of them suits it (D115). The tree line's and the open's
light-demanding small trees (a krummholz pine, a giant groundsel) are the alpine's and the
krummholz's own and keep to the edges of the forests (D137). Stand age varies smoothly across
the land (young stands to old growth); a tree's age is the stand's ± its own, capped by its
species' lifespan. The understory is placed per column from the species that fit the climate
(the summer's warmth bounding the lowland's, D135) and the light under the canopy
(shade-tolerant under trees, pioneers in clearings and on edges) and the ground; the stand-ins
come in full only where the realm's own would not cover the ground, judged over the place
rather than by the patches on one spot (D136). The distant terrain grows the same trees from
their templates' crown summaries.

The mountains (V2-10 (d)) have a montane forest below the tree line (larch, fir, spruce and
stone pine in the Alps and Siberia; spruce, subalpine fir, lodgepole and whitebark pine and
mountain hemlock in the Rockies; birch and deodar in the Himalaya; lenga, coihue and monkey
puzzle in the southern Andes; pencil cedar, yellowwood and kosso in East Africa), a krummholz
along it and the alpine above it, of every mountain whose tree line stands above the sea
(D134): alpenrose, dwarf juniper and green alder, fescues, cushions and flowers (moss campion,
glacier buttercup, edelweiss, gentians, asters, blue poppies); the Afroalpine's giant groundsels
and lobelias, tree heath, everlastings and lady's mantle; the páramo's and the puna's frailejón
and polylepis, ichu and páramo grass, lupines, chuquiragua, werneria and yareta.

### Useful, edible, medicinal and poisonous plants
The temperate understory has 26 species: berries (raspberry, bilberry, lingonberry, wild
strawberry, bramble, dog rose), greens and herbs (wild garlic, wood sorrel, dandelion, ribwort
plantain, yarrow, nettle, fireweed, burdock, wild carrot), poisonous plants (hemlock,
foxglove, deadly nightshade), juniper, heather, bracken and fungi (chanterelle, porcini, field
mushroom, death cap, fly agaric). Each is placed per column by climate, the light under the
canopy (from the tree density and the stand's age), the ground it needs (wet, rich, acid or
broken) and its abundance, in patches of its own size. Plants yield their parts by season
through gathering processes (picking berries, leaves and herbs, digging roots, gathering
mushrooms, cutting bracken). Medicinal plants have modest effects: willow bark eases pain by
30 % for four hours; a yarrow poultice slows a wound's bleeding to 0.4 of its rate; plantain
cleans a wound. Poisonous plants have look-alikes (hemlock among the edible umbellifers, death
cap among the pale mushrooms, deadly nightshade among the dark berries): until the player has
learned to tell a group apart, its plants and their parts are seen by the group's name, and
eating one poisons (plant poisoning, or deadly poisoning for the death cap and nightshade).
Telling a group apart (`telling_<group>`) is knowledge, learned by its routes, among them
eating the wrong one.

### Felling, limbing and bucking
Cutting through a trunk with a chopping tool takes time by its diameter, the wood's hardness
and the tool. The tree then **falls** away from the cut: the server takes the tree's blocks
(its template, as changed), removes them, and lays the trunk with its crown along the ground,
turned about its foot, resting where it first meets the ground; what stood in its way is
broken, and anyone under it is hurt by its weight and speed. The client shows the fall as the
whole tree turning about its foot under gravity over a few seconds. Felled trees stay felled:
the felling is kept in the vegetation state (below), not as the player's change to the blocks,
so the stump stands until another tree takes the place a few years on. The fallen trunk is the
player's change and lies where it came to rest. Limbing takes the branches off a fallen trunk
(poles, sticks, twigs; the foliage becomes litter); bucking cuts the trunk into log sections (a
metre of 40 cm oak is over 100 kg). Snags and charred trunks are felled the same way.

### Vegetation state and succession
The generator grows the land from a **vegetation state**: the year the trees have grown to
(calendar years since the world began) and every disturbance so far: a tree felled at its
foot, ground cleared, land burned, each with its year, centre, radius (its edge ragged) and
severity (a light fire leaves unburned patches). Disturbances are found by place; the
ecological cells (256 m) are the grid fire spreads over far from the player.

**Tree sites.** Each tree site holds a sequence of generations drawn from its hash. The first
is the tree the world began with, at the stand's age. A tree lives its species' lifespan
(± its own share), then stands as a snag for a few years and falls; a tree of the gap takes
its place, the shade-tolerant most often and the quicker-growing of them more often still. A
disturbance ends a generation early: a felled tree leaves its stump until the next takes root;
fire leaves the trunks of trees from poles up standing charred for 3–15 years; clearing takes
all. Opened ground grows back with the light-demanding first, by how their seed travels and
how fast they grow (wind-sown pioneers within 1–4 years: willow, aspen, birch, pine; others in
3–11), the shade-tolerant coming up under them after a third of the pioneers' lives and taking
the site when the pioneers die. A young stand thins as the stand grows, by the crowns of the
place's usual species at the stand's age, so the pioneers are not thinned before the others.
One tree felled in a closed forest is a gap (the shade-tolerant take it); three felled in each
other's gaps make an opening (the pioneers do).

**Ground.** Burned ground lies bare and black (`burnt_ground`) for 0.4 years; then herbs come
(fireweed first on burns); on cleared ground the herbs of open, broken ground come at once
and shrubs (brambles, raspberries, roses) from the second year; the ground counts as broken
for six years, and the young trees close the canopy over it, dimming the light, over the
forty years after the third. A felled tree's gap lets more light onto the ground from its
second year to its twelfth.

**Change in the loaded world.** Trees age with the calendar: generating a cube gives the year
it next changes (a tree's next stage, a generation's birth or death, a snag or stump falling,
a ground phase). The server keeps each loaded cube's state and that year; when it comes, or a
new disturbance reaches the cube, the cube is generated with the old state and the new, and
what changed is laid in where the player has not changed the block and no water stands (the
seasonal cover taken off and laid again), then relit and remeshed. The state is saved
(`vegetation.json`), sent to the client and drawn in the distant terrain.

### Wildfire
Lightning sets the vegetation alight (the struck tree's crown, and far off, cells of dry
land), and so can the player, by putting a burning brand to dry growth (`set_alight`).

**Danger.** How readily a place burns is the dryness of its fine dead fuel times how much of
its growth has cured. The fuel's moisture is its equilibrium moisture in the air (Simard's
formula from humidity and temperature: about 5 % in hot dry air, 12 % at 60 % humidity, 25 %
in damp air), raised by the rain of the last two days and soaked while rain falls; it carries
no fire above 30 %. The growth has cured in a dry month (by Gaussen's rule, the month's rain
under twice its temperature, as in a Mediterranean summer) and only a fifth otherwise.

**Near the player** fire spreads block by block through turf and litter (grass, podzol and
moss blocks), plants, foliage, twigs, limbs and trunks. Each burning block sets its neighbours
alight with a chance by their fuel, its own heat, the danger to the power 1.5, the wind
(downwind up to five times more readily, upwind a tenth or less, so a fire runs before the
wind and hardly backs) and the slope (flames climb). Turf burns to bare black ground
(`burnt_ground`); plants, foliage and twigs to nothing; limbs and trunks char (`charred_branch`,
`charred_log`). Flames show where the fire is. Rain puts burning blocks out. Fires stop at
what does not burn: water, rock, sand, burned ground. A body in the flames is burned and the
fire's heat reaches it as radiant heat. The fire's smoke rises over it, a plume for each
32 m square burning. Each 4 m square of ground the fire has burned and left is kept, a few at a
time, as a Burned disturbance with that exact shape, so it enters succession and stays burned
when the terrain is generated again.

**Far away** (beyond the loaded terrain, as far as 12 km from the player) a fire spreads over
the ecological cells by their fuel (grass and scrub most, forest less, nothing on water, rock,
ice and desert), the danger there, the wind and diagonals, burns each cell for 6–18 hours and
keeps it as a Burned disturbance of the cell (its edge ragged, unburned patches by how little
fuel it had). Its smoke rises a kilometre and more and is seen across the land. A near fire
running out of the loaded terrain is taken up by the far fire, and a far fire coming within
about 200 m of the player lights the near fire at the cell's edge.

### Distant terrain (engine)
The server sends the client the vegetation state (when a disturbance is added, and as the
years turn) and the player's changes as the distant terrain needs them (per changed column,
its highest solid block). Tiles are built from the generator with both: burned ground lies
black, cleared and burned land grows back, felled and dead trees are gone, trees grow by
whole years, and what the player has built stands above the ground. A tile a new disturbance
or change reaches is built again while the old one is drawn. Tiles are cached on disk beside
the planet's cache (`lod/<seed>_<circumference>/<level>/<x>_<z>.lod`, zstd), stamped with a
fingerprint of the build (two probe tiles near the spawn, so a change to generation, meshing
or colours makes every tile stale) and the vegetation and changes that reach them; a tile
whose stamp holds is read back instead of built. The tiles stay within the video-memory
budget (`lod_vram_budget_mb`): over it, the selection's distance rule is scaled down (the far
tiles coarsen first) until they fit, and scaled back up when they take less than 60 %. The
season shows in the tiles as near (deciduous crowns by phenology, bare in winter, snow, sea
ice). Temporal anti-aliasing is an option (Anti-aliasing: Temporal; on in the Fabulous
preset): the camera is jittered by a Halton (2, 3) sequence of eight, and each frame is
blended into the reprojected, neighbourhood-clamped history.

## Parameters
- Stages by height: seedling < 0.6 m, sapling < 3 m, pole < 0.4 Hmax, young < 0.75 Hmax,
  mature, old > ½ lifespan, ancient > ⅘ lifespan.
- Logs from 0.9 m of wood; branch thickness 12/8/4/2 px from 0.5/0.3/0.15/0 m.
- Templates: eight variants per stage; four turns and a mirror per tree.
- Ecological cells: 256 m; disturbances found in 16 m buckets; edges ragged by ±15 %.
- Succession: burned ground bare 0.4 years; herbs at once; shrubs from 2 years; broken ground
  6 years; canopy closing over 40 years from the third; a felled tree's gap lighter from 1 to
  12 years; pioneers (shade tolerance under 0.3) from 1–4 years, others 3–11, the
  shade-tolerant under them from ⅓ of the pioneers' lifespan; gap trees 1–5 years after a
  death or a felling.
- Lifespans: the first trees die at 0.97–1.12 of their species' lifespan, later ones at
  0.75–1.10; snags stand 4–14 years, charred trunks 3–15, stumps until the next tree takes root.
- Taking opened ground: (1 − shade tolerance)³ × seed travel (wind 1.5, water 1, animals 0.6,
  gravity 0.3) × pace (growth ÷ 0.5 m a year, 0.5–2). Taking a gap: (0.2 + 1.6 × shade
  tolerance) × pace.
- Fire: the near fire steps every 15 game seconds; catching in fully dry, cured fuel and still
  air per step: turf 0.20, herbs 0.25, foliage 0.09, twigs 0.07, limbs 0.025, trunks 0.012;
  burning: turf 40–90 s, herbs 30–60 s, foliage 40–100 s, twigs 2–5 min, limbs 15–30 min,
  trunks 40–80 min; wind factor e^(0.22·u·cos θ) downwind, e^(0.5·u·cos θ) upwind; climbing
  ×2.5, going down ×0.35; at most 6,000 blocks burning. Far: 0.15 × fuel × danger² per
  neighbour cell and game hour, at most 200 cells burning.

## Acceptance (V2-6)
- Species silhouettes at three ages: every species as a sapling, young and old over open
  grass, e.g. `hearth --screenshot "seed=7,planet=tiny,x=7640,z=2010,above=9,pitch=-9,
  yaw=90,fov=80,hour=9.5,yf=0.5,dry=true,clear=70@0:50,tree=english_oak:sapling:0@28:-9,
  tree=english_oak:young:0@45:3,tree=english_oak:old:0@60:28"` (a contact sheet of all 25
  was made from these).
- A cleared area goes through succession over simulated years:
  `crates/hearth/tests/succession.rs` (herbs at once, no shrubs in the first year, shrubs by
  the third, pioneers by the tenth, a young wood at forty, the shade-tolerant at two hundred)
  and `crates/hearth_worldgen/tests/succession.rs`; screenshots with `clear=R@YEARS:AHEAD`.
- Wildfire spreads and burns out plausibly in a dry-season test:
  `crates/hearth/tests/wildfire.rs` (in the driest month in hot dry air and wind it runs
  downwind and hardly backs, burns out within hours and stays burned when its terrain is
  generated again; in damp air it goes nowhere); screenshots with `fire=MINUTES@AHEAD` and
  `farsmoke=METRES`.
- The horizon from a peak: `hearth --screenshot "seed=7,planet=standard,x=10506,z=8200,
  y=422,pitch=3,yaw=150,lod=1024,hour=10,yf=0.45,dry=true,clouds=0.15,taa=true"`.
- The fly-through benchmark: `hearth bench --scenes flythrough` (1.2 km at 30 m/s, 70 m over
  the land, near cubes and distant tiles streaming as in the game; its report counts the
  frames over twice the median).

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
- A site holds one tree at a time: the tree that takes a gap waits unseen until the snag or
  stump before it falls, and grows from when it took root as if in the open.
- Trees regrow only on tree sites; the understory's plants are drawn afresh for a disturbed
  place, not grown individually.
- A fallen trunk does not rot away, and a felled tree's foliage does not wither.
- Fire burning when the world is saved is out when it is opened again (what it burned is
  kept). Fire does not yet spread from a campfire to the grass about it, and the player
  cannot beat it out.
- Smoke is drawn but does not yet blind, choke or smell; it does not darken the sky's light.

## Future extensions
Tier 2 species and biomes (V2-10); crops and domestication (V2-12); coppicing and pollarding;
fungi on dead wood; seeds as items and planting.
