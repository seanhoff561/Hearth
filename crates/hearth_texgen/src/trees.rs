//! Textures of the trees (V2-6): each species' bark by its pattern and colours (the logs and
//! the limbs of its wood), its log ends with their rings, and its foliage, broad or needles,
//! drawn grey for the season's tint.

use hearth_content::Content;
use hearth_content::schema::flora::{BarkPattern, LeafKind};

use crate::TexEntry;
use crate::paint::{Rgb, Tex, fbm, lerp, paint, rand01, scale, value_noise, voronoi};

const S: u32 = 16;

fn seed(name: &str) -> u64 {
    hearth_math::hash::derive_seed(0x7eee_5000, name)
}

/// Bark by its pattern.
pub fn bark(pattern: BarkPattern, base: Rgb, second: Option<Rgb>, seed: u64) -> Tex {
    let dark = scale(base, 0.62);
    let light = scale(base, 1.18);
    match pattern {
        BarkPattern::Furrowed => paint(S, S, |x, y| {
            // Deep vertical furrows between broad ridges that wander.
            let wander = value_noise(seed, x as f32, y as f32 * 0.35, 4.0, 16.0) * 3.0;
            let f = ((x as f32 + wander) * 0.5).sin().abs();
            let n = fbm(seed ^ 3, x, y, S, 0.35);
            if f < 0.28 {
                scale(dark, 0.8 + n * 0.3)
            } else {
                lerp(base, light, n * 0.6 * f)
            }
        }),
        BarkPattern::Smooth => paint(S, S, |x, y| {
            // Even grey, softly mottled, with faint flutes.
            let n = fbm(seed, x, y, S, 0.15);
            let flute = (x as f32 * 0.8 + value_noise(seed ^ 9, 0.0, y as f32, 8.0, 16.0)).sin();
            let c = lerp(scale(base, 0.92), scale(base, 1.08), n);
            scale(c, 1.0 + 0.04 * flute)
        }),
        BarkPattern::Peeling => {
            // Pale papery bark with dark horizontal marks.
            let mark = second.unwrap_or([40, 38, 34]);
            paint(S, S, |x, y| {
                let n = fbm(seed, x, y, S, 0.2);
                let band = value_noise(seed ^ 5, x as f32 * 0.6, y as f32, 4.0, 16.0);
                let streak = rand01(seed ^ 7, x / 4, y) > 0.82 && band > 0.45;
                if streak {
                    lerp(mark, base, 0.15)
                } else {
                    lerp(scale(base, 0.9), base, n)
                }
            })
        }
        BarkPattern::Scaly => {
            // Plates with dark cracks between them; a second colour on some plates.
            let tint = second.unwrap_or(light);
            paint(S, S, |x, y| {
                let (d1, d2, id) = voronoi(seed, x as f32 + 0.5, y as f32 * 0.6 + 0.5, 16.0, 9);
                if d2 - d1 < 0.9 {
                    dark
                } else {
                    let t = ((id >> 24) % 1000) as f32 / 1000.0;
                    let plate = if second.is_some() && t > 0.55 {
                        tint
                    } else {
                        base
                    };
                    scale(plate, 0.88 + 0.24 * t)
                }
            })
        }
        BarkPattern::Fissured => paint(S, S, |x, y| {
            // Long shallow fissures meeting in narrow diamonds.
            let a = ((x as f32 + y as f32 * 0.35) * 0.9).sin();
            let b = ((x as f32 - y as f32 * 0.35) * 0.9).sin();
            let n = fbm(seed, x, y, S, 0.3);
            if a.abs() < 0.18 || b.abs() < 0.12 {
                scale(dark, 0.9 + 0.2 * n)
            } else {
                lerp(base, light, n * 0.5)
            }
        }),
        BarkPattern::Banded => {
            // Glossy bark ringed with pale bands of lenticels.
            let band = second.unwrap_or(light);
            paint(S, S, |x, y| {
                let n = fbm(seed, x, y, S, 0.2);
                let ring = (y as f32 * 1.6 + value_noise(seed ^ 4, x as f32, 0.0, 8.0, 16.0)).sin();
                if ring > 0.8 && rand01(seed ^ 6, x, y) > 0.35 {
                    band
                } else {
                    lerp(scale(base, 0.9), scale(base, 1.15), n)
                }
            })
        }
        BarkPattern::Ridged => paint(S, S, |x, y| {
            // Interlacing ridges in a net of diamonds.
            let a = ((x as f32 * 0.7 + y as f32 * 0.5)
                + value_noise(seed, x as f32, y as f32, 8.0, 16.0))
            .sin();
            let b = (x as f32 * 0.7 - y as f32 * 0.5).sin();
            let n = fbm(seed ^ 2, x, y, S, 0.3);
            if a * b < -0.45 {
                scale(dark, 0.85 + 0.25 * n)
            } else {
                lerp(base, light, 0.3 + n * 0.4)
            }
        }),
        BarkPattern::Flaky => {
            // Reddish bark shedding flakes that show a lighter layer.
            let under = second.unwrap_or(light);
            paint(S, S, |x, y| {
                let (d1, d2, id) = voronoi(seed, x as f32 + 0.5, y as f32 + 0.5, 16.0, 12);
                let n = fbm(seed ^ 1, x, y, S, 0.25);
                if d2 - d1 < 0.7 {
                    dark
                } else if (id >> 30) % 3 == 0 {
                    lerp(under, base, n * 0.4)
                } else {
                    lerp(scale(base, 0.9), light, n * 0.5)
                }
            })
        }
    }
}

/// A log's end: rings of its wood inside a band of bark.
pub fn log_end(bark_rgb: Rgb, wood: Rgb, seed: u64) -> Tex {
    paint(S, S, |x, y| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
        let d = (dx * dx + dy * dy).sqrt() + rand01(seed, x, y) * 0.6;
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        if edge {
            scale(bark_rgb, 0.85 + rand01(seed ^ 1, x, y) * 0.25)
        } else if (d * 0.95) as i32 % 2 == 0 {
            scale(wood, 0.9)
        } else {
            scale(wood, 1.06)
        }
    })
}

/// Foliage: broad leaves in clumps with gaps, or needles in fine strokes, grey for the tint;
/// `light` (about 0.75–1.2) makes a species' leaves lighter or darker than most.
pub fn foliage(kind: LeafKind, density: f32, light: f32, seed: u64) -> Tex {
    let mut t = Tex::new(S, S);
    let light = light.clamp(0.6, 1.3);
    match kind {
        LeafKind::Broad => {
            let holes = 0.42 - 0.22 * density;
            for y in 0..16 {
                for x in 0..16 {
                    let n = fbm(seed, x, y, S, 0.45);
                    if n < holes {
                        continue;
                    }
                    let v = 150.0 + 90.0 * rand01(seed ^ 9, x, y);
                    // Lit tops of clumps, shaded undersides.
                    let v = if fbm(seed ^ 4, x, y - 1, S, 0.2) < holes {
                        v * 1.12
                    } else if (x + y) % 3 == 0 {
                        v * 0.85
                    } else {
                        v
                    };
                    let v = (v * light).min(255.0) as u8;
                    t.set(x, y, [v, v, v]);
                }
            }
        }
        LeafKind::Needle | LeafKind::Scale => {
            // Sprays of needles: short diagonal strokes off dark twigs.
            for k in 0..(26.0 + 14.0 * density) as i32 {
                let x0 = (rand01(seed, k, 0) * 16.0) as i32;
                let y0 = (rand01(seed, k, 1) * 16.0) as i32;
                let dir = if rand01(seed, k, 2) > 0.5 { 1 } else { -1 };
                let v = ((120.0 + 110.0 * rand01(seed, k, 3)) * light).min(255.0) as u8;
                for j in 0..4 {
                    let (x, y) = ((x0 + j * dir).rem_euclid(16), (y0 + j).rem_euclid(16));
                    t.set(x, y, [v, v, v]);
                }
            }
            for y in 0..16 {
                for x in 0..16 {
                    if rand01(seed ^ 5, x, y) < 0.1 * density {
                        t.set(x, y, [96, 96, 96]);
                    }
                }
            }
        }
    }
    t
}

/// The trees' textures, by the blocks their growth forms name: `block/<log>`,
/// `block/<log>_top` and `block/<leaves>`.
pub fn textures(c: &Content) -> Vec<TexEntry> {
    let mut out: Vec<TexEntry> = Vec::new();
    let path = |id: &str| id.rsplit(':').next().unwrap_or(id).to_owned();
    for p in c.plants.iter() {
        let Some(form) = &p.tree else {
            continue;
        };
        let log = path(form.log.as_str());
        if !out.iter().any(|t| t.name == format!("block/{log}")) {
            let base = p.appearance.bark.map_or([100, 84, 66], |c| c.0);
            let second = p.appearance.bark2.map(|c| c.0);
            let wood = p
                .wood
                .as_ref()
                .and_then(|w| c.materials.get(w.as_str()))
                .map_or([190, 160, 120], |m| m.appearance.color.0);
            out.push(TexEntry::still(
                &format!("block/{log}"),
                bark(form.bark, base, second, seed(&log)),
            ));
            out.push(TexEntry::still(
                &format!("block/{log}_top"),
                log_end(base, wood, seed(&log) ^ 0x70),
            ));
        }
        let leaves = path(form.leaves.as_str());
        if !out.iter().any(|t| t.name == format!("block/{leaves}")) {
            // Lighter or darker than a middling green (#557a35), by the species' foliage.
            let light = p.appearance.foliage.map_or(1.0, |c| {
                let l = |c: [u8; 3]| 0.3 * c[0] as f32 + 0.59 * c[1] as f32 + 0.11 * c[2] as f32;
                (l(c.0) / l([0x55, 0x7a, 0x35])).powf(0.8)
            });
            out.push(TexEntry::still(
                &format!("block/{leaves}"),
                foliage(form.leaf, form.foliage_density, light, seed(&leaves)),
            ));
        }
    }
    out
}
