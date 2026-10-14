# S5 (c) review pictures: the trees in the world as their meshes (D300)

Seed 7, the Earth. Rendered by the screenshot tool on the cloud machine's software device, from
one build: `trees=blocks` for the old drawing, `trees=meshes` for the new.

- `walks.jpg`: at head height, before on the left and after on the right. Rows, top to bottom:
  - Temperate rainforest. A trunk of stacked cubes becomes a round, tapering stem.
  - Broadleaf forest. Walls of leaf blocks become crowns with light between them.
  - A mixed forest in autumn. The old drawing's leaf blocks show as dark squares; the new shows
    bare birches and green conifers.
  - Savanna. The baobab is a bottle-shaped trunk, not a stack of cubes.
- `from_40m.jpg`: boreal forest (top) and tropical rainforest (bottom) from 40 m; before on the
  left, after on the right. Within the render distance (96 m here) the trees are meshes. Beyond
  it they are still the distant terrain's boxes, which is L1's work.
- `winter_from_above.jpg`: a temperate rainforest's conifers in winter, from 30 m straight down.
  The sprays of needles are drawn where blocks were.
- `fall.jpg`: a birch felled with `fell=`, at 0, 50, 80 and 100 % of its fall. It turns down
  whole about its felling cut, faster as it goes, its crown coming to rest on the ground.
  - In play the stump is drawn once the world's vegetation has the felling; the screenshot tool
    fells the tree alone, so no stump is shown here.
- `outline.jpg`: what is looked at within reach, before on the left and after on the right.
  - Top, a limb (`look=block:_branch`). Before, one block glowed. After, the young spruce's
    whole stem and whorls glow along their shape.
  - Bottom, its foliage (`look=block:_leaves`). Before, a cube of leaves glowed. After, the needle
    sprays glow cut to their shapes. With every stripe of needles outlined, the foliage's glow
    is busy.
