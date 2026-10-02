# Building and structure

*Status: V2-8 in progress — (a) pieces in the world done. Pieces' data since V2-0
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

## Planned (V2-8 (b)–(h))
An incremental stability solver (support propagated from the ground and anchored rock, then a
load and span check from the materials' strengths) off the main thread with a budget; collapse
into physical debris; excavation and its shoring; rain shed by pitch and material, leaks, rot in
ground contact and mudbrick's erosion; shelter quality for the body's exposure; a builder's view
of load and stress; the Era 0–3 techniques (thatch, wattle and daub, pit house, mudbrick, snow
blocks, timber shoring).
