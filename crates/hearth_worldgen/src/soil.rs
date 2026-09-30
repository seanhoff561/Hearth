//! Soils (v2 §5.3): the profile under every land column. The soil type comes from the content's
//! soil table by how well its formation fits the place — climate (Köppen), parent rock (the
//! geology's rock under the soil), vegetation (from the biome, and floodplains near rivers),
//! drainage and slope — and its horizons become blocks, thinned on steep ground. Beaches,
//! dunes, deserts, river and lake beds, salt flats, glaciers and bare rock have their own
//! surfaces.

use hearth_content::Content;
use hearth_content::schema::geology::Soil;
use hearth_math::hash::{derive_seed, hash_2d, unit_f32};
use hearth_world::{BlockRegistry, BlockStateId};
use smallvec::SmallVec;

use crate::cubegen::blocks::MissingBlock;
use crate::noise::BlockFbm;
use crate::region::biome::Biome;
use crate::region::{ColumnSample, Surface};

use hearth_content::schema::geology::SOIL_VEGETATION;

fn vegetation_bit(word: &str) -> Option<u16> {
    SOIL_VEGETATION
        .iter()
        .position(|w| *w == word)
        .map(|i| 1 << i)
}

/// Vegetation words of a place.
fn vegetation_of(biome: Biome) -> u16 {
    let words: &[&str] = match biome {
        Biome::TropicalRainforest => &["tropical_forest"],
        Biome::Savanna => &["savanna", "grassland"],
        Biome::HotDesert | Biome::DuneSea | Biome::Mesa | Biome::SaltFlat => &["desert"],
        Biome::Oasis => &["grassland"],
        Biome::Steppe => &["steppe", "grassland"],
        Biome::ColdDesert => &["desert", "steppe"],
        Biome::MediterraneanScrub => &["scrub"],
        Biome::TemperatePlains => &["grassland"],
        Biome::BroadleafForest | Biome::BirchForest => &["broadleaf_forest"],
        Biome::TemperateRainforest => &["temperate_rainforest", "broadleaf_forest"],
        Biome::MixedForest => &["mixed_forest"],
        Biome::BorealForest | Biome::SnowyTaiga => &["boreal_forest"],
        Biome::Tundra => &["tundra"],
        Biome::MontaneForest => &["montane_forest", "mixed_forest"],
        Biome::Krummholz => &["alpine", "boreal_forest"],
        Biome::AlpineMeadow => &["alpine", "grassland"],
        Biome::AlpineRock => &["alpine"],
        Biome::Wetland => &["wetland"],
        _ => &[],
    };
    words
        .iter()
        .filter_map(|w| vegetation_bit(w))
        .fold(0, |a, b| a | b)
}

#[derive(Debug, Clone)]
struct SoilModel {
    id: String,
    /// Horizons: block and thickness range (blocks), top down; thin horizons dropped.
    horizons: SmallVec<[(BlockStateId, f32, f32); 4]>,
    climates: Vec<String>,
    parents: Vec<BlockStateId>,
    vegetation: u16,
    drainage: Option<(f32, f32)>,
    max_slope: Option<f32>,
}

/// Blocks of a soil profile from the top down; `len()` is its depth in blocks.
pub type Profile = SmallVec<[BlockStateId; 12]>;

/// Soils and surface sediments of one world.
pub struct Soils {
    soils: Vec<SoilModel>,
    fallback: usize,
    turf: BlockStateId,
    podzol_turf: BlockStateId,
    moss: BlockStateId,
    snow: BlockStateId,
    glacier_ice: BlockStateId,
    quartz_sand: BlockStateId,
    black_sand: BlockStateId,
    shell_sand: BlockStateId,
    red_sand: BlockStateId,
    gravel: BlockStateId,
    clay: BlockStateId,
    mud: BlockStateId,
    salt: BlockStateId,
    stony: BlockStateId,
    /// Per state: rock of volcanic origin (black beaches) and carbonate (shell beaches).
    volcanic: Vec<bool>,
    carbonate: Vec<bool>,
    porosity: Vec<f32>,
    patches: BlockFbm,
    seed: u64,
}

/// A soil's id and its formation-based fit at a place (for maps and tests).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoilChoice {
    pub soil: usize,
    pub drainage: f32,
}

impl Soils {
    pub fn new(
        content: &Content,
        reg: &BlockRegistry,
        seed: u64,
        circumference: i64,
    ) -> Result<Self, MissingBlock> {
        let state = |id: &str| -> Result<BlockStateId, MissingBlock> {
            let path = id
                .split_once(':')
                .map_or(id, |(ns, p)| if ns == "hearth" { p } else { id });
            reg.parse_state(path)
                .map_err(|_| MissingBlock(id.to_owned()))
        };
        let mut soils = Vec::new();
        for s in content.soils.iter() {
            soils.push(Self::resolve(s, &state)?);
        }
        let fallback = soils
            .iter()
            .position(|s| {
                s.climates.is_empty()
                    && s.parents.is_empty()
                    && s.vegetation == 0
                    && s.drainage.is_none()
            })
            .ok_or_else(|| {
                MissingBlock("a soil without formation conditions (the fallback)".into())
            })?;
        let material_of = |st: BlockStateId| reg.block_of(st).def.material.clone();
        let mut volcanic = vec![false; reg.state_count()];
        let mut carbonate = vec![false; reg.state_count()];
        let mut porosity = vec![0.1f32; reg.state_count()];
        for rock in content.rocks.iter() {
            let Ok(st) = state(rock.id.as_str()) else {
                continue;
            };
            let block = reg.block_of(st);
            let volc = matches!(
                rock.class,
                hearth_content::schema::geology::RockClass::IgneousExtrusive
            );
            let m = material_of(st).and_then(|m| content.materials.get(&m).cloned());
            let carb = m
                .as_ref()
                .is_some_and(|m| m.tags.iter().any(|t| t == "lime_source"));
            let por = m.as_ref().and_then(|m| m.porosity).unwrap_or(0.05);
            for i in 0..block.state_count {
                let s = block.first_state.0 as usize + i as usize;
                volcanic[s] = volc;
                carbonate[s] = carb;
                porosity[s] = por;
            }
        }
        Ok(Self {
            soils,
            fallback,
            turf: state("grass_block[snowy=false]")?,
            podzol_turf: state("podzol[snowy=false]")?,
            moss: state("moss_block")?,
            snow: state("snow_block")?,
            glacier_ice: state("packed_ice")?,
            quartz_sand: state("quartz_sand")?,
            black_sand: state("black_sand")?,
            shell_sand: state("shell_sand")?,
            red_sand: state("red_sand")?,
            gravel: state("gravel")?,
            clay: state("earthenware_clay")?,
            mud: state("mud")?,
            salt: state("rock_salt")?,
            stony: state("stony_loam")?,
            volcanic,
            carbonate,
            porosity,
            patches: BlockFbm::new(
                derive_seed(seed, "soil patches"),
                circumference,
                180.0,
                2,
                0.5,
            ),
            seed: derive_seed(seed, "soils"),
        })
    }

    fn resolve(
        s: &Soil,
        state: &impl Fn(&str) -> Result<BlockStateId, MissingBlock>,
    ) -> Result<SoilModel, MissingBlock> {
        use hearth_content::schema::Entry;
        let mut horizons = SmallVec::new();
        for h in &s.horizons {
            // Horizons thinner than half a block are part of the turf.
            if h.thickness_m.1 < 0.5 {
                continue;
            }
            horizons.push((
                state(h.material.as_str())?,
                h.thickness_m.0.max(0.0),
                h.thickness_m.1,
            ));
        }
        if horizons.is_empty() {
            return Err(MissingBlock(format!(
                "soil {} has no horizon thicker than half a block",
                s.id()
            )));
        }
        let f = &s.formation;
        Ok(SoilModel {
            id: s.id().to_owned(),
            horizons,
            climates: f.climates.clone(),
            parents: f
                .parent_rocks
                .iter()
                .map(|r| state(r.as_str()))
                .collect::<Result<_, _>>()?,
            vegetation: f
                .vegetation
                .iter()
                .filter_map(|w| vegetation_bit(w))
                .fold(0, |a, b| a | b),
            drainage: f.drainage.map(|r| (r.0, r.1)),
            max_slope: f.max_slope_deg,
        })
    }

    pub fn soil_id(&self, i: usize) -> &str {
        &self.soils[i].id
    }

    pub fn soil_count(&self) -> usize {
        self.soils.len()
    }

    /// How well-drained the ground is, 0 (waterlogged) .. 1 (excessively drained).
    fn drainage(&self, s: &ColumnSample, parent: BlockStateId, x: i32, z: i32) -> f32 {
        if s.biome == Biome::Wetland {
            return 0.1;
        }
        let mut d = 0.5;
        d += 0.3 * (s.slope / 0.5).clamp(0.0, 1.0);
        d += 0.4 * self.porosity.get(parent.0 as usize).copied().unwrap_or(0.1);
        d -= 0.15 * ((s.precipitation - 1200.0) / 1500.0).clamp(0.0, 1.0);
        d += 0.15 * ((400.0 - s.precipitation) / 300.0).clamp(0.0, 1.0);
        if let Some(r) = s.river
            && r.distance < r.width * 2.5 + 6.0
            && s.height - r.level < 3.0
        {
            d -= 0.35;
        }
        // Occasional hollows on flat ground stay wet.
        if s.slope < 0.08 {
            d -= 0.3 * (self.patches.sample2(x as f64 + 3.7e4, z as f64) as f32 - 0.25).max(0.0);
        }
        d.clamp(0.0, 1.0)
    }

    fn floodplain(s: &ColumnSample) -> bool {
        s.river.is_some_and(|r| {
            r.distance < r.width * 4.0 + 12.0 && s.height - r.level < 4.0 && s.slope < 0.15
        })
    }

    /// Chooses the soil for a place.
    pub fn choose(&self, s: &ColumnSample, parent: BlockStateId, x: i32, z: i32) -> SoilChoice {
        let code = s.climate.code();
        let drainage = self.drainage(s, parent, x, z);
        let mut veg = vegetation_of(s.biome);
        if Self::floodplain(s) {
            veg |= vegetation_bit("floodplain").unwrap_or(0);
        }
        let slope_deg = s.slope.atan().to_degrees();
        let mut best = (f32::MIN, self.fallback);
        for (i, m) in self.soils.iter().enumerate() {
            let mut score = 0.0f32;
            // An exact climate counts for more than a broad group ("Cfb" over "C").
            if !m.climates.is_empty() {
                if m.climates.iter().any(|c| c == code) {
                    score += 3.0;
                } else if m
                    .climates
                    .iter()
                    .any(|c| code.starts_with(c.as_str()) || c.starts_with(code))
                {
                    score += 2.0;
                } else {
                    continue;
                }
            }
            // Soils defined by their parent rock (podzol, rendzina, terra rossa, andosol) win
            // where that rock is: they are what the rock makes of the climate.
            if !m.parents.is_empty() {
                if !m.parents.contains(&parent) {
                    continue;
                }
                score += 3.5;
            }
            if let Some((lo, hi)) = m.drainage {
                if drainage < lo || drainage > hi {
                    continue;
                }
                score += 1.0;
            }
            if m.max_slope.is_some_and(|max| slope_deg > max) {
                continue;
            }
            // Landforms (floodplain, wetland) define a soil more than the plants on it.
            if m.vegetation != 0 {
                let landforms = vegetation_bit("floodplain").unwrap_or(0)
                    | vegetation_bit("wetland").unwrap_or(0);
                score += if m.vegetation & veg & landforms != 0 {
                    4.0
                } else if m.vegetation & veg != 0 {
                    2.0
                } else {
                    -2.0
                };
            }
            // Patchy transitions between soils that fit equally well.
            score += 0.4 * unit_f32(hash_2d(self.seed ^ i as u64, x >> 5, z >> 5));
            if score > best.0 {
                best = (score, i);
            }
        }
        SoilChoice {
            soil: best.1,
            drainage,
        }
    }

    /// The profile of a land or water-bed column from the top down. `parent` is the rock
    /// under the soil (for choosing the soil and the colour of beach sand).
    pub fn profile(&self, s: &ColumnSample, parent: BlockStateId, x: i32, z: i32) -> Profile {
        let mut p = Profile::new();
        let depth = s.soil_depth as usize;
        let fill = |p: &mut Profile, b: BlockStateId, n: usize| p.extend(std::iter::repeat_n(b, n));
        match s.surface {
            Surface::Stone | Surface::Sandstone | Surface::RedSandstone | Surface::Tuff => {}
            Surface::Snow => fill(&mut p, self.snow, depth.max(1)),
            Surface::Ice => fill(&mut p, self.glacier_ice, depth.max(1)),
            Surface::Sand => {
                let pi = parent.0 as usize;
                let sand = if self.volcanic.get(pi).copied().unwrap_or(false) {
                    self.black_sand
                } else if self.carbonate.get(pi).copied().unwrap_or(false) && s.temperature > 18.0 {
                    self.shell_sand
                } else {
                    self.quartz_sand
                };
                fill(&mut p, sand, depth.max(1));
            }
            Surface::RedSand => fill(&mut p, self.red_sand, depth.max(1)),
            Surface::Gravel => fill(&mut p, self.gravel, depth.max(1)),
            Surface::Clay => fill(&mut p, self.clay, depth.max(1)),
            Surface::Dirt => fill(&mut p, self.mud, depth.max(1)),
            Surface::Calcite => {
                p.push(self.salt);
                fill(&mut p, self.mud, depth.max(1));
            }
            Surface::Grass
            | Surface::SnowGrass
            | Surface::Podzol
            | Surface::Moss
            | Surface::CoarseDirt
            | Surface::Mud => {
                // Soil thickness follows the ground: full on flats and in hollows, thin on
                // slopes, a few stony centimetres on steep ground.
                let patch = self.patches.sample2(x as f64, z as f64) as f32;
                let factor = (1.15 - s.slope * 1.3).clamp(0.0, 1.2) * (0.9 + 0.35 * patch);
                if factor < 0.15 || depth == 0 {
                    // Treeless steep slopes are scree; wooded ones keep a stony soil.
                    let scree = matches!(
                        s.biome,
                        Biome::AlpineMeadow
                            | Biome::AlpineRock
                            | Biome::Krummholz
                            | Biome::Tundra
                            | Biome::ColdDesert
                            | Biome::HotDesert
                            | Biome::Mesa
                            | Biome::Steppe
                    );
                    p.push(if scree { self.gravel } else { self.stony });
                    return p;
                }
                let choice = self.choose(s, parent, x, z);
                let m = &self.soils[choice.soil];
                let t = unit_f32(hash_2d(self.seed ^ 0x7a, x >> 3, z >> 3));
                for (k, &(block, lo, hi)) in m.horizons.iter().enumerate() {
                    let n = ((lo + (hi - lo) * t) * factor).round() as usize;
                    fill(&mut p, block, if k == 0 { n.max(1) } else { n });
                }
                p.truncate(10);
                match s.surface {
                    Surface::Grass | Surface::SnowGrass => p[0] = self.turf,
                    Surface::Podzol => p[0] = self.podzol_turf,
                    Surface::Moss => p[0] = self.moss,
                    _ => {}
                }
            }
        }
        p
    }

    /// Blocks crops, grass and trees can root in.
    pub fn plantable(&self) -> impl Iterator<Item = BlockStateId> + '_ {
        let mut v: Vec<BlockStateId> = vec![
            self.turf,
            self.podzol_turf,
            self.moss,
            self.quartz_sand,
            self.black_sand,
            self.shell_sand,
            self.red_sand,
            self.gravel,
            self.clay,
            self.mud,
            self.stony,
        ];
        for m in &self.soils {
            v.extend(m.horizons.iter().map(|h| h.0));
        }
        v.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    use hearth_math::PlanetSize;

    use super::*;
    use crate::planet::PlanetGrid;
    use crate::planet::climate::ClimateClass;
    use crate::region::Terrain;
    use crate::region::rivers::RiverHit;
    use crate::settings::WorldGenSettings;

    struct Fixture {
        reg: BlockRegistry,
        soils: Soils,
        base: ColumnSample,
    }

    fn fixture() -> &'static Fixture {
        static F: OnceLock<Fixture> = OnceLock::new();
        F.get_or_init(|| {
            let s = WorldGenSettings {
                seed: 4,
                planet_size: PlanetSize::Tiny,
                grid_resolution: 128,
                ..WorldGenSettings::default()
            };
            let terrain = Terrain::new(Arc::new(PlanetGrid::build(&s, &|_, _| {})));
            let (x, z) = terrain.find_spawn(false);
            let content = Content::load_base();
            let reg = hearth_world::datapack::load_builtin_registry().expect("registry");
            let soils = Soils::new(&content, &reg, 4, 16_384).expect("soils");
            Fixture {
                reg,
                soils,
                base: terrain.sample(x, z),
            }
        })
    }

    fn place(
        biome: Biome,
        climate: ClimateClass,
        surface: Surface,
        slope: f32,
        precip: f32,
    ) -> ColumnSample {
        let mut s = fixture().base;
        s.biome = biome;
        s.climate = climate;
        s.surface = surface;
        s.slope = slope;
        s.precipitation = precip;
        s.river = None;
        s.soil_depth = 3;
        s.temperature = 10.0;
        s
    }

    fn rock(name: &str) -> BlockStateId {
        fixture().reg.parse_state(name).expect(name)
    }

    fn soil_of(s: &ColumnSample, parent: &str) -> String {
        let f = fixture();
        f.soils
            .soil_id(f.soils.choose(s, rock(parent), 100, 100).soil)
            .to_owned()
    }

    fn names(p: &Profile) -> Vec<String> {
        p.iter()
            .map(|b| fixture().reg.block_of(*b).name.path().to_owned())
            .collect()
    }

    #[test]
    fn soils_form_where_they_should() {
        let boreal = place(
            Biome::BorealForest,
            ClimateClass::Subarctic,
            Surface::Podzol,
            0.12,
            600.0,
        );
        assert_eq!(soil_of(&boreal, "granite"), "hearth:podzol");
        let steppe = place(
            Biome::Steppe,
            ClimateClass::ColdSteppe,
            Surface::Grass,
            0.1,
            400.0,
        );
        assert_eq!(soil_of(&steppe, "shale"), "hearth:chernozem");
        let med = place(
            Biome::MediterraneanScrub,
            ClimateClass::Mediterranean,
            Surface::Grass,
            0.15,
            600.0,
        );
        assert_eq!(soil_of(&med, "limestone"), "hearth:terra_rossa");
        let bog = place(
            Biome::Wetland,
            ClimateClass::Oceanic,
            Surface::Mud,
            0.02,
            1100.0,
        );
        assert_eq!(soil_of(&bog, "shale"), "hearth:peat_bog");
        let tropics = place(
            Biome::TropicalRainforest,
            ClimateClass::TropicalRainforest,
            Surface::Grass,
            0.15,
            2500.0,
        );
        assert_eq!(soil_of(&tropics, "gneiss"), "hearth:red_tropical_soil");
        let volcanic = place(
            Biome::BroadleafForest,
            ClimateClass::Oceanic,
            Surface::Grass,
            0.15,
            1000.0,
        );
        assert_eq!(soil_of(&volcanic, "andesite"), "hearth:andosol");
        let mut river = place(
            Biome::TemperatePlains,
            ClimateClass::Oceanic,
            Surface::Grass,
            0.03,
            800.0,
        );
        river.river = Some(RiverHit {
            distance: 12.0,
            level: river.height - 1.0,
            width: 10.0,
            depth: 2.0,
        });
        assert_eq!(soil_of(&river, "sandstone"), "hearth:alluvial_soil");
    }

    #[test]
    fn profiles_are_layered_and_thin_on_slopes() {
        let f = fixture();
        let forest = place(
            Biome::BroadleafForest,
            ClimateClass::Oceanic,
            Surface::Grass,
            0.1,
            900.0,
        );
        let p = f.soils.profile(&forest, rock("sandstone"), 40, 40);
        let n = names(&p);
        assert_eq!(n[0], "grass_block", "turf on top");
        assert!(
            n.len() >= 2 && n[1..].iter().all(|b| b == "loam" || b == "clay_loam"),
            "{n:?}"
        );
        let steep = place(
            Biome::BroadleafForest,
            ClimateClass::Oceanic,
            Surface::Grass,
            0.95,
            900.0,
        );
        let p = f.soils.profile(&steep, rock("sandstone"), 40, 40);
        assert_eq!(
            names(&p),
            vec!["stony_loam".to_string()],
            "steep ground keeps only stony soil"
        );
        let cliff = place(
            Biome::AlpineRock,
            ClimateClass::Tundra,
            Surface::Stone,
            1.5,
            900.0,
        );
        assert!(
            f.soils.profile(&cliff, rock("granite"), 1, 1).is_empty(),
            "bare rock"
        );
    }

    #[test]
    fn beach_sand_comes_from_the_rocks() {
        let f = fixture();
        let beach = place(
            Biome::Beach,
            ClimateClass::Oceanic,
            Surface::Sand,
            0.02,
            900.0,
        );
        assert_eq!(
            names(&f.soils.profile(&beach, rock("basalt"), 1, 1))[0],
            "black_sand"
        );
        assert_eq!(
            names(&f.soils.profile(&beach, rock("granite"), 1, 1))[0],
            "quartz_sand"
        );
        let mut warm = beach;
        warm.temperature = 25.0;
        assert_eq!(
            names(&f.soils.profile(&warm, rock("limestone"), 1, 1))[0],
            "shell_sand"
        );
    }
}
