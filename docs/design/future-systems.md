# Future systems (Eras 6–8)

*Design only. Knowledge nodes for Eras 6–8 exist as planned data so the graph can grow without
restructuring (v2 §12.2). Completed in V2-14.*

These systems will be needed by later eras. The architecture leaves room for each.

## Mechanical power networks (Era 5 → 6)
Shafts, gears, belts and cams connecting sources (water wheel, windmill, animal gin) to
consumers (millstones, bellows, trip hammers, saws). Model: a graph of rotating components
solved per tick for speed and torque with losses; power sources and loads as data
(`workstations` with capabilities `Torque`/`Speed`). Water wheels read river flow from the
hydrology model; windmills read wind from weather.

## Fluids under pressure (Era 6–7)
Pipes, pumps, cisterns and boilers. Model: nodes with volume and head, pipes with flow
resistance; the finite-water system of V2-2 is the base. Steam as a separate fluid with
pressure and temperature.

## Heat engines and chemistry (Era 7)
Boilers, cylinders and condensers on top of the thermal model of V2-5; industrial processes
(smelting at scale, soda ash, sulfuric acid) as ordinary `processes` with larger stations.

## Electricity (Era 7–8)
Circuits as graphs of sources, conductors and loads (resistance, voltage, current), solved per
tick when changed. Generators couple to the mechanical network.

## Vehicles (Era 3 → 8)
Carts and boats first (V2-12/V2-14) as physical entities with mass, wheels/hulls and draft
animals; later self-propelled vehicles reuse them with engines.

## Communications
Signals, telegraph and radio as messages between nodes with latency, sharing the agent
framework's knowledge-transfer channel so information and ideas spread through simulated
societies.
