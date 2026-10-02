//! What the populations lacked for the cold lands (V2-10 (a), D108–D110): migrants winter
//! elsewhere; a hunter is judged by its prey, so a fox's berries do not make the tundra a desert
//! to it and a wolf of the tundra is rarer than a wolf of the oak woods; cold-blooded animals
//! need warm months; dwarf shrubs are browse on cold open ground.

use std::sync::Arc;

use hearth_content::Content;
use hearth_fauna::ecology::{Ecology, REGION_LEN, about, away};
use hearth_fauna::habitat::{Habitat, Uniform, dwarf_shrubs, temperate_wood};
use hearth_fauna::species::{Catalog, Forage};
use hearth_worldgen::realms::Realm;

fn catalog() -> Arc<Catalog> {
    Arc::new(Catalog::new(&Content::load_base()))
}

/// The ecosystems' bits for some ids.
fn bits(cat: &Catalog, ids: &[&str]) -> u32 {
    ids.iter()
        .map(|id| {
            cat.ecosystems
                .iter()
                .position(|e| e.id.ends_with(id))
                .map_or(0, |i| 1u32 << i)
        })
        .fold(0, |a, b| a | b)
}

/// Low-arctic tundra: a short cool summer, grass and sedge, dwarf shrubs, no trees, no mast.
fn tundra(cat: &Catalog) -> Habitat {
    let wood = temperate_wood(cat);
    let mut h = wood;
    h.temp_c = -8.0;
    h.warm_c = 8.0;
    h.precip_mm = 300.0;
    h.cover = 0.1;
    h.ecosystems = bits(cat, &["tundra_ecosystem"]);
    h.forage = [0.0; hearth_fauna::species::FORAGE_KINDS];
    h.forage[Forage::Graze as usize] = 0.9 * wood.forage[Forage::Graze as usize];
    h.forage[Forage::Browse as usize] = 0.4 * wood.forage[Forage::Browse as usize];
    h.forage[Forage::Fruit as usize] = 0.02 * wood.forage[Forage::Fruit as usize];
    h.forage[Forage::Seeds as usize] = 0.5 * wood.forage[Forage::Seeds as usize];
    h.forage[Forage::Invertebrates as usize] = 0.2 * wood.forage[Forage::Invertebrates as usize];
    h.fresh = 0.0;
    h.land = 1.0;
    h
}

fn ecology(cat: &Arc<Catalog>, h: Habitat) -> Ecology {
    let land = Uniform {
        habitat: h,
        cells_around: 512,
    };
    Ecology::new(cat.clone(), 1, 0.0, &land)
}

#[test]
fn migrants_winter_elsewhere_and_residents_stay() {
    let cat = catalog();
    let h = tundra(&cat);
    let bunting = cat.get("hearth:snow_bunting").expect("snow bunting");
    let ptarmigan = cat.get("hearth:rock_ptarmigan").expect("rock ptarmigan");
    assert!(bunting.migrates && !ptarmigan.migrates);
    // January (0.8 of the year from the March equinox) and July (0.3).
    assert!(away(bunting, &h, 0.8) && !about(bunting, &h, 0.8));
    assert!(!away(bunting, &h, 0.3) && about(bunting, &h, 0.3));
    assert!(about(ptarmigan, &h, 0.8));
    // A migrant of the temperate lowlands, where no month freezes hard, stays.
    let robin = cat.get("hearth:american_robin").expect("American robin");
    assert!(!away(robin, &temperate_wood(&cat), 0.8));
}

#[test]
fn hunters_are_judged_by_their_prey() {
    let cat = catalog();
    let wood = temperate_wood(&cat);
    let mut tundra = tundra(&cat);
    tundra.realm = Realm::Palearctic as u8;
    tundra.fauna = Realm::Palearctic as u8;
    let eco = ecology(&cat, wood);
    let mean = |q: &[f32], id: &str| {
        let s = cat.index(id).expect(id);
        q[s * REGION_LEN..(s + 1) * REGION_LEN].iter().sum::<f32>() / REGION_LEN as f32
    };
    let (in_wood, _) = eco.qualities(&vec![wood; REGION_LEN]);
    let (in_tundra, _) = eco.qualities(&vec![tundra; REGION_LEN]);
    // A wolf of the oak woods has deer and boar by the dozen, a wolf of the tundra a few
    // reindeer and musk oxen.
    let (w0, w1) = (mean(&in_wood, "gray_wolf"), mean(&in_tundra, "gray_wolf"));
    println!("gray wolf: wood {w0:.2}, tundra {w1:.2}");
    assert!(w0 > 0.6, "a wolf in the reference wood: {w0:.2}");
    assert!(w1 > 0.02 && w1 < 0.6 * w0, "a wolf of the tundra: {w1:.2}");
    // The arctic fox eats a few crowberries; the berryless tundra is not a desert to it.
    let fox = mean(&in_tundra, "arctic_fox");
    println!("arctic fox: tundra {fox:.2}");
    assert!(fox > 0.3, "an arctic fox of the tundra: {fox:.2}");
    // The red deer cannot live there at all, and is not counted for its hunters there.
    assert_eq!(mean(&in_tundra, "red_deer"), 0.0);
}

#[test]
fn cold_blooded_animals_need_warm_months() {
    let cat = catalog();
    let wood = temperate_wood(&cat);
    let eco = ecology(&cat, wood);
    let mean = |q: &[f32], id: &str| {
        let s = cat.index(id).expect(id);
        q[s * REGION_LEN..(s + 1) * REGION_LEN].iter().sum::<f32>() / REGION_LEN as f32
    };
    // The same wood with the summers of the far north: no month above 10 °C for a snake, a few
    // weeks above 8 °C for a frog.
    let mut north = wood;
    north.temp_c = -4.0;
    north.warm_c = 9.5;
    let (warm, _) = eco.qualities(&vec![wood; REGION_LEN]);
    let (cold, _) = eco.qualities(&vec![north; REGION_LEN]);
    let (a0, a1) = (
        mean(&warm, "common_european_adder"),
        mean(&cold, "common_european_adder"),
    );
    let (f0, f1) = (mean(&warm, "common_frog"), mean(&cold, "common_frog"));
    println!("adder {a0:.2} -> {a1:.2}, common frog {f0:.2} -> {f1:.2}");
    assert!(a0 > 0.4, "an adder in the reference wood: {a0:.2}");
    assert_eq!(a1, 0.0, "an adder where no month passes 10 °C");
    assert!(
        f1 > 0.0 && f1 < 0.5 * f0,
        "a frog of the north: {f1:.2} of {f0:.2}"
    );
    // The warm-blooded do not care.
    assert!((mean(&warm, "roe_deer") - mean(&cold, "roe_deer")).abs() < 1e-6);
}

#[test]
fn dwarf_shrubs_grow_on_cold_open_ground() {
    // None in the temperate lowlands, half the open ground's growth on the low-arctic tundra,
    // less again on the polar desert's cushions and mosses.
    assert_eq!(dwarf_shrubs(18.0), 0.0);
    assert!((dwarf_shrubs(8.0) - 0.5).abs() < 1e-6);
    let polar = dwarf_shrubs(3.0);
    assert!(polar > 0.1 && polar < 0.35, "{polar}");
    let bog = dwarf_shrubs(13.5);
    assert!(bog > 0.1 && bog < 0.4, "{bog}");
}
