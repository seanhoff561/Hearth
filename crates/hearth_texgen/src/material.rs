//! Textures drawn from a material's appearance (colour, second colour, pattern, roughness), so
//! blocks generated from data (every rock type, later soils and ores) get pixel art without
//! code. Deterministic: the same name and appearance always give the same image.

use hearth_content::Content;
use hearth_content::schema::material::{Appearance, Pattern};

use crate::TexEntry;
use crate::paint::{Palette, Rgb, Tex, fbm, lerp, paint, rand01, scale, value_noise, voronoi};

const S: u32 = 16;

fn seed_of(name: &str) -> u64 {
    hearth_math::hash::derive_seed(0x6d61_7465, name)
}

/// A 16×16 texture for a material's appearance.
pub fn texture(name: &str, a: &Appearance) -> Tex {
    let seed = seed_of(name);
    let base: Rgb = a.color.0;
    let second: Rgb = a.color2.map_or_else(|| scale(base, 0.72), |c| c.0);
    // Matte materials show more pixel contrast than glossy ones.
    let rough = a.roughness.unwrap_or(0.7).clamp(0.0, 1.0);
    let pal = Palette::around(base, 5, 0.06 + 0.1 * rough);
    let fs = S as f32;
    match a.pattern {
        Pattern::Plain => paint(S, S, |x, y| pal.pick(fbm(seed, x, y, S, 0.35))),
        Pattern::Grainy | Pattern::Powder => {
            let grain = if a.pattern == Pattern::Powder {
                0.25
            } else {
                0.6
            };
            paint(S, S, |x, y| {
                let c = pal.pick(fbm(seed, x, y, S, grain));
                // Occasional darker or lighter grains.
                let r = rand01(seed ^ 0x9a, x, y);
                if a.pattern == Pattern::Grainy && r < 0.06 {
                    lerp(c, second, 0.6)
                } else if r > 0.97 {
                    scale(c, 1.12)
                } else {
                    c
                }
            })
        }
        Pattern::Speckled => paint(S, S, |x, y| {
            let r = rand01(seed ^ 0x5e, x, y);
            if r < 0.14 {
                second
            } else if r > 0.95 {
                scale(base, 1.18)
            } else {
                pal.pick(fbm(seed, x, y, S, 0.5))
            }
        }),
        Pattern::Crystalline => paint(S, S, |x, y| {
            // Interlocking crystals: Voronoi cells, some of the second mineral.
            let (d1, d2, id) = voronoi(seed, x as f32 + 0.5, y as f32 + 0.5, fs, 14);
            let t = ((id >> 24) % 1000) as f32 / 1000.0;
            let c = if t < 0.3 {
                second
            } else {
                pal.pick(0.15 + t * 0.8)
            };
            if d2 - d1 < 0.6 { scale(c, 0.9) } else { c }
        }),
        Pattern::Layered => paint(S, S, |x, y| {
            // Beds: bands along x with slight waviness, alternating tones.
            let wave = value_noise(seed, x as f32, 0.0, 8.0, fs) * 2.0;
            let band = value_noise(seed ^ 0x1a, 0.0, y as f32 + wave, 3.0, fs);
            let c = lerp(
                pal.pick(0.3 + 0.5 * band),
                second,
                (band - 0.6).max(0.0) * 1.5,
            );
            scale(c, 0.95 + 0.1 * rand01(seed ^ 0x2b, x, y))
        }),
        Pattern::Banded => paint(S, S, |x, y| {
            let wave = value_noise(seed, x as f32, 0.0, 8.0, fs) * 3.0;
            let t = ((y as f32 + wave) / 3.0).sin() * 0.5 + 0.5;
            let c = lerp(base, second, t);
            scale(c, 0.94 + 0.12 * rand01(seed ^ 0x3c, x, y))
        }),
        Pattern::Veined => {
            let mut t = paint(S, S, |x, y| pal.pick(fbm(seed, x, y, S, 0.3)));
            // A few thin wandering veins of the second colour.
            for v in 0..3 {
                let mut y = rand01(seed, v, 1) * fs;
                let slope = rand01(seed, v, 2) * 1.6 - 0.8;
                for x in 0..S as i32 {
                    y += slope + (rand01(seed, v, x + 3) - 0.5) * 0.8;
                    t.set(x, y.rem_euclid(fs) as i32, second);
                }
            }
            t
        }
        Pattern::Glassy => paint(S, S, |x, y| {
            // Smooth glass with conchoidal ripples and a few glints.
            let ripple = ((x as f32 * 0.7
                + y as f32 * 0.4
                + value_noise(seed, x as f32, y as f32, 4.0, fs) * 4.0)
                .sin())
                * 0.5
                + 0.5;
            let c = lerp(base, second, ripple * 0.35 * (1.0 - rough * 0.5));
            if rand01(seed ^ 0x61, x, y) > 0.975 {
                scale(c, 1.9)
            } else {
                c
            }
        }),
        Pattern::Porous => paint(S, S, |x, y| {
            // Vesicles: small dark holes.
            let (d1, _, _) = voronoi(seed, x as f32 + 0.5, y as f32 + 0.5, fs, 18);
            if d1 < 0.9 {
                scale(second, 0.7)
            } else {
                pal.pick(fbm(seed, x, y, S, 0.55))
            }
        }),
        Pattern::Fibrous => paint(S, S, |x, y| {
            let streak = value_noise(seed, x as f32 * 0.3, y as f32 * 3.0, 2.0, fs);
            lerp(pal.pick(fbm(seed, x, y, S, 0.4)), second, streak * 0.5)
        }),
        Pattern::Clumpy => paint(S, S, |x, y| {
            // Rounded clasts in a finer matrix (gravel, breccia, conglomerate).
            let (d1, d2, id) = voronoi(seed, x as f32 + 0.5, y as f32 + 0.5, fs, 10);
            let t = ((id >> 24) % 1000) as f32 / 1000.0;
            if d2 - d1 < 1.0 {
                scale(second, 0.85 + 0.2 * rand01(seed, x, y))
            } else {
                let c = lerp(pal.pick(t), second, (t - 0.7).max(0.0));
                if d1 < 1.8 { scale(c, 1.06) } else { c }
            }
        }),
        Pattern::WoodGrain | Pattern::Bark => paint(S, S, |x, y| {
            let grain = value_noise(seed, x as f32 * 3.0, y as f32 * 0.25, 2.0, fs);
            lerp(base, second, grain * 0.6)
        }),
    }
}

/// Rock carrying an ore: a grey rock matrix flecked and veined with the mineral, glinting
/// where the mineral is metallic.
pub fn ore_texture(name: &str, a: &Appearance) -> Tex {
    let seed = seed_of(name);
    let matrix = Palette(vec![
        [98, 96, 92],
        [110, 107, 102],
        [121, 118, 112],
        [133, 129, 122],
    ]);
    let mineral: Rgb = a.color.0;
    let second: Rgb = a.color2.map_or_else(|| scale(mineral, 0.75), |c| c.0);
    let shiny = a.roughness.unwrap_or(0.7) < 0.45;
    paint(S, S, |x, y| {
        let blob = value_noise(seed, x as f32, y as f32, 3.0, S as f32);
        let r = rand01(seed ^ 0x0e, x, y);
        if blob > 0.62 || r < 0.08 {
            let c = if r < 0.5 { mineral } else { second };
            if shiny && rand01(seed ^ 0x61, x, y) > 0.8 {
                scale(c, 1.5)
            } else {
                c
            }
        } else {
            matrix.pick(fbm(seed, x, y, S, 0.5))
        }
    })
}

/// Stream gravel with heavy grains of a mineral in it (a placer).
pub fn placer_texture(name: &str, a: &Appearance) -> Tex {
    let seed = seed_of(name);
    let grains: Rgb = a.color.0;
    let shiny = a.roughness.unwrap_or(0.7) < 0.45;
    paint(S, S, |x, y| {
        let (d1, d2, id) = voronoi(seed, x as f32 + 0.5, y as f32 + 0.5, S as f32, 20);
        if rand01(seed ^ 0x9d, x, y) < 0.07 {
            return if shiny { scale(grains, 1.4) } else { grains };
        }
        if d2 - d1 < 0.6 {
            [84, 80, 76]
        } else {
            let tone = (id % 5) as f32 / 5.0;
            let c = lerp([114, 108, 102], [168, 160, 150], tone);
            if d1 < 1.0 { scale(c, 1.08) } else { c }
        }
    })
}

/// Textures for the natural blocks generated from the content (`block/<path>`).
pub fn natural_textures(content: &Content) -> Vec<TexEntry> {
    use hearth_content::generate::NaturalKind;
    hearth_content::generate::natural_blocks(content)
        .into_iter()
        .filter_map(|b| {
            let m = content.materials.get(&b.material)?;
            let path = b.id.split_once(':').map_or(b.id.as_str(), |(_, p)| p);
            let tex = match b.kind {
                NaturalKind::Ore => ore_texture(path, &m.appearance),
                NaturalKind::Placer => placer_texture(path, &m.appearance),
                _ => texture(path, &m.appearance),
            };
            Some(TexEntry::still(&format!("block/{path}"), tex))
        })
        .collect()
}
