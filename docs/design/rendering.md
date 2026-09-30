# Rendering

*Status: implemented (v1 M3). Code: `crates/hearth_render`. Details: `ARCHITECTURE.md` §7,
`DECISIONS.md` D18–D19.*

## Purpose
Fast, correct voxel rendering that scales to large view distances.

## Model
Procedural textures in a texture array; greedy meshing of uniform faces into 16-byte packed
quads, 64-byte general quads for models/fluids/translucent; vertex pulling from sub-allocated
storage buffers; reverse-Z infinite projection with camera-relative origins; CPU cave culling;
two-phase Hi-Z GPU occlusion culling with indirect-count draws on Vulkan (CPU draw lists
elsewhere); translucent back-to-front with near re-sorting; headless screenshots with a GPU vs
CPU culling pixel check.

## v2 extensions (planned)
Seasonal tints as GPU parameters (V2-1), atmosphere/sky (V2-1), water and ice (V2-2),
passable foliage and branch models (V2-6), LOD showing vegetation state (V2-6), instanced
animals (V2-7), fire, smoke, glowing hot items (V2-5).
