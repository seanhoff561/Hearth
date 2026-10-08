# Gathering by hand and digging

*Status: implemented (V2-5, D72). Data: the gathering processes in
`processes/paleolithic.ron`, the plant blocks in `blocks/craft.json`; code:
`crates/hearth/src/workshop.rs` (effects on blocks, spoil and slumping), worldgen's placement of
useful plants (`hearth_worldgen/src/cubegen/features.rs`).*

## Purpose
v2 §11.1–11.2: with nothing but hands a person can gather loose stones, deadwood, grass,
fibre, fruit and nuts in season, dig loose ground slowly and drink; never punch down a tree or
break rock. What is dug is real earth that has to go somewhere.

## Model
- **Gathering** is processes with a block target, open to anyone:
  - loose stones (two or three cobbles, or a flat stone for an anvil);
  - dead branches and twigs from trees, a dead sapling for a pole, a dead bush broken up;
  - hazel rods (with an edge);
  - dry grass from dry grassland, dead stems from tussocks all year, dead grass in autumn and
    winter;
  - grass seed, hazelnuts, acorns, blackberries, nettle tops, nettle fibre in their seasons;
  - birch bark, tinder fungus (on some birches), spruce resin, reeds, vines;
  - clay and ochre by the lump (a bank goes once its whole mass is taken);
  - grubs from rotten wood with a twig.
  Each block yields a process only so often a year (a tree's dead branches thrice, a bush's
  berries thrice, a birch's bark once), kept per block.
- **Useful plants** (stand-ins until the flora framework, V2-6): nettle patches, hazel bushes
  (two blocks tall) and brambles in the temperate woods and along rivers.
- **Digging** (a cubic metre by hand in 4 h, with a digging stick 2 h, snow 0.5 h; S1): each
  stroke takes its share of the metre from the ground where the digger looks, as a bowl
  (`smooth-terrain.md`, "Digging and slumping"). The loose earth (*spoil*) is thrown a stride
  clear of the hole, away from the digger; snow is packed away. Loose ground (spoil, sand,
  gravel, ash, snow) slumps until no slope of it stands steeper than its angle of repose, dry
  or rain-wet (sand 34°, gravel 40°, loose earth 37°); intact earth stands in a pit's wall.
- **Throwing** (hold R, let go): a stone flies from the hand at up to 20 m/s (slower for heavy
  things), falls ballistically and lies where it lands.

## Parameters
Durations, yields, seasons and harvests per block in the data; the angles of repose per ground
family in `materials/reference.ron`.

## Interactions
Knowledge (gathering teaches: knocking stones, cutting, probing, seeing vines), carrying (what
is gathered is stowed, or put down at the feet), seasons, worldgen (plants placed with the
woods), blocks remeshed and relit as they change.

## Known simplifications
- Spoil is one kind of loose earth whatever was dug; placing earth back to build piles and
  ramps comes with building (V2-8).
- Per-block harvest counts stand in for plants' own growth and fruiting (V2-6).

## Future extensions
Real plant models with their parts and phenology (V2-6), quarrying and mining (V2-13).
