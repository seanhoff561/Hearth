# Building and structure

*Status: V2-8 in progress — (a) pieces in the world, (b) stability and (c) excavation done. Pieces' data since V2-0
(`construction/`).*

## Pieces in the world (V2-8 (a))

**Data.** A construction piece (`construction/*.ron`, `ConstructionPiece`) is a member or a
covering: a post, a beam, bark laid on a slope, brush piled against the wind, a course of dry
stone. It names the materials it may be made of (a filter: any wood, any bark, any hide, any
rock), its size *in its one block* (`size_m`: a post's thickness and height, a roof's width,
thickness and slope length; its mass and later its strength come from it and the material), how
full that is (`fill`: brush is mostly air, dry stone three parts stone to one of gaps), its
`shape`, what putting one up uses (`inputs`, `tools`), the time a practised builder takes
(`build`), the knowledge it needs, and for coverings the pitch, the least pitch at which it
sheds rain and its insulation.

**Generated, not enumerated** (`hearth_content::building`):

- *Blocks.* Every piece in every material it may be made of is a block, `ns:<piece>/<material>`
  (`hearth:post/hazel_wood`, `hearth:bark_roof/birch_bark`), from the `hearth:piece` template,
  textured from its material (`block/material/<material>`), sounding as its material does (wood,
  stone, earth, hide), burning if it is wood or bark. Its shape comes from the piece's shape and
  thickness (sixteenths, at least one; two for posts and beams, which a sapling pole would
  otherwise be under): a **post** upright in the middle; a **beam** across the top of its block
  along the way it faces; a **panel** at the side it faces; a **layer** on its floor; a **roof**
  in eight steps rising toward the way it faces; a **wall** the half of its block at that side;
  a **block** all of it. All but posts and whole blocks turn with a `facing` property. Roofs,
  layers, walls and whole blocks shade the sky; posts, beams and panels do not.
- *Putting up* (`place_<piece>`): the piece's inputs, tools, time, knowledge and work (METs),
  done to the open place where it goes (`Target::Ground`, `Effect::Place`), with the building
  skill; the knowledge a piece needs lists it as what it enables. Its material is its first
  input's (the bark at hand: birch bark makes a birch-bark roof).
- *Taking down* (`take_down_<piece>`): done to any block of the piece (`BlockMatch::Prefix`),
  in two fifths of the time, giving back four fifths to all of each input in the block's
  material; a little is broken or lost.

**Quantities are real** (`shelter.ron`): a bark roof is 1.4 m² of slope laid twice over, 56
strips of 0.05 m²; a flat bark covering 40; hides overlap (7 over a slope, 4 on a wall); a
metre of brush windbreak 24 sticks; a dry-stone wall half a metre thick 36 fieldstones a block
(0.375 m³ of stone); a lintel one slab. One simplification: a pole (2.2 m) makes one post or beam
a block long, its offcut not kept.

**Where a piece goes.** Beside the face looked at — the client sends `AimAt::Beside { pos,
face }` for a process that puts up a piece, the face found from the side of the hit box the ray
came in by — or in the place of what is looked at when that gives way (grass, lying snow).
There must be room: nothing there but what gives way, and no one standing in it. It faces the
way the builder faces, to the nearest quarter: a roof rises away from its builder, a panel
stands at the far side of its block.

**Stages** (`building::rests`): a piece must rest on what is about it, so a frame goes up before
what hangs on it. A post stands on the ground or on what stands (a post, a wall); a wall or a
whole block on the ground, a wall or a beam (a lintel over a door); a panel on anything below,
or lashed to a post or a panel beside it; a beam — lying across the top of its block — from a
post, a wall, the ground or another beam beside it, never balanced on a post's top; a roof or a
layer on anything below, or against the frame, the bank or the slope beside it. Whether it is
*strong* enough there is the frame's reckoning (b). The server checks it when the work starts
and again before anything is used up; the refusal says "It would have nothing to rest on
there."

**The ghost.** While a piece is the chosen offer, the client draws the edges of its boxes where
it will go (`building::ghost`, thin bars in the figure pass), pale where it would rest and red
where it would not; once chosen it stays there while the work goes on.

**A lean-to in blocks.** Posts two high at the ends of the ridge; a beam between them, flush
with their tops; roof rows stepping down from the ridge toward the open side, each against the
one above, the lowest on the ground; brush or hides at the ends.

## Stability (V2-8 (b))

**Members** (`building::member`). Every piece block is a member with a weight and what it can
bear, from its size, fill and material (`strengths`: measured bending, crushing and stiffness
where the material has them, else what its kind usually has — a rock's bending strength a
twelfth of its crushing, green wood's 60 MPa — all halved for a safety margin of two):

- the **moment** it breaks at bent (section modulus × bending strength): a pole as a circle
  (`round`), a slab as a rectangle, a panel or wall on edge with its height for depth, a layer or
  roof flat — a covering on a **frame** (`frame: (form, count, material)`, a bark roof's two
  rafters) bends as its frame does. A heap of many things merely laid together (dry stone,
  brush: `Stacked` joints only, more than one input) bears no moment at all;
- the **continuity** of a joint between two pieces of a run, as a share of that moment, from the
  best joint the piece is made with: stacked 0, mortared 0.1, woven 0.2, lashed 0.4, pegged or
  nailed 0.6, mortise and tenon 0.8 — so stones laid end to end carry nothing across;
- the load it is **crushed** by as a column (area × crushing strength), and its **stiffness**
  (E·I) for buckling;
- its **reach**: fifty times its depth from what holds it (a span twice that), past which it
  sags beyond use even if it does not break.

**The reckoning** (`hearth_world::structure`). A change anywhere marks the structure about it;
its connected pieces (faces, and a roof's next up and down its slope; at most 8192) are
reckoned whole:

- *Columns* — posts, and walls, blocks and panels with something under them that bears them —
  carry their weight and what rests on them down to it; crushed past their crushing load, and a
  post buckles past π²EI/h² over the height of the posts it stands in. A column with nothing
  under it falls.
- *Runs* — beams along the way they face, roofs along their slope, layers along the way they
  are held at both ends (else one), and walls, blocks or panels with nothing under them along
  their length — are held by what is under any of their pieces and by what their ends rest
  against (two roofs leaning together at a ridge hold each other's tops, the weight going down
  each slope). Each span between supports is simply held, beyond the outermost a cantilever;
  the moment is found at every piece's middle (against its moment) and every joint (against the
  joint's), linear in the run's length; and each piece's reach from its supports.
- Loads go from the top down: at each level coverings, then the frame's runs (each before the
  runs it rests on), then the columns; each passes its supports their shares.
- A piece fails where what it bears is more than it can (its **stress**, the largest share, is
  over one). What is outside the reckoning, and land not loaded, holds as it did.

It is deterministic (everything in order of place) and the same whatever the order of the
changes. The server reckons up to 8192 pieces a tick (a hut is a hundred or two; 4300 pieces
take a few milliseconds); what is left waits.

**Collapse.** What fails is gone at once: half of what taking it down would give lies where it
fell (a broken beam's pole, half a wall's stones), and what it held is reckoned on the next
tick, so a collapse spreads a step a tick as it would. The client sees each piece's boxes drop
and turn until they strike the ground, dust thrown up and settling, and hears it give way —
wood cracking and splintering, stone grinding and knocking, brush and earth slumping, by its
weight — and land.

What follows from it, e.g. with the first pieces: a lean-to of hazel poles stands with a bark
roof on two rafters; its ridge spans up to four metres between posts (stiffness, not strength,
limits a pole); a pole reaches two metres from the post it is lashed to; a granite lintel spans
a one-block doorway under a dry-stone wall, but two slabs end to end over two blocks fall at the
joint; dry stone over nothing falls.

## Excavation (V2-8 (c))

Natural ground holds as long as nothing is dug under it. Over an opening, a block of ground
roofs only so wide (`building::self_span`): what falls (sand, gravel, ash) not at all, loose
soil (loam, alluvium, mud, humus) half a metre, firmer soil 1.2 m, clay 1.5, rock one metre and
a fifteenth of its crushing strength in MPa (chalk two or three metres, sandstone five, granite
fifteen and more), ice four, packed snow one and a half; a material may say otherwise
(`span_m`: loess three, as its cave dwellings show, laterite two and a half, frozen ground
three). The surface's own blocks with no material are reckoned as turf (a metre, bound by
roots) or bare earth (half). Wood, leaves and plants are not reckoned and hold.

When a change opens a place (the server notes the player's changes and what falls; the water's
own flow opens nothing), each open place along the four ways through it, to its walls, has the
ground over it reckoned: the opening's width there — the open places along x and along z until
ground, a piece or land not loaded, the narrower — against how wide that ground roofs. Ground
over too wide an opening falls in: the block goes, and the same ground broken (a rock's loose
stones where it has them, else loose earth) lands on the floor below; the place it left is an
opening the next tick, so an undercut bank slumps and a wide tunnel in soil chimneys up to the
surface.

**Shoring.** A piece under ground holds it: a post, a wall, a beam across. The ground then
bears down on the piece as deep as the opening it stands in is wide (at most eight blocks; the
rest arches over), and the piece must bear that like any other load — so sapling poles under a
clay roof buckle at once and the roof comes down after them, while dry stone pillars or stout
timbers (V2-8 (g)) hold. A one-block tunnel in clay or firm soil needs nothing; in loam it
falls; in rock a hall stands.

## Planned (V2-8 (d)–(h))
Rain shed by pitch and material, leaks, rot in ground contact and mudbrick's erosion; shelter
quality for the body's exposure; a builder's view of load and stress; the Era 0–3 techniques
(thatch, wattle and daub, pit house, mudbrick, snow blocks, timber shoring).
