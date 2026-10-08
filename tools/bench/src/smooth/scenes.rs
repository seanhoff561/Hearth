//! S0's eight test scenes (S §3.1): analytic ground on a 96 × 80 × 96 m box, each a signed
//! distance (metres, positive inside the ground) and a material for every point inside. Grid
//! positions are voxel centres, so a scene's coordinates are the meshes' coordinates.

use glam::DVec3;
use hearth_math::hash::{hash2, unit_f64};
use hearth_worldgen::noise::BlockFbm;

/// The box the scenes fill: x and z 0..96, y 0..80.
pub const SIZE: [usize; 3] = [96, 80, 96];

/// No planet seam here: the noise's period is far beyond the box.
const NO_WRAP: i64 = 1 << 24;

pub const GRASS: u16 = 1;
pub const DIRT: u16 = 2;
pub const LIMESTONE: u16 = 3;
pub const SAND: u16 = 5;
pub const MUD: u16 = 6;
pub const GRAVEL: u16 = 7;
pub const SNOW: u16 = 8;
pub const CLAY: u16 = 9;
pub const GRANITE: u16 = 10;

/// A material's look and how crisp it is.
pub struct Material {
    pub name: &'static str,
    /// sRGB albedo.
    pub color: [u8; 3],
    /// 0 soft … 1 crisp (`docs/design/art-direction.md`'s families).
    pub sharpness: f32,
}

pub const MATERIALS: [Material; 11] = [
    Material {
        name: "air",
        color: [0, 0, 0],
        sharpness: 0.0,
    },
    Material {
        name: "soil, grass",
        color: [84, 116, 50],
        sharpness: 0.2,
    },
    Material {
        name: "dirt",
        color: [112, 86, 60],
        sharpness: 0.25,
    },
    Material {
        name: "limestone",
        color: [184, 178, 162],
        sharpness: 0.85,
    },
    Material {
        name: "basalt",
        color: [72, 70, 68],
        sharpness: 0.95,
    },
    Material {
        name: "sand",
        color: [212, 192, 146],
        sharpness: 0.08,
    },
    Material {
        name: "mud",
        color: [94, 78, 60],
        sharpness: 0.3,
    },
    Material {
        name: "gravel",
        color: [140, 134, 124],
        sharpness: 0.45,
    },
    Material {
        name: "snow",
        color: [230, 234, 240],
        sharpness: 0.1,
    },
    Material {
        name: "clay",
        color: [152, 106, 72],
        sharpness: 0.25,
    },
    Material {
        name: "granite",
        color: [148, 140, 134],
        sharpness: 0.9,
    },
];

pub fn sharpness(m: u16) -> f32 {
    MATERIALS.get(m as usize).map_or(0.5, |x| x.sharpness)
}

/// A test scene.
pub struct Scene {
    pub key: &'static str,
    pub title: &'static str,
    /// What the scene tests.
    pub what: &'static str,
    pub eye: DVec3,
    pub target: DVec3,
    /// A close look at what the scene tests.
    pub close: (DVec3, DVec3),
    /// Toward the sun.
    pub sun: DVec3,
    /// A still water surface, if any.
    pub water: Option<f64>,
    /// The raw signed density (positive inside); [`Scene::distance`] normalizes it.
    density: Box<dyn Fn(DVec3) -> f64 + Sync + Send>,
    /// The material at a point inside, given its depth below the surface and the surface's
    /// outward normal there.
    material: Box<dyn Fn(DVec3, f64, DVec3) -> u16 + Sync + Send>,
}

impl Scene {
    /// The signed distance at a point (metres, positive inside, to first order) and the
    /// surface's outward normal: the density over its gradient's length.
    pub fn distance(&self, p: DVec3) -> (f64, DVec3) {
        let r = (self.density)(p);
        let h = 0.25;
        let g = DVec3::new(
            (self.density)(p + DVec3::X * h) - (self.density)(p - DVec3::X * h),
            (self.density)(p + DVec3::Y * h) - (self.density)(p - DVec3::Y * h),
            (self.density)(p + DVec3::Z * h) - (self.density)(p - DVec3::Z * h),
        ) / (2.0 * h);
        let len = g.length().max(0.2);
        (r / len, -g / len)
    }

    /// A sample's distance and material: far from the surface the density alone says which side
    /// (the fill saturates beyond 1.5 m anyway).
    pub fn sample(&self, p: DVec3) -> (f64, u16) {
        let r = (self.density)(p);
        if r > 8.0 {
            return (r, (self.material)(p, r, DVec3::Y));
        }
        if r < -8.0 {
            return (r, 0);
        }
        let (d, n) = self.distance(p);
        let m = if d > 0.0 { (self.material)(p, d, n) } else { 0 };
        (d, m)
    }
}

fn fbm(seed: u64, wavelength: f64, octaves: u32) -> BlockFbm {
    BlockFbm::new(seed, NO_WRAP, wavelength, octaves, 0.5)
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Polynomial smooth maximum (blend width `k`).
fn smax(a: f64, b: f64, k: f64) -> f64 {
    let h = (0.5 + 0.5 * (a - b) / k).clamp(0.0, 1.0);
    b + (a - b) * h + k * h * (1.0 - h)
}

/// Soil over rock by depth and slope: turf on the top 0.8 m of gentle ground, dirt to 3.5 m,
/// rock beneath; rock at the surface where it is too steep for soil to hold.
fn soil_over(rock: u16, depth: f64, n: DVec3) -> u16 {
    if n.y < 0.62 {
        rock
    } else if depth < 0.8 {
        GRASS
    } else if depth < 3.5 {
        DIRT
    } else {
        rock
    }
}

pub fn all() -> Vec<Scene> {
    vec![
        rolling_hills(),
        sea_cliffs(),
        cave(),
        dune_field(),
        riverbank(),
        talus_slope(),
        dug_pit(),
        mountain_ridge(),
    ]
}

fn rolling_hills() -> Scene {
    let base = fbm(11, 70.0, 5);
    let detail = fbm(12, 13.0, 3);
    let scarps = fbm(13, 44.0, 3);
    let height = move |x: f64, z: f64| {
        let s = scarps.ridged2(x, z);
        30.0 + 9.0 * base.sample2(x, z)
            + 1.0 * detail.sample2(x, z)
            + 2.4 * smoothstep(0.56, 0.62, s)
    };
    Scene {
        key: "rolling_hills",
        title: "Rolling hills",
        what: "gentle grassland with small limestone scarps: soft ground soft, rock steps crisp",
        eye: DVec3::new(6.0, 45.0, 10.0),
        target: DVec3::new(56.0, 29.0, 60.0),
        close: (DVec3::new(22.0, 38.0, 26.0), DVec3::new(34.0, 31.0, 40.0)),
        sun: DVec3::new(-0.45, 0.55, 0.7),
        water: None,
        density: Box::new(move |p| height(p.x, p.z) - p.y),
        material: Box::new(|_, d, n| soil_over(LIMESTONE, d, n)),
    }
}

fn sea_cliffs() -> Scene {
    let wobble = fbm(21, 24.0, 3);
    let top = fbm(22, 30.0, 3);
    let coast = move |z: f64| 54.0 + 7.0 * (z / 15.0 + 0.3).sin() + 3.0 * wobble.sample2(z, 0.5);
    // Shale bands, softer than the limestone between them, weathered back under ledges.
    const BANDS: [(f64, f64); 4] = [(24.6, 25.8), (29.1, 29.9), (33.4, 35.0), (41.0, 41.8)];
    let boulders: Vec<(DVec3, f64)> = (0..9)
        .map(|i| {
            let h = hash2(2100, i);
            let z = 6.0 + 84.0 * unit_f64(h);
            let off = 1.2 + 3.0 * unit_f64(hash2(h, 1));
            let r = 0.9 + 1.0 * unit_f64(hash2(h, 2));
            (DVec3::new(coast(z) + off, 21.8, z), r)
        })
        .collect();
    let block = {
        let coast = coast.clone();
        move |p: DVec3| {
            // The face leans back a little; the bands are cut 0.9 m into it.
            let line = coast(p.z) + 0.04 * (p.y - 20.0);
            let plateau = 52.0 + 1.4 * top.sample2(p.x, p.z) - p.y;
            let mut d = (line - p.x).min(plateau);
            for &(lo, hi) in &BANDS {
                let band = (p.x - line + 0.9).min(p.y - lo).min(hi - p.y);
                d = d.min(-band);
            }
            d
        }
    };
    let beach = move |p: DVec3| {
        let h = (22.4 - 0.2 * (p.x - coast(p.z))).max(13.0);
        h - p.y
    };
    let rocks = boulders.clone();
    let boulder = move |p: DVec3| {
        rocks
            .iter()
            .map(|&(c, r)| r - ((p - c) * DVec3::new(1.0, 1.4, 1.0)).length())
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let (block2, boulder2) = (block.clone(), boulder.clone());
    Scene {
        key: "sea_cliffs",
        title: "Sea cliffs",
        what: "a limestone cliff with recessed shale bands over a sand beach: crisp ledges and \
               edges, soft sand, boulders",
        eye: DVec3::new(92.0, 34.0, 16.0),
        target: DVec3::new(52.0, 34.0, 58.0),
        close: (DVec3::new(72.0, 27.0, 28.0), DVec3::new(55.0, 37.0, 44.0)),
        sun: DVec3::new(0.75, 0.5, -0.3),
        water: Some(20.4),
        density: Box::new(move |p| block(p).max(beach(p)).max(boulder(p))),
        material: Box::new(move |p, d, n| {
            if boulder2(p) > -0.3 {
                LIMESTONE
            } else if block2(p) > -0.3 && p.y > 21.0 {
                // The soil thins to nothing toward the edge, leaving the rock bare.
                if n.y > 0.62 && d < 0.8 && block2(p + DVec3::new(2.5, 0.0, 0.0)) > 0.0 {
                    GRASS
                } else {
                    LIMESTONE
                }
            } else if d < 2.0 {
                SAND
            } else {
                GRAVEL
            }
        }),
    }
}

fn cave() -> Scene {
    let lumps = fbm(31, 20.0, 3);
    let rough = fbm(32, 4.0, 2);
    let curve = |t: f64| {
        DVec3::new(
            12.0 + 52.0 * t,
            33.4 + 2.2 * (5.0 * t).sin() + 2.0 * t,
            34.0 + 12.0 * (2.2 * t).sin(),
        )
    };
    let points: Vec<DVec3> = (0..=48).map(|i| curve(i as f64 / 48.0)).collect();
    let chamber = curve(1.0) + DVec3::new(4.0, 1.0, 0.0);
    let void = move |p: DVec3| {
        // Distance to the tunnel's axis, its radius swelling and narrowing along it.
        let mut best = f64::INFINITY;
        let mut at = 0.0;
        for (i, w) in points.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            let dist = (p - (a + ab * t)).length();
            if dist < best {
                best = dist;
                at = (i as f64 + t) / 48.0;
            }
        }
        let tube = 3.3 + 0.7 * (9.0 * at).sin() - best;
        let room = 8.0 - ((p - chamber) * DVec3::new(1.0, 1.5, 1.0)).length();
        smax(tube, room, 2.0) + 0.45 * rough.sample3(p.x, p.y, p.z)
    };
    let hill = move |p: DVec3| {
        let r2 = (p.x - 58.0).powi(2) + (p.z - 56.0).powi(2);
        34.5 + 17.0 * (-r2 / 1100.0).exp() + 1.4 * lumps.sample2(p.x, p.z) - p.y
    };
    let void2 = void.clone();
    Scene {
        key: "cave",
        title: "Cave",
        what: "a winding limestone tunnel into a hill, opening into a chamber: overhangs, a \
               gravel floor, the mouth where turf meets rock",
        eye: DVec3::new(2.0, 38.0, 24.0),
        target: DVec3::new(22.0, 34.0, 38.0),
        close: (DVec3::new(14.0, 34.6, 35.0), DVec3::new(26.0, 35.0, 40.5)),
        sun: DVec3::new(-0.6, 0.6, -0.5),
        water: None,
        density: Box::new(move |p| hill(p).min(-void(p))),
        material: Box::new(move |p, d, n| {
            let v = void2(p);
            if v > -2.2 {
                if n.y > 0.55 && v > -1.2 {
                    GRAVEL
                } else {
                    LIMESTONE
                }
            } else {
                soil_over(LIMESTONE, d, n)
            }
        }),
    }
}

fn dune_field() -> Scene {
    let sway = fbm(41, 40.0, 3);
    let ground = fbm(42, 26.0, 3);
    let height = move |x: f64, z: f64| {
        let wavelength = 40.0;
        let shifted = x + 7.0 * (z / 19.0).sin() + 5.0 * sway.sample2(z, 3.5);
        let u = (shifted / wavelength).rem_euclid(1.0);
        let a = 5.5 + 1.6 * (z / 27.0 + 1.0).sin();
        // Gentle windward rise to the brink, then the slip face at the angle of repose.
        let slip = (a / 0.65 / wavelength).min(0.4);
        let windward = 1.0 - slip;
        let p = if u < windward {
            a * (std::f64::consts::FRAC_PI_2 * u / windward).sin()
        } else {
            a * (1.0 - (u - windward) / slip)
        };
        28.0 + p + 0.7 * ground.sample2(x, z)
    };
    Scene {
        key: "dune_field",
        title: "Dune field",
        what: "transverse dunes: long windward slopes, slip faces at 33°, brinks that should stay \
               soft",
        eye: DVec3::new(92.0, 42.0, 8.0),
        target: DVec3::new(44.0, 30.0, 56.0),
        close: (DVec3::new(62.0, 37.0, 38.0), DVec3::new(48.0, 31.0, 52.0)),
        sun: DVec3::new(-0.7, 0.42, 0.3),
        water: None,
        density: Box::new(move |p| height(p.x, p.z) - p.y),
        material: Box::new(|_, _, _| SAND),
    }
}

fn riverbank() -> Scene {
    let plain = fbm(51, 30.0, 3);
    // The channel's shape across a section: q −1 at the inner edge, +1 at the cut bank.
    let section = move |x: f64, z: f64| {
        let centre = 46.0 + 16.0 * (z / 21.0).sin();
        let slope = 16.0 / 21.0 * (z / 21.0).cos();
        let s = (x - centre) / (1.0 + slope * slope).sqrt();
        let bend = (z / 21.0).sin();
        let q = s * bend.signum() / 7.0;
        (q, bend.abs())
    };
    let depth = move |x: f64, z: f64| {
        let (q, asym) = section(x, z);
        let deepest = 0.5 * asym;
        let d = if q < deepest {
            smoothstep(-1.7, deepest, q)
        } else {
            1.0 - smoothstep(0.98 - 0.45 * (1.0 - asym) - 0.22, 0.98, q)
        };
        4.6 * d
    };
    let height = move |x: f64, z: f64| 32.4 + 0.5 * plain.sample2(x, z) - depth(x, z);
    Scene {
        key: "riverbank",
        title: "Riverbank",
        what: "a meandering channel in a floodplain: a steep mud cut bank on the outside of each \
               bend, a sand and gravel point bar inside",
        eye: DVec3::new(30.0, 41.0, 6.0),
        target: DVec3::new(56.0, 29.0, 38.0),
        close: (DVec3::new(54.0, 33.5, 25.0), DVec3::new(68.0, 29.5, 34.0)),
        sun: DVec3::new(-0.3, 0.6, -0.75),
        water: Some(30.1),
        density: Box::new(move |p| height(p.x, p.z) - p.y),
        material: Box::new(move |p, d, n| {
            let dep = depth(p.x, p.z);
            if dep > 0.3 && d < 2.0 {
                let (q, asym) = section(p.x, p.z);
                if n.y < 0.6 {
                    MUD
                } else if q < 0.5 * asym && dep < 3.4 {
                    SAND
                } else {
                    GRAVEL
                }
            } else if d < 0.6 && n.y > 0.6 {
                GRASS
            } else if d < 3.0 {
                DIRT
            } else {
                CLAY
            }
        }),
    }
}

fn talus_slope() -> Scene {
    let ground = fbm(61, 28.0, 3);
    let face = fbm(62, 16.0, 3);
    let lumps = fbm(63, 2.2, 2);
    let cliff_z = move |x: f64| {
        62.0 + 3.0 * (x / 11.0).sin()
            + 1.5 * face.sample2(x, 7.5)
            + 1.6 * (1.0 - (x / 7.0).sin().abs()).powi(2)
    };
    let cones: Vec<(f64, f64, f64)> = [17.0, 39.0, 61.0, 83.0]
        .iter()
        .enumerate()
        .map(|(i, &x)| (x, cliff_z(x) - 0.5, 51.0 + 3.0 * (i % 2) as f64))
        .collect();
    let tan_repose = 37f64.to_radians().tan();
    let talus = move |x: f64, z: f64| {
        cones
            .iter()
            .map(|&(cx, cz, apex)| apex - tan_repose * ((x - cx).powi(2) + (z - cz).powi(2)).sqrt())
            .fold(f64::NEG_INFINITY, f64::max)
            + 0.35 * lumps.sample2(x, z)
    };
    let boulders: Vec<(DVec3, f64)> = (0..12)
        .map(|i| {
            let h = hash2(6100, i);
            let x = 6.0 + 84.0 * unit_f64(h);
            let z = 38.0 + 10.0 * unit_f64(hash2(h, 1));
            let r = 0.8 + 1.0 * unit_f64(hash2(h, 2));
            (DVec3::new(x, 30.0 + r * 0.4, z), r)
        })
        .collect();
    let boulder = move |p: DVec3| {
        boulders
            .iter()
            .map(|&(c, r)| r - ((p - c) * DVec3::new(1.0, 1.3, 1.0)).length())
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let base = move |p: DVec3| 30.0 + 0.9 * ground.sample2(p.x, p.z) - p.y;
    let cliff = move |p: DVec3| (p.z - cliff_z(p.x)).min(66.0 - p.y);
    let (talus2, boulder2, cliff2, base2) =
        (talus.clone(), boulder.clone(), cliff.clone(), base.clone());
    Scene {
        key: "talus_slope",
        title: "Talus slope",
        what: "scree cones at their angle of repose (37°) under a limestone cliff with chutes, \
               boulders at the toe",
        eye: DVec3::new(72.0, 46.0, 10.0),
        target: DVec3::new(46.0, 44.0, 58.0),
        close: (DVec3::new(47.0, 40.0, 36.0), DVec3::new(39.0, 45.0, 58.0)),
        sun: DVec3::new(-0.55, 0.55, -0.6),
        water: None,
        density: Box::new(move |p| {
            base(p)
                .max(cliff(p))
                .max(talus(p.x, p.z) - p.y)
                .max(boulder(p))
        }),
        material: Box::new(move |p, d, n| {
            if boulder2(p) > -0.25 || cliff2(p) > -0.4 {
                LIMESTONE
            } else if talus2(p.x, p.z) - p.y > -0.4 && talus2(p.x, p.z) > base2(p) + p.y + 0.2 {
                GRAVEL
            } else {
                soil_over(LIMESTONE, d, n)
            }
        }),
    }
}

fn dug_pit() -> Scene {
    let ground = fbm(71, 20.0, 3);
    let lumps = fbm(72, 1.1, 2);
    let level = move |x: f64, z: f64| 30.35 + 0.15 * ground.sample2(x, z);
    let (cx, cz) = (44.3, 46.7);
    let pit = move |p: DVec3, top: f64| {
        // The pit's open volume: inside its walls and above its floor.
        (2.6 - (p.x - cx).abs())
            .min(2.1 - (p.z - cz).abs())
            .min(p.y - (top - 2.2))
    };
    let spine = (DVec3::new(51.6, 0.0, 47.0), DVec3::new(55.2, 0.0, 47.0));
    let tan_repose = 34f64.to_radians().tan();
    let pile = move |x: f64, z: f64| {
        let p = DVec3::new(x, 0.0, z);
        let ab = spine.1 - spine.0;
        let t = ((p - spine.0).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        let dist = (p - (spine.0 + ab * t)).length();
        (2.4 - tan_repose * dist).max(0.0)
    };
    let lumps2 = fbm(72, 1.1, 2);
    let level2 = level.clone();
    Scene {
        key: "dug_pit",
        title: "Dug pit and spoil pile",
        what: "a fresh pit with cut walls through turf, soil and clay; its spoil heaped beside \
               it at the angle of repose (34°)",
        eye: DVec3::new(36.5, 35.0, 37.0),
        target: DVec3::new(48.0, 29.6, 47.5),
        close: (DVec3::new(39.0, 34.5, 40.0), DVec3::new(45.0, 29.6, 47.0)),
        sun: DVec3::new(-0.5, 0.62, -0.6),
        water: None,
        density: Box::new(move |p| {
            let top = level(p.x, p.z);
            let ground = (top - p.y).min(-pit(p, top));
            let heap = pile(p.x, p.z);
            let heap = if heap > 0.0 {
                top + heap + 0.12 * lumps.sample3(p.x, p.y, p.z) - p.y
            } else {
                f64::NEG_INFINITY
            };
            ground.max(heap)
        }),
        material: Box::new(move |p, _, _| {
            let top = level2(p.x, p.z);
            if p.y > top + 0.05 && pile(p.x, p.z) > 0.0 {
                // Spoil: the last-dug clay lies over the soil dug first, in lumps.
                if p.y > top + 1.1 || lumps2.sample3(p.x * 2.0, p.y * 2.0, p.z * 2.0) > 0.25 {
                    CLAY
                } else {
                    DIRT
                }
            } else {
                let below = top - p.y;
                if below < 0.45 {
                    GRASS
                } else if below < 1.6 {
                    DIRT
                } else {
                    CLAY
                }
            }
        }),
    }
}

fn mountain_ridge() -> Scene {
    let line = fbm(81, 40.0, 3);
    let crest = fbm(82, 30.0, 4);
    let ribs = fbm(83, 11.0, 3);
    let valley = fbm(84, 30.0, 3);
    let height = move |x: f64, z: f64| {
        let ridge_z = 46.0 + 6.0 * (x / 23.0).sin() + 2.5 * line.sample2(x, 1.5);
        let top = 60.0 + 6.0 * (x / 17.0 + 0.5).sin() + 3.0 * crest.ridged2(x, 3.5);
        let dz = z - ridge_z;
        let k = if dz > 0.0 { 1.55 } else { 1.1 };
        let faces = top - k * dz.abs() + 2.0 * ribs.ridged2(x, z) * smoothstep(1.0, 6.0, dz.abs());
        let floor = 30.0 + 1.5 * valley.sample2(x, z);
        smax(faces, floor, 6.0)
    };
    Scene {
        key: "mountain_ridge",
        title: "Mountain ridge",
        what: "a granite arête with rock ribs and snow on its gentler upper slopes, scree below, \
               a grassy valley",
        eye: DVec3::new(4.0, 60.0, 6.0),
        target: DVec3::new(54.0, 50.0, 50.0),
        close: (DVec3::new(38.0, 63.0, 34.0), DVec3::new(52.0, 57.0, 51.0)),
        sun: DVec3::new(-0.35, 0.55, -0.75),
        water: None,
        density: Box::new(move |p| height(p.x, p.z) - p.y),
        material: Box::new(|p, d, n| {
            if p.y > 47.0 && n.y > 0.68 && d < 0.9 {
                SNOW
            } else if n.y < 0.72 || p.y > 44.0 {
                GRANITE
            } else if p.y < 41.0 && n.y < 0.86 && d < 1.5 {
                GRAVEL
            } else {
                soil_over(GRANITE, d, n)
            }
        }),
    }
}
