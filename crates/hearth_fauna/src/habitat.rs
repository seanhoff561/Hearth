//! What a 256 m ecological cell offers its animals (v2 §7.4): its shares of land, fresh water
//! and sea, the forage of each kind it produces through the year, cover, its ecosystems and
//! realm, and its climate — sampled from the world generator and the vegetation state.
//!
//! Production follows the Miami model of net primary production (Lieth 1975) from the yearly
//! mean temperature and precipitation, shared out by the vegetation: grass and herbs where the
//! canopy lets light through, browse where young trees and shrubs grow back after a clearing,
//! a fire or a fall, mast under the nut trees once they are old enough to bear, fruit at the
//! woods' edges, seeds of the herbs, the soil's earthworms and grubs, fungi in the woods and
//! nectar where things flower. The amounts are what animals can reach and use, calibrated so
//! that a temperate mixed wood feeds its fauna at the densities the species data give.

use hearth_worldgen::WorldGenerator;
use hearth_worldgen::planet::climate::{ClimateClass, dry_season};
use hearth_worldgen::realms::Realm;
use hearth_worldgen::vegetation::{DisturbanceKind, Vegetation};
use serde::{Deserialize, Serialize};

use crate::species::{Catalog, FORAGE_KINDS, Forage};

/// An ecological cell's side, m.
pub const CELL_M: f64 = 256.0;
/// An ecological cell's area, km².
pub const CELL_KM2: f32 = 0.065_536;

/// What a cell offers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Habitat {
    /// Shares of the cell that are land, fresh water and sea (they sum to 1).
    pub land: f32,
    pub fresh: f32,
    pub sea: f32,
    /// Usable production of each kind of forage in a year, kg (dry matter; fresh for
    /// invertebrates) per km² of land (of fresh water for the aquatic invertebrates), in an
    /// average year.
    pub forage: [f32; FORAGE_KINDS],
    /// How much of the cell hides an animal (canopy, thickets, tall herbs), 0–1.
    pub cover: f32,
    /// Bits of the catalog's ecosystems present.
    pub ecosystems: u32,
    pub realm: u8,
    /// The realm whose animals live here: the cell's own, or the stand-in's where the cell's
    /// realm has none of its own for its ecosystems (set by the ecology).
    #[serde(default)]
    pub fauna: u8,
    pub island: bool,
    /// Yearly mean and warmest-month temperature, °C, and precipitation, mm.
    pub temp_c: f32,
    pub warm_c: f32,
    pub precip_mm: f32,
    /// South of the equator (seasons half a year on).
    pub southern: bool,
    /// Its dry season ([`dry_season`]): none, in the months of the low sun (the savannas), or in
    /// the hot months (the Mediterranean's).
    #[serde(default)]
    pub dry: u8,
}

impl Default for Habitat {
    fn default() -> Self {
        Self {
            land: 0.0,
            fresh: 0.0,
            sea: 1.0,
            forage: [0.0; FORAGE_KINDS],
            cover: 0.0,
            ecosystems: 0,
            realm: Realm::Oceanian as u8,
            fauna: Realm::Oceanian as u8,
            island: false,
            temp_c: 10.0,
            warm_c: 18.0,
            precip_mm: 700.0,
            southern: false,
            dry: dry_season::NONE,
        }
    }
}

/// Net primary production by the Miami model, g of dry matter per m² a year: the lesser of
/// the limits set by temperature and by precipitation.
pub fn miami_npp(temp_c: f32, precip_mm: f32) -> f32 {
    let by_t = 3000.0 / (1.0 + (1.315 - 0.119 * temp_c).exp());
    let by_p = 3000.0 * (1.0 - (-0.000_664 * precip_mm.max(0.0)).exp());
    by_t.min(by_p).max(0.0)
}

/// A bell over the year (fraction `f`) about `centre`, `width` wide (a standard deviation),
/// wrapping at the year's end.
fn bell(f: f32, centre: f32, width: f32) -> f32 {
    let mut d = (f - centre).rem_euclid(1.0);
    if d > 0.5 {
        d -= 1.0;
    }
    (-0.5 * (d / width) * (d / width)).exp()
}

impl Habitat {
    pub fn realm(&self) -> Realm {
        Realm::ALL[(self.realm as usize).min(7)]
    }

    /// The realm whose animals live here.
    pub fn fauna(&self) -> Realm {
        Realm::ALL[(self.fauna as usize).min(7)]
    }

    /// The year fraction as the cell's seasons run (the south half a year on).
    pub fn local_frac(&self, f: f32) -> f32 {
        if self.southern {
            (f + 0.5).rem_euclid(1.0)
        } else {
            f.rem_euclid(1.0)
        }
    }

    /// The month's mean temperature at year fraction `f` (0 at the March equinox, north),
    /// °C: warmest about late July.
    pub fn temp_at(&self, f: f32) -> f32 {
        let f = self.local_frac(f);
        let amp = (self.warm_c - self.temp_c).max(0.0);
        self.temp_c + amp * (std::f32::consts::TAU * (f - 0.07)).sin()
    }

    /// The share of the year whose months are warmer than `t` °C.
    pub fn share_above(&self, t: f32) -> f32 {
        let amp = self.warm_c - self.temp_c;
        if amp <= 1e-3 {
            return if self.temp_c > t { 1.0 } else { 0.0 };
        }
        let x = ((t - self.temp_c) / amp).clamp(-1.0, 1.0);
        0.5 - x.asin() / std::f32::consts::PI
    }

    /// How fast plants grow at year fraction `f`, 0–1: by the warmth, and the moisture of the
    /// season ([`Self::wet_at`]).
    pub fn growth_at(&self, f: f32) -> f32 {
        ((self.temp_at(f) - 4.0) / 12.0).clamp(0.0, 1.0) * self.wet_at(f)
    }

    /// How moist the season is for growing at year fraction `f`, 0–1: all year where it rains
    /// all year; where the low sun's months are dry (the savannas) the grass browns from the
    /// rains' end to their return, and where the summer is (the Mediterranean) the hot months
    /// stand still.
    pub fn wet_at(&self, f: f32) -> f32 {
        let lf = self.local_frac(f);
        match self.dry {
            dry_season::WINTER => 0.1 + 0.9 * bell(lf, 0.3, 0.13),
            dry_season::SUMMER => 1.0 - 0.8 * bell(lf, 0.33, 0.1),
            _ => 1.0,
        }
    }

    /// Snow lying at year fraction `f`, 0 (none) to about 1.5 (deep).
    pub fn snow_at(&self, f: f32) -> f32 {
        let t = self.temp_at(f);
        if t >= 0.5 {
            return 0.0;
        }
        ((0.5 - t) / 8.0).clamp(0.0, 1.0) * (self.precip_mm / 800.0).clamp(0.2, 1.5)
    }

    /// How much of a kind of forage is to be had at year fraction `f`, before snow and the
    /// mast year, against no particular scale (see [`Self::avail_mean`]): grass and twigs
    /// grow in the warm months and stand through the winter, thinner; nuts fall in autumn and
    /// are eaten, rot and sprout through the winter and spring; fruit, fungi and nectar have
    /// their weeks (haws and hips hang into the winter); worms and grubs are most to be had in
    /// warm moist soil, fewer in the leaf litter of winter.
    pub fn raw_avail(&self, k: Forage, f: f32) -> f32 {
        let lf = self.local_frac(f);
        let wet = (self.precip_mm / (self.precip_mm + 400.0)).clamp(0.0, 1.0);
        let growth = self.growth_at(f);
        match k {
            Forage::Graze => 0.4 + growth,
            Forage::Browse => 0.8 + 0.6 * growth,
            Forage::Mast => (-((lf - 0.52).rem_euclid(1.0)) / 0.35).exp(),
            Forage::Fruit => 0.1 + bell(lf, 0.42, 0.08),
            Forage::Seeds => 0.3 + bell(lf, 0.45, 0.1),
            Forage::Invertebrates => {
                (0.25 + 0.75 * ((self.temp_at(f) - 2.0) / 10.0).clamp(0.0, 1.0))
                    * wet
                    * (0.4 + 0.6 * self.wet_at(f))
            }
            Forage::Fungi => 0.05 + bell(lf, 0.55, 0.07) * wet,
            Forage::Nectar => bell(lf, 0.25, 0.1) * growth,
            Forage::Aquatic => 0.3 + ((self.temp_at(f) - 1.0) / 12.0).clamp(0.0, 1.0),
        }
    }

    /// The yearly mean of [`Self::raw_avail`].
    pub fn avail_mean(&self, k: Forage) -> f32 {
        const N: usize = 24;
        let s: f32 = (0..N)
            .map(|i| self.raw_avail(k, (i as f32 + 0.5) / N as f32))
            .sum();
        (s / N as f32).max(1e-6)
    }

    /// How much of a kind of forage is to be had at year fraction `f` against its yearly mean
    /// (1), under `snow` and in a year whose mast crop is `mast` times the usual.
    pub fn availability(&self, k: Forage, f: f32, snow: f32, mast: f32, mean: f32) -> f32 {
        let buried = match k {
            Forage::Graze | Forage::Seeds | Forage::Invertebrates => 1.0 - (0.7 * snow).min(0.85),
            Forage::Fruit | Forage::Fungi | Forage::Mast => 1.0 - (0.5 * snow).min(0.8),
            _ => 1.0,
        };
        let crop = if k == Forage::Mast { mast } else { 1.0 };
        self.raw_avail(k, f) / mean.max(1e-6) * buried * crop
    }

    /// The coldest month's mean temperature (°C), as far as the year's mean and its warmest
    /// month tell it.
    pub fn coldest_c(&self) -> f32 {
        2.0 * self.temp_c - self.warm_c
    }

    /// Land area of the cell, km².
    pub fn land_km2(&self) -> f32 {
        self.land * CELL_KM2
    }

    /// Fresh water area of the cell, km².
    pub fn fresh_km2(&self) -> f32 {
        self.fresh * CELL_KM2
    }

    /// The area a kind of forage grows on in the cell, km².
    pub fn area_of(&self, k: Forage) -> f32 {
        if k == Forage::Aquatic {
            self.fresh_km2()
        } else {
            self.land_km2()
        }
    }
}

/// A source of habitats: the generated world, or a made-up one in tests.
pub trait Land: Send + Sync {
    /// The habitat of the cell `(i, j)` (cell `(i, j)` spans x from `i × 256` and z from
    /// `j × 256`), with the vegetation as it stands.
    fn habitat(&self, cell: (i64, i64)) -> Habitat;
    /// The circumference in cells (the cells wrap east–west), and the rows of the planet
    /// (`-rows..rows` in z).
    fn cells_around(&self) -> i64;
    fn rows(&self) -> i64;
}

/// Mast and fruit a canopy tree bears in an average year, kg of dry matter, by the forest's
/// species index.
#[derive(Debug, Clone, Default)]
pub struct TreeYields {
    pub mast: Vec<f32>,
    pub fruit: Vec<f32>,
    pub nectar: Vec<bool>,
}

impl TreeYields {
    pub fn new(wg: &WorldGenerator, content: &hearth_content::Content) -> Self {
        use hearth_content::schema::flora::PartKind;
        let n = wg.forest.templates.species.len();
        let mut out = Self {
            mast: vec![0.0; n],
            fruit: vec![0.0; n],
            nectar: vec![false; n],
        };
        for (i, sp) in wg.forest.templates.species.iter().enumerate() {
            let Some(p) = content.plants.get(&sp.id) else {
                continue;
            };
            for part in &p.parts {
                // The lower end of the yield in an ordinary year, half of it dry matter.
                let y = part.yield_kg.map_or(0.0, |(lo, hi)| lo + 0.25 * (hi - lo)) * 0.5;
                match part.part {
                    // Nuts, and the seed crops of the conifers' cones.
                    PartKind::Nut | PartKind::Seed => out.mast[i] += y,
                    PartKind::Fruit => out.fruit[i] += y,
                    PartKind::Flower | PartKind::Sap => out.nectar[i] = true,
                    _ => {}
                }
            }
            // Lime, cherry, apple, hawthorn, willow and maple flower for the bees.
            let id = sp.id.as_str();
            if [
                "lime", "cherry", "apple", "hawthorn", "willow", "maple", "chestnut",
            ]
            .iter()
            .any(|w| id.contains(w))
            {
                out.nectar[i] = true;
            }
        }
        out
    }
}

/// The generated world as a source of habitats.
pub struct GenLand<'a> {
    pub wg: &'a WorldGenerator,
    pub veg: &'a Vegetation,
    pub catalog: &'a Catalog,
    pub trees: &'a TreeYields,
}

/// Usable production in a temperate mixed wood at the reference NPP (1200 g/m²/yr), kg per km²
/// a year: open grassland's graze, young regrowth's browse, a nut wood's mast and so on.
const GRAZE_OPEN: f32 = 90_000.0;
const BROWSE_YOUNG: f32 = 66_000.0;
const BROWSE_OLD: f32 = 8_400.0;
const FRUIT_EDGE: f32 = 12_000.0;
const INVERTEBRATES: f32 = 9_000.0;
const FUNGI: f32 = 1_500.0;
const NECTAR: f32 = 6_000.0;
/// Usable invertebrates of fresh water, kg (fresh) per km² of water a year (streams and lake
/// shallows produce hundreds of tonnes).
const AQUATIC: f32 = 250_000.0;
/// A mature canopy tree's crown, m².
const CROWN_M2: f32 = 110.0;
/// The share of a dwarf shrub's growth (leaves, buds, catkins, shoot tips) that is usable
/// browse, against a grass's.
const HEATH_USE: f32 = 0.6;

/// The share of open ground's growth in dwarf shrubs where the warmest month is `warm_c` (°C):
/// none in the temperate lowlands, rising through the boreal bogs and burns to half on the
/// low-arctic tundra (dwarf birch, willows, heaths), falling again to the cushions and mosses
/// of the polar desert.
pub fn dwarf_shrubs(warm_c: f32) -> f32 {
    0.5 * smooth(warm_c, 1.0, 5.0) * (1.0 - smooth(warm_c, 11.0, 16.0))
}

/// The dry season of a climate: the savannas' and the hot steppes' in the months of the low sun,
/// the Mediterranean's in its summer.
pub fn dry_of(climate: ClimateClass) -> u8 {
    match climate {
        ClimateClass::TropicalSavanna | ClimateClass::HotSteppe => dry_season::WINTER,
        ClimateClass::Mediterranean => dry_season::SUMMER,
        _ => dry_season::NONE,
    }
}

/// The share of open ground's growth in shrubs where the year's rain is `precip_mm`: a little
/// in the steppes, half in the deserts (creosote, sagebrush, saltbush, saxaul).
pub fn desert_shrubs(precip_mm: f32) -> f32 {
    0.15 * (1.0 - smooth(precip_mm, 400.0, 600.0)) + 0.35 * (1.0 - smooth(precip_mm, 150.0, 350.0))
}

impl GenLand<'_> {
    /// One column's contribution: (land, fresh, sea, forage per km², cover, ecosystems).
    fn column(&self, x: i32, z: i32, roll: f32) -> Column {
        let wg = self.wg;
        let s = wg.terrain.sample(x, z);
        let mut col = Column {
            temp_c: s.temperature,
            warm_c: s.t_warm,
            precip_mm: s.precipitation,
            realm: s.realm,
            dry: dry_of(s.climate),
            ..Column::default()
        };
        if s.ocean && s.is_underwater() {
            col.sea = 1.0;
            return col;
        }
        col.ecosystems = self.catalog.ecosystems_of_biome(s.biome.name());
        if s.is_underwater() {
            col.fresh = 1.0;
            col.forage[Forage::Aquatic as usize] =
                AQUATIC * (miami_npp(s.temperature, s.precipitation) / 1200.0).min(1.5);
            return col;
        }
        col.land = 1.0;
        if s.biome == hearth_worldgen::region::biome::Biome::Wetland {
            col.fresh = 0.3;
            col.land = 0.7;
            col.forage[Forage::Aquatic as usize] =
                AQUATIC * (miami_npp(s.temperature, s.precipitation) / 1200.0).min(1.5);
        }
        // A river running by.
        if let Some(r) = s.river
            && r.distance < r.width * 0.5 + 24.0
        {
            col.ecosystems |= self.catalog.ecosystems_of_biome("river");
        }
        // The canopy as the vegetation has grown it.
        let (canopy, young, mast, fruit_tree, flowers) =
            match wg.features().expected_canopy(wg, self.veg, &s, x, z, roll) {
                Some((sp, _h, cover)) => {
                    let (age, opened) = match self.veg.ground(x, z) {
                        Some((kind, since)) if kind != DisturbanceKind::Felled => (since, true),
                        _ => (wg.features().stand_age(x, z) + self.veg.year as f32, false),
                    };
                    // Regrowth is thick and in reach from a few years to about thirty.
                    let young = if opened {
                        smooth(age, 2.0, 6.0) * (1.0 - smooth(age, 20.0, 40.0))
                    } else {
                        0.0
                    };
                    let bearing = smooth(age, 25.0, 70.0);
                    let trees_km2 = cover * 1.0e6 / CROWN_M2;
                    (
                        cover,
                        young,
                        trees_km2 * self.trees.mast.get(sp).copied().unwrap_or(0.0) * bearing,
                        trees_km2 * self.trees.fruit.get(sp).copied().unwrap_or(0.0) * bearing,
                        self.trees.nectar.get(sp).copied().unwrap_or(false),
                    )
                }
                None => (s.tree_density.min(1.0) * 0.3, 0.0, 0.0, 0.0, false),
            };
        let stand = Stand {
            canopy,
            young,
            mast,
            fruit_tree,
            flowers,
        };
        (col.forage, col.cover) = stand.forage(s.temperature, s.t_warm, s.precipitation);
        col
    }
}

/// The vegetation of a column as its forage reads it: the canopy's cover, the young regrowth,
/// the mast and fruit of its trees (kg a km²), whether they flower for the bees.
#[derive(Debug, Clone, Copy, Default)]
struct Stand {
    canopy: f32,
    young: f32,
    mast: f32,
    fruit_tree: f32,
    flowers: bool,
}

impl Stand {
    /// The usable forage of each kind a km² of such a stand produces in a year in a climate
    /// (yearly mean and warmest month °C, rain mm), and the cover it gives.
    fn forage(&self, temp_c: f32, warm_c: f32, precip_mm: f32) -> ([f32; FORAGE_KINDS], f32) {
        let Stand {
            canopy,
            young,
            mast,
            fruit_tree,
            flowers,
        } = *self;
        let n = miami_npp(temp_c, precip_mm) / 1200.0;
        let light = (1.0 - canopy).clamp(0.0, 1.0);
        let edge = 4.0 * canopy * light;
        let wet = (precip_mm / (precip_mm + 400.0)).clamp(0.0, 1.0);
        let heath = dwarf_shrubs(warm_c).max(desert_shrubs(precip_mm));
        let open = GRAZE_OPEN * n * light * light * 0.95;
        let mut f = [0.0f32; FORAGE_KINDS];
        f[Forage::Graze as usize] = open * (1.0 - heath) + GRAZE_OPEN * n * 0.05;
        f[Forage::Browse as usize] = n
            * (BROWSE_OLD * (0.3 + canopy) + BROWSE_YOUNG * young + 0.1 * GRAZE_OPEN * edge * 0.1)
            + open * heath * HEATH_USE;
        f[Forage::Mast as usize] = mast * 0.5;
        f[Forage::Fruit as usize] =
            FRUIT_EDGE * n * (edge * 0.8 + young * 0.6 + 0.1) + fruit_tree * 0.4;
        f[Forage::Seeds as usize] = 0.08 * f[Forage::Graze as usize];
        f[Forage::Invertebrates as usize] = INVERTEBRATES * n.min(1.5) * wet * 1.6;
        f[Forage::Fungi as usize] = FUNGI * n * canopy * wet * 1.6;
        f[Forage::Nectar as usize] =
            NECTAR * n * (0.3 * light + edge * 0.3 + if flowers { canopy } else { 0.0 });
        let cover = (canopy * 0.7 + young * 0.9 + light * 0.1).clamp(0.0, 1.0);
        (f, cover)
    }
}

/// The habitat of an ecosystem's reference land (its own ecosystem's, the animals of a realm).
pub fn reference_land(
    catalog: &Catalog,
    r: &hearth_content::schema::ecosystem::ReferenceLand,
    ecosystem: &str,
    realm: Realm,
) -> Habitat {
    let stand = Stand {
        canopy: r.canopy.clamp(0.0, 1.0),
        young: r.young.clamp(0.0, 1.0),
        mast: r.mast_kg.max(0.0),
        fruit_tree: r.fruit_kg.max(0.0),
        flowers: r.flowers,
    };
    let (forage, cover) = stand.forage(r.temp_c, r.warm_c, r.precip_mm);
    let mut h = open_land(
        catalog,
        r.temp_c,
        r.warm_c,
        r.precip_mm,
        &[ecosystem],
        realm,
    );
    h.forage = forage;
    h.cover = cover;
    if r.fresh > 0.0 {
        h.fresh = r.fresh.clamp(0.0, 0.9);
        h.land = 1.0 - h.fresh;
        h.forage[Forage::Aquatic as usize] =
            AQUATIC * (miami_npp(r.temp_c, r.precip_mm) / 1200.0).min(1.5);
    }
    h
}

/// Open land (a grassland, a steppe, a desert) of a climate (yearly mean and warmest month
/// °C, rain mm), of the ecosystems named, whose animals are a realm's: what the generated world
/// makes of such a place without its trees (tests, calibration).
pub fn open_land(
    catalog: &Catalog,
    temp_c: f32,
    warm_c: f32,
    precip_mm: f32,
    ecosystems: &[&str],
    realm: Realm,
) -> Habitat {
    let (forage, cover) = Stand::default().forage(temp_c, warm_c, precip_mm);
    let eco = ecosystems
        .iter()
        .map(|id| {
            catalog
                .ecosystems
                .iter()
                .position(|e| e.id.ends_with(id))
                .map_or(0, |i| 1u32 << i)
        })
        .fold(0, |a, b| a | b);
    Habitat {
        land: 1.0,
        fresh: 0.0,
        sea: 0.0,
        forage,
        cover,
        ecosystems: eco,
        realm: realm as u8,
        fauna: realm as u8,
        island: false,
        temp_c,
        warm_c,
        precip_mm,
        southern: false,
        dry: dry_season::NONE,
    }
}

#[derive(Debug, Clone, Copy)]
struct Column {
    land: f32,
    fresh: f32,
    sea: f32,
    forage: [f32; FORAGE_KINDS],
    cover: f32,
    ecosystems: u32,
    realm: Realm,
    temp_c: f32,
    warm_c: f32,
    precip_mm: f32,
    dry: u8,
}

impl Default for Column {
    fn default() -> Self {
        Self {
            land: 0.0,
            fresh: 0.0,
            sea: 0.0,
            forage: [0.0; FORAGE_KINDS],
            cover: 0.0,
            ecosystems: 0,
            realm: Realm::Palearctic,
            temp_c: 0.0,
            warm_c: 0.0,
            precip_mm: 0.0,
            dry: dry_season::NONE,
        }
    }
}

fn smooth(x: f32, a: f32, b: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Land for GenLand<'_> {
    fn habitat(&self, (i, j): (i64, i64)) -> Habitat {
        let planet = self.wg.planet();
        let c = planet.circumference() as i64;
        let (x0, z0) = (i * CELL_M as i64, j * CELL_M as i64);
        if z0 < -c / 2 || z0 >= c / 2 {
            return Habitat::default();
        }
        // Four columns, a quarter of the cell in from each corner.
        let mut sum = Column::default();
        let mut eco = 0u32;
        let mut realms = [0u8; 8];
        let mut dry = [0u8; 3];
        for (k, (dx, dz)) in [(64, 64), (192, 64), (64, 192), (192, 192)]
            .into_iter()
            .enumerate()
        {
            let x = planet.wrap_x((x0 + dx) as i32);
            let z = (z0 + dz) as i32;
            let col = self.column(x, z, (k as f32 + 0.5) / 4.0);
            sum.land += col.land;
            sum.fresh += col.fresh;
            sum.sea += col.sea;
            for (k, (a, b)) in sum.forage.iter_mut().zip(col.forage).enumerate() {
                *a += b * if k == Forage::Aquatic as usize {
                    col.fresh
                } else {
                    col.land
                };
            }
            sum.cover += col.cover * col.land;
            sum.temp_c += col.temp_c;
            sum.warm_c += col.warm_c;
            sum.precip_mm += col.precip_mm;
            eco |= col.ecosystems;
            realms[col.realm as usize] += 1;
            dry[(col.dry as usize).min(2)] += 1;
        }
        let land = sum.land / 4.0;
        let per_land = |v: f32| if sum.land > 0.0 { v / sum.land } else { 0.0 };
        let mut forage = sum.forage.map(per_land);
        forage[Forage::Aquatic as usize] = if sum.fresh > 0.0 {
            sum.forage[Forage::Aquatic as usize] / sum.fresh
        } else {
            0.0
        };
        let realm = (0..8).max_by_key(|&r| realms[r]).unwrap_or(0) as u8;
        let zc = (z0 + 128) as f64;
        Habitat {
            land,
            fresh: sum.fresh / 4.0,
            sea: sum.sea / 4.0,
            forage,
            cover: per_land(sum.cover),
            ecosystems: eco,
            realm,
            fauna: realm,
            island: self
                .wg
                .terrain
                .realms
                .island_at(planet.wrap_xf((x0 + 128) as f64), zc),
            temp_c: sum.temp_c / 4.0,
            warm_c: sum.warm_c / 4.0,
            precip_mm: sum.precip_mm / 4.0,
            southern: planet.latitude_deg(zc) < 0.0,
            dry: (0..3).max_by_key(|&d| dry[d]).unwrap_or(0) as u8,
        }
    }

    fn cells_around(&self) -> i64 {
        self.wg.planet().circumference() as i64 / CELL_M as i64
    }

    fn rows(&self) -> i64 {
        self.cells_around() / 2
    }
}

/// The same habitat everywhere (tests).
pub struct Uniform {
    pub habitat: Habitat,
    pub cells_around: i64,
}

impl Land for Uniform {
    fn habitat(&self, (_, j): (i64, i64)) -> Habitat {
        if j < -self.rows() || j >= self.rows() {
            return Habitat::default();
        }
        self.habitat
    }

    fn cells_around(&self) -> i64 {
        self.cells_around
    }

    fn rows(&self) -> i64 {
        self.cells_around / 2
    }
}

/// A temperate mixed wood with glades: the habitat the species' densities describe (tests,
/// calibration).
pub fn temperate_wood(catalog: &Catalog) -> Habitat {
    let n = miami_npp(9.0, 850.0) / 1200.0;
    let canopy = 0.65;
    let light = 1.0 - canopy;
    let edge = 4.0 * canopy * light;
    let young = 0.12;
    let wet = 850.0 / 1250.0;
    let mut forage = [0.0; FORAGE_KINDS];
    forage[Forage::Graze as usize] = GRAZE_OPEN * n * (light * light * 0.95 + 0.05);
    forage[Forage::Browse as usize] =
        n * (BROWSE_OLD * (0.3 + canopy) + BROWSE_YOUNG * young + 0.1 * GRAZE_OPEN * edge * 0.1);
    // Oak, beech and hazel share the canopy with trees that bear none.
    forage[Forage::Mast as usize] = canopy * 1.0e6 / CROWN_M2 * 0.5 * 5.0 * 0.5;
    forage[Forage::Fruit as usize] = FRUIT_EDGE * n * (edge * 0.8 + young * 0.6 + 0.1);
    forage[Forage::Seeds as usize] = 0.08 * forage[Forage::Graze as usize];
    forage[Forage::Invertebrates as usize] = INVERTEBRATES * n * wet * 1.6;
    forage[Forage::Fungi as usize] = FUNGI * n * canopy * wet * 1.6;
    forage[Forage::Nectar as usize] = NECTAR * n * (0.3 * light + edge * 0.3 + canopy * 0.5);
    forage[Forage::Aquatic as usize] = AQUATIC * n;
    let eco = ["temperate_broadleaf_forest", "temperate_freshwater"]
        .iter()
        .map(|id| {
            catalog
                .ecosystems
                .iter()
                .position(|e| e.id.ends_with(id))
                .map_or(0, |i| 1u32 << i)
        })
        .fold(0, |a, b| a | b);
    Habitat {
        land: 0.97,
        fresh: 0.03,
        sea: 0.0,
        forage,
        cover: canopy * 0.7 + young * 0.9 + light * 0.1,
        ecosystems: eco,
        realm: Realm::Palearctic as u8,
        fauna: Realm::Palearctic as u8,
        island: false,
        temp_c: 9.0,
        warm_c: 18.0,
        precip_mm: 850.0,
        southern: false,
        dry: dry_season::NONE,
    }
}
