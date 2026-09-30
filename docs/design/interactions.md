# Interaction matrix

How each system affects the others (v2 §18). Every row must be implemented by the end of the
milestones named; ✅ = implemented, ⏳ = planned (milestone).

| From ↓ / effect on → | What it changes | Status |
|---|---|---|
| **Planet & climate** → geology | Tectonic history assigns provinces; climate weathers rock into soils | ⏳ V2-2 |
| **Planet & climate** → ecosystems | Biomes from climate place ecosystems and their species | ⏳ V2-6/V2-7 (data linked in V2-0) |
| **Seasons** → plants | Leaf-out, flowering, fruiting, leaf fall, curing grass, snow on the ground | ⏳ V2-1/V2-6 |
| **Seasons** → animals | Births, rut, migration, hibernation, winter coats, lean-season boldness | ⏳ V2-7 |
| **Seasons** → body | Cold and heat stress, day length, food availability | ⏳ V2-3 |
| **Weather** → fire | Dry seasons and lightning ignite wildfires; rain puts them out | ⏳ V2-6 |
| **Weather** → water | Snowmelt floods, dry-season lows, freezing | ⏳ V2-1/V2-2 |
| **Water** → animals | Shrinking water holes concentrate prey and predators | ⏳ V2-7/V2-10 |
| **Fire** → vegetation | Burns reset succession to grassland; smoke visible from afar | ⏳ V2-6 |
| **Geology** → soils → plants | Parent rock and drainage set fertility, pH and which species thrive | ⏳ V2-2/V2-6 |
| **Geology** → knowledge | Toolstone, clay, ores, pigments exist only where deposits are; progress needs travel | ✅ lint reachability (V2-0); ⏳ placement V2-2 |
| **Plants** → herbivores | Forage and mast set carrying capacity | ⏳ V2-7 (diets linked in V2-0) |
| **Herbivores** → predators | Prey density sets predator numbers; predators limit herbivores | ⏳ V2-7 |
| **Predators** → player | Attacks for real causes; tracks, alarm calls and kills reveal them | ⏳ V2-7 |
| **Player** → ecosystem | Hunting pressure, clearing, fire, farming, waste attract scavengers | ⏳ V2-7/V2-12 |
| **Hominins** → player knowledge | Watching them grants observation insight | ⏳ V2-11 (routes in data V2-0) |
| **Body** ↔ environment | Cold/wet drive clothing, shelter and fire; heat drives water carrying | ⏳ V2-3/V2-4/V2-8 |
| **Knowledge** → processes | Nodes enable processes; processes need materials from geology and biology | ✅ data + lint (V2-0); ⏳ engine V2-5 |
| **Materials** → everything | Density → mass and carrying; strength → tools and structures; fuel → fire; knapping → edge quality | ✅ items derive from materials (V2-0) |
| **Structures** → body | Shelter quality feeds thermoregulation and sleep | ⏳ V2-8 |
| **Balance** → all systems | Realism preset multipliers | ✅ V2-0 |
