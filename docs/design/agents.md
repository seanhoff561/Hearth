# Agents: hominins now, simulated humans later

*Status: partial (V2-11). Data in `hominins/` and its population in `fauna/hominins.ron`;
code in `crates/hearth_fauna` (the population).*

## Purpose
v2 §8: sporadic small groups of *Australopithecus* in suitable habitat around the world — rare
enough that meeting them is memorable — built on a general **agent** framework that simulated
humans (v2 §17) will run on: a body, a mind, what it knows, what it carries, its group and its
group's ways, making things with the same processes as the player.

## Model
### The population (V2-11 (a))
A hominin species (`hominins/`) names its **population** in the ecological cells: an animal
entry (`fauna/hominins.ron`) saying what it eats, how it lives and dies, how far it ranges and
who hunts it. The population is simulated as the animals' groups are (D161): far from the
player its groups are numbers in their home ranges, feeding, breeding, dying, budding off; near
the player they are drawn out as the hominin's agents rather than as animals.

*Australopithecus* lives in the savanna–woodland mosaic and along the tropical waters (the
gallery forests and the lakeshores) of Africa, tropical Asia, South America and Australia — a
deliberate period liberty of the timeless Wild Earth (v2 §8.1) — in groups of five to
twenty-five, some 0.15 to the km² of good land: a few groups in a day's walk. It eats fruit
(marula, figs, baobab), nuts, pods and seeds, grubs and termites, and meat it scavenges; it
bears one young every four or five years from eleven; the leopard takes it most, and the lions,
hyenas, crocodiles, the crowned eagle (its young), the tiger and the jaguar now and then. The
world setting **Hominin range** keeps it to its cradle, Africa (`Single cradle region`), or lets
it live in all suitable habitat (the default).

## Parameters
- Density 0.15 /km², groups 5–25, home range 15 km², the females dispersing (as in chimpanzees).
- Maturity 11 years, a birth every 4.5 years, adult survival 0.95 a year (predation aside).

## Interactions
- Fauna: a prey of the big cats, hyenas, crocodiles and eagles; competes for fruit and nuts with
  the baboons, pigs and elephants.
- Settings: Hominin range.

## Known simplifications
- The population's numbers are the ecology's; its groups' members are counts until drawn out.

## Future extensions
- Agents (body, mind, knowledge, carrying, group, culture), their days, traces and learning by
  watching them (V2-11 (b)–(d)); simulated humans on the same framework (v2 §17).
