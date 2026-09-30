# Migration: v1 → v2 ("Earth, Transposed")

v2 (`docs/spec/v2-direction-change.md`) keeps the v1 engine (`docs/spec/v1-build-prompt.md`)
and replaces the game built on it: a trimmed block game becomes a realistic Earth simulation.
This table records, for every v1 milestone and subsystem, what happens to it and where the
work now lives in the v2 plan (`PLAN.md`).

**Keep** = stays as is (bug fixes and extensions only). **Modify** = stays, with changes listed.
**Replace** = the purpose stays but the implementation/content is redone the v2 way.
**Drop** = removed; nothing replaces it.

## Milestones

| v1 milestone | State at the change | Decision | What changes / where it goes |
|---|---|---|---|
| M0 Foundation | done | **Keep** | Default key layout changes with the v2 interactions (§10.6) as they arrive (V2-3/V2-4); settings screens and conventions stay. |
| M1 Voxel core | done | **Keep** | Block registry gains material-derived properties (mass, strength) and generated block families (V2-0/V2-2). |
| M2 World generation | done | **Keep & extend** | Planet model, erosion, drainage and climate stay. Rock/ore palette, trees and plants are replaced (V2-2, V2-6); seasonal climate added (V2-1). |
| M3 Near-field rendering | done | **Keep & extend** | Seasonal tints move from baked vertex colours to GPU parameters (V2-1); branch models, passable foliage and canopy shade (V2-6). |
| M4 Player & interaction | not started | **Split** | Physics, collision, client/server split: **Keep** (V2-3). Movement constants: **Modify** to realistic speeds (V2-3). Breaking/placing: **Replace** with gathering, excavation volumes and processes (V2-5). Inventory/hotbar: **Replace** with hands/body/containers (V2-4). HUD: **Replace** with diegetic feedback (V2-3). Crafting/smelting: **Drop** → process crafting (V2-5). Containers: **Modify** into carried/placed containers (V2-4). Saving/loading: **Keep & extend** with versioning and migrations (V2-0). |
| M5 Light & fluids | light engine done | **Keep & modify** | Light engine stays (canopy becomes partial shade, V2-6). Water: sustained reservoirs plus finite, conserved player-moved water (V2-2). Random ticks stay (growth, phenology, snow). |
| M6 Sky & atmosphere | not started | **Keep & extend** | Built in V2-1 with axial tilt, seasonal sun path and day length by latitude, synodic moon, stars. |
| M7 Water rendering | not started | **Keep & extend** | Built in V2-2 (hydrology) with ice and seasonal levels; tides as a stretch goal. |
| M8 LOD | not started | **Keep & extend** | Built in V2-6: LOD tiles show the abstract vegetation state (species mix, season, snow, burn scars). |
| M9 Entities & AI | not started | **Replace (content) / Keep (infrastructure)** | ECS, models/animation, pathfinding stay as infrastructure (V2-7). The 8 mob types, darkness spawning, MC-style combat/drops/taming are dropped; fauna comes from population simulation, domestication from wild ancestors (V2-7, V2-12). |
| M10 Weather & effects | not started | **Keep & extend** | Weather cells become seasonal (V2-1); wildfire smoke (V2-6); snow cover and ice (V2-1). Shadows, bloom, particles as needed per milestone. |
| M11 UI & audio | not started | **Keep (framework) / Replace (game screens)** | UI toolkit, options screens, controller, audio engine, localization: built in V2-3. Inventory/HUD screens replaced (V2-3/V2-4); journal, body panel, character creator, globe spawn picker added (V2-3, V2-5, V2-15). |
| M12 Modding | not started | **Keep & extend** | Content platform (data packs with schemas, lint, hot reload) in V2-0; resource packs and WASM API in V2-15. |
| M13 Optimization | not started | **Keep** | Folded into V2-16 together with the v2 budgets (§21). |
| M14 Final QA | not started | **Keep** | Folded into V2-16. |

## Subsystems and content

| Subsystem / content | Decision | Notes |
|---|---|---|
| wgpu renderer, GPU culling, greedy meshing, texture array | Keep | Extended for foliage, animals, seasons (§15). |
| Cubic chunks, palette storage, wrap-aware coordinates, pole crossing | Keep | — |
| Planet sphere simulation, plates, erosion, drainage, climate, Köppen biomes | Keep & extend | Geological provinces, stratigraphy, soils, groundwater, reefs, seasonal climate (V2-1, V2-2). |
| Light engine (sky + block light) | Keep & modify | Foliage becomes partial shade instead of opaque to sky light (V2-6). |
| Options model, presets, key bindings, display modes, F11, frame limiter | Keep | New actions and defaults per §10.6 when the interactions exist. |
| Input action registry | Modify | Hotbar 1–9 → quick slots 1–6 + radial; drop → G; crouch C, prone Z, carry F, body B, journal J (V2-3/V2-4). |
| Time: 20-minute day, 8 moon phases, fixed latitude sun | Modify | Configurable day length (48 min default), calendar and seasons, axial tilt (V2-1). |
| Block set (MC-style stones, ores, woods, building blocks) | Replace | Rocks, minerals and soils from geology; woods from real species; construction pieces from data. Dropped outright in V2-0: ore blocks (coal/iron/copper/gold/diamond/redstone/lapis/emerald), crafting table, furnace, chest-as-workbench, bed, wool colours, bricks, glass, crops-as-blocks from MC. |
| Ore generation (MC Y-bands) | Replace | Deposit models placed by geology (V2-2); removed in V2-0. |
| Procedural trees (11 hand-made shapes) | Replace | Species growth models with real sizes (V2-6). The current generator stays until then; its three species (English oak, silver birch, Norway spruce) are real Appendix A species. |
| Plants and flowers (MC-style set) | Replace | Flora species data (V2-6). |
| Texture generator | Modify | Recipes driven by material/species appearance data instead of block names (V2-0 onward). |
| Block model naming rules in the renderer | Modify | Blocks declare their model kind in data (V2-0/V2-2) instead of name-based rules. |
| Tools/armor tiers (wood/stone/iron/gold/diamond) | Drop | Knowledge-based technology graph (V2-5, V2-13). |
| Crafting grid, furnace, recipe book | Drop | In-world processes, physical workstations, journal (V2-5). |
| XP, enchanting | Drop | Already out of scope. |
| Mobs (pig/cow/sheep/chicken/wolf, zombie/skeleton/spider) | Replace | Real fauna by ecosystem; predators instead of monsters (V2-7). |
| Darkness-based hostile spawning | Drop | Population simulation (V2-7). |
| Hunger bar, hearts, regeneration | Replace | Physiology model (V2-3). |
| 36-slot inventory + hotbar | Replace | Hands, body slots, clothing, containers with mass/volume (V2-4). |
| Beds that skip the night | Replace | Sleep with time acceleration (V2-3). |
| Player model & skin | Replace | Character creator, realistic proportions, clothing layers (V2-3). |
| Structures | Still excluded | Except hominin nests and tool scatters (V2-11); settlement data model designed for the future (§17). |
| Lava fluid | Still excluded | Volcanic landforms and rocks are in (V2-2). |
| Redstone, lapis | Still excluded | Azurite, malachite, ochre as pigments (V2-2, V2-13). |
| Saves | Keep & extend | Save format version 2 is the first written; format 1 (v1) is refused with a clear message. Registry mapping per save, versioned structs, migrations with fixture tests (V2-0). |
| Data packs | Keep & extend | RON (preferred) or JSON, schema-validated, cross-referenced, lintable, hot-reloadable (V2-0). |
| WASM modding | Keep | Later (V2-15); API surface grows with the v2 systems. |
| `bench` tool (worldmap/region/gen/textures) | Keep & extend | Geology, soil, deposit, vegetation and ecology layers. |
| `hearth --screenshot` | Keep | Screenshot suites per milestone (seasons × latitudes, species, ecosystems). |

## Compatibility

v1 never wrote world saves, so no v1 world can exist; the save loader still recognises format 1
and refuses it with a clear message, as §2 requires. From format 2 on, every persisted format
change comes with a migration step and a fixture test.
