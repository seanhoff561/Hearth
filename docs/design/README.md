# Design documents

One short document per system (v2 §3.6): **purpose, model, parameters, interactions, known
simplifications, future extensions**. A system's doc is updated in the same commit as the
system. Status: *implemented*, *partial* or *planned* (the milestone that builds it).

| System | Doc | Status |
|---|---|---|
| Content platform (data, lint, graphs, hot reload) | [content-platform.md](content-platform.md) | implemented (V2-0) |
| Units and the two time scales | [time-scales.md](time-scales.md) | implemented (V2-0, calendar V2-1) |
| Balance layer and realism presets | [balance.md](balance.md) | implemented (V2-0) |
| Saves, versioning and migrations | [saves.md](saves.md) | implemented (V2-0) |
| Planet generation (tectonics, erosion, climate) | [planet.md](planet.md) | implemented (v1 M2) |
| Rendering (terrain, sky, lighting, weather) | [rendering.md](rendering.md) | implemented (v1 M3, V2-1) |
| Light | [light.md](light.md) | implemented (v1) |
| Calendar, seasons and weather | [seasons.md](seasons.md) | implemented (V2-1) |
| Geology, soils, hydrology, resources | [geology.md](geology.md) | implemented (V2-2) |
| Body and physiology | [physiology.md](physiology.md) | implemented (V2-3) |
| Movement (collision, gaits, climbing, swimming, falls) | [movement.md](movement.md) | implemented (V2-3b) |
| The world loop (server, client, protocol) | [world-loop.md](world-loop.md) | implemented in process (V2-3c) |
| Sound (procedural sounds, mixer, what the player hears) | [audio.md](audio.md) | implemented (V2-3d) |
| The player's person (appearance, rig, movement, views) | [character.md](character.md) | implemented (V2-3e) |
| What the body tells the player (senses, Body panel, Guided HUD) | [hud.md](hud.md) | implemented (V2-3f) |
| Inventory, carrying, clothing | [inventory.md](inventory.md) | implemented (V2-4) |
| Processes and the knowledge graph (crafting, discovery, skills, journal, knapping) | [knowledge-and-processes.md](knowledge-and-processes.md) | implemented for Eras 0–2 (V2-5) |
| Fire, cooking, preservation and food | [fire-and-food.md](fire-and-food.md) | implemented (V2-5) |
| Gathering by hand and digging | [gathering.md](gathering.md) | implemented (V2-5) |
| Flora | [flora.md](flora.md) | implemented (V2-6, V2-10) |
| Fauna and ecosystems | [fauna.md](fauna.md) | implemented (V2-7, V2-10) |
| Building and structure | [building.md](building.md) | implemented (V2-8) |
| Smooth terrain: fill, meshing, shading (Amendment S) | [smooth-terrain.md](smooth-terrain.md) | prototype (S0); S1–S8 |
| Art direction: photographic plausibility | [art-direction.md](art-direction.md) | living document (S0, Q) |
| Motion timing: what moves, by which clock, how fast | [motion-timing.md](motion-timing.md) | partial (P0); the audit and its check P5 |
| The Neolithic (pottery, fields, herds, cloth, timber, moving loads) | [neolithic.md](neolithic.md) | implemented (V2-12) |
| Menus and world management | [menus.md](menus.md) | implemented (P1) |
| Controls: bindings, lone modifiers, the controller, a click at nothing, blows | [controls.md](controls.md) | implemented (E2) |
| The interface's look: typefaces, panels, the journal's pages | [interface.md](interface.md) | implemented (Q1) |
| Game modes and Creative | [modes-creative.md](modes-creative.md) | implemented (P2) |
| Budgets: every system's cost | [budgets.md](budgets.md) | living document (Audit 0) |
| Interaction matrix | [interactions.md](interactions.md) | living document |
| Future systems (Eras 6–8) | [future-systems.md](future-systems.md) | design only |
| Simulated humanity (Phase F): minds, societies, history, eras | [future/humanity/README.md](future/humanity/README.md) | design only (E1) |
| Simulated humans of V2.1 (removed by Amendment E) | [../archive/humans-v2.1/README.md](../archive/humans-v2.1/README.md) | archived (E0) |
