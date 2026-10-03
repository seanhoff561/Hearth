//! Textures of the understory's plants (V2-6): each species drawn by its sprite (a bush hung
//! with fruit, a heath, a rosette, an umbel, a spike of flowers, a fern, a mushroom, a clump of
//! leaves, a creeper, a tuft of sedge, a carpet of moss or lichen) in its own foliage, flower and
//! fruit colours. Plants two blocks tall get a bottom and a top.

use hearth_content::Content;
use hearth_content::schema::flora::{GrowthForm, Sprite};

use crate::TexEntry;
use crate::paint::{Rgb, Tex, line, rand01, scale};

const S: u32 = 16;

fn seed(name: &str) -> u64 {
    hearth_math::hash::derive_seed(0x9a17_3000, name)
}

/// The colours a sprite is drawn in.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub leaf: Rgb,
    pub flower: Option<Rgb>,
    pub fruit: Option<Rgb>,
}

fn shade(c: Rgb, seed: u64, x: i32, y: i32) -> Rgb {
    scale(c, 0.82 + 0.3 * rand01(seed, x, y))
}

/// A leafy mound with fruit (or flowers) dotted in it; `part` 0 a whole short bush, 1 the
/// bottom of a tall one, 2 its top.
fn bush(p: Palette, seed: u64, part: u8) -> Tex {
    let mut t = Tex::new(S, S);
    let stem = scale(p.leaf, 0.55);
    if part != 2 {
        for (k, x0) in [5, 8, 11].into_iter().enumerate() {
            let x1 = x0 + (rand01(seed, k as i32, 9) * 4.0) as i32 - 2;
            line(&mut t, x0, 15, x1, if part == 1 { 0 } else { 6 }, stem);
        }
    }
    let (top, bottom) = match part {
        0 => (3, 13),
        1 => (0, 10),
        _ => (2, 15),
    };
    for y in top..=bottom {
        for x in 1..15 {
            let edge = ((x as f32 - 7.5).abs() / 7.0).powi(2)
                + if part == 2 {
                    ((y as f32 - 9.0) / 7.0).powi(2)
                } else {
                    0.0
                };
            if rand01(seed, x, y) < 0.72 - 0.4 * edge {
                t.set(x, y, shade(p.leaf, seed ^ 1, x, y));
            }
        }
    }
    let dots = p.fruit.or(p.flower);
    if let Some(c) = dots
        && part != 1
    {
        for k in 0..9 {
            let x = 2 + (rand01(seed ^ 3, k, 0) * 12.0) as i32;
            let y = top + 1 + (rand01(seed ^ 3, k, 1) * (bottom - top - 2) as f32) as i32;
            t.set(x, y, c);
            if k % 3 == 0 {
                t.set(x + 1, y, scale(c, 0.8));
            }
        }
    }
    t
}

/// A low, dense, twiggy shrub with flowers or berries on top.
fn heath(p: Palette, seed: u64) -> Tex {
    let mut t = Tex::new(S, S);
    for y in 7..16 {
        for x in 0..16 {
            let height = 8.0 + 2.5 * (x as f32 * 0.9 + rand01(seed, x, 0) * 3.0).sin();
            if (y as f32) >= 16.0 - height * 0.8 && rand01(seed, x, y) < 0.75 {
                t.set(x, y, shade(p.leaf, seed ^ 2, x, y));
            }
        }
    }
    if let Some(c) = p.fruit.or(p.flower) {
        for k in 0..10 {
            let x = (rand01(seed ^ 4, k, 0) * 16.0) as i32;
            let y = 7 + (rand01(seed ^ 4, k, 1) * 5.0) as i32;
            t.set(x, y, c);
        }
    }
    t
}

/// Leaves in a ring on the ground (seen from the side: a low spray).
fn rosette(p: Palette, seed: u64) -> Tex {
    let mut t = Tex::new(S, S);
    for k in 0..7 {
        let x0 = 7 + (k % 2);
        let x1 = 1 + k * 2 + (rand01(seed, k, 0) * 2.0) as i32;
        let y1 = 8 + (rand01(seed, k, 1) * 4.0) as i32;
        line(&mut t, x0, 15, x1, y1, shade(p.leaf, seed, k, 0));
        line(&mut t, x0 + 1, 15, x1 + 1, y1 + 1, scale(p.leaf, 0.85));
    }
    if let Some(c) = p.flower {
        line(&mut t, 8, 15, 8, 6, scale(p.leaf, 0.7));
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)] {
            t.set(8 + dx, 5 + dy, c);
        }
    }
    t
}

/// A stem crowned with a flat umbrella of small flowers; `part` as for `bush`.
fn umbel(p: Palette, seed: u64, part: u8) -> Tex {
    let mut t = Tex::new(S, S);
    let stem = scale(p.leaf, 0.7);
    let flower = p.flower.unwrap_or([240, 238, 230]);
    if part == 1 {
        line(&mut t, 8, 15, 8, 0, stem);
        for k in 0..4 {
            let y = 13 - k * 3;
            let d = if k % 2 == 0 { 1 } else { -1 };
            line(&mut t, 8, y, 8 + d * 5, y - 3, shade(p.leaf, seed, k, y));
        }
        return t;
    }
    line(&mut t, 8, 15, 8, 5, stem);
    if part == 0 {
        for k in 0..3 {
            let y = 14 - k * 2;
            line(
                &mut t,
                8,
                y,
                8 + if k % 2 == 0 { 4 } else { -4 },
                y - 2,
                p.leaf,
            );
        }
    }
    // The umbrella: spokes and a flat head of dots.
    for x in 3..14 {
        line(&mut t, 8, 6, x, 3, scale(stem, 1.1));
        if rand01(seed ^ 7, x, 0) < 0.85 {
            t.set(x, 2, flower);
            t.set(x, 3, scale(flower, 0.9));
        }
    }
    t
}

/// A tall stem with bells or flowers up its length.
fn spike(p: Palette, seed: u64, part: u8) -> Tex {
    let mut t = Tex::new(S, S);
    let stem = scale(p.leaf, 0.75);
    let flower = p.flower.unwrap_or([200, 80, 140]);
    let top = if part == 2 { 2 } else { 0 };
    line(&mut t, 8, 15, 8, top, stem);
    if part == 1 {
        for k in 0..5 {
            let y = 15 - k * 3;
            line(
                &mut t,
                8,
                y,
                3 + (k % 2) * 10,
                y - 2,
                shade(p.leaf, seed, k, 0),
            );
        }
        return t;
    }
    for y in (top + 1..15).step_by(2) {
        let side = if (y / 2) % 2 == 0 { 1 } else { -1 };
        t.set(8 + side, y, flower);
        t.set(8 + 2 * side, y + 1, scale(flower, 0.8));
    }
    if part == 0 {
        for k in 0..3 {
            line(&mut t, 8, 15, 3 + k * 5, 11, p.leaf);
        }
    }
    t
}

/// Arching fronds.
fn fern(p: Palette, seed: u64, part: u8) -> Tex {
    let mut t = Tex::new(S, S);
    let frond = |t: &mut Tex, x0: i32, y0: i32, dx: i32, len: i32| {
        let (mut x, mut y) = (x0, y0);
        for k in 0..len {
            t.set(x, y, shade(p.leaf, seed, x, y));
            // Leaflets either side.
            t.set(x, y - 1, scale(p.leaf, 0.9));
            t.set(x + dx, y + 1, scale(p.leaf, 0.8));
            x += dx;
            if k % 2 == 1 {
                y -= 1;
            }
        }
    };
    if part == 1 {
        line(&mut t, 8, 15, 8, 0, scale(p.leaf, 0.6));
        frond(&mut t, 8, 9, 1, 6);
        frond(&mut t, 8, 5, -1, 6);
        return t;
    }
    for (x0, dx) in [(8, 1), (8, -1), (7, 1), (9, -1)] {
        frond(&mut t, x0, 14, dx, 7);
    }
    frond(&mut t, 8, 8, 1, 5);
    frond(&mut t, 8, 7, -1, 5);
    t
}

/// A cap on a stem: `cap` from the flower colour, the stem from the foliage; spotted caps
/// (fly agaric) when the cap is red.
fn mushroom(p: Palette, seed: u64) -> Tex {
    let mut t = Tex::new(S, S);
    let cap = p.flower.unwrap_or([150, 110, 70]);
    let stem = p.leaf;
    for (k, (cx, w, h)) in [(5, 3, 5), (11, 2, 4)].into_iter().enumerate() {
        for y in (15 - h)..16 {
            t.set(cx, y, scale(stem, 0.95));
            t.set(cx + 1, y, stem);
        }
        let top = 15 - h - 2;
        for dy in 0..3 {
            let half = w + 1 - dy / 2;
            for dx in -half..=half + 1 {
                let c = if dy == 2 { scale(cap, 0.75) } else { cap };
                t.set(cx + dx, top + dy, shade(c, seed ^ k as u64, dx, dy));
            }
        }
        // White warts on a red cap.
        if cap[0] > 180 && cap[1] < 90 {
            t.set(cx - 1, top, [240, 236, 230]);
            t.set(cx + 2, top + 1, [240, 236, 230]);
        }
    }
    t
}

/// Strap-shaped leaves in a clump, with flower heads.
fn clump(p: Palette, seed: u64) -> Tex {
    let mut t = Tex::new(S, S);
    for k in 0..6 {
        let x0 = 4 + k * 2;
        let lean = if k % 2 == 0 { -2 } else { 2 };
        line(
            &mut t,
            x0,
            15,
            x0 + lean,
            6 + (rand01(seed, k, 0) * 3.0) as i32,
            shade(p.leaf, seed, k, 1),
        );
    }
    if let Some(c) = p.flower {
        line(&mut t, 8, 15, 8, 3, scale(p.leaf, 0.8));
        for (dx, dy) in [(0, 0), (1, 1), (-1, 1), (1, -1), (-1, -1), (0, 2)] {
            t.set(8 + dx, 2 + dy, c);
        }
    }
    t
}

/// Low runners with leaves and fruit.
fn creeper(p: Palette, seed: u64) -> Tex {
    let mut t = Tex::new(S, S);
    for y in 10..16 {
        for x in 0..16 {
            if rand01(seed, x, y) < 0.45 {
                t.set(x, y, shade(p.leaf, seed ^ 1, x, y));
            }
        }
    }
    if let Some(c) = p.fruit {
        for k in 0..5 {
            let x = 1 + (rand01(seed ^ 2, k, 0) * 14.0) as i32;
            t.set(x, 12 + (k % 3), c);
            t.set(x, 13 + (k % 3), scale(c, 0.8));
        }
    }
    if let Some(c) = p.flower {
        t.set(4, 10, c);
        t.set(11, 11, c);
    }
    t
}

/// Grassy leaves in a tuft, arching out; flowers or seed heads (cottongrass's white tufts) on
/// stalks above.
fn tuft(p: Palette, seed: u64) -> Tex {
    let mut t = Tex::new(S, S);
    for k in 0..9 {
        let x0 = 5 + (k % 5);
        let lean = (rand01(seed, k, 0) * 9.0) as i32 - 4;
        let y1 = 5 + (rand01(seed, k, 1) * 5.0) as i32;
        line(&mut t, x0, 15, x0 + lean, y1, shade(p.leaf, seed, k, 2));
    }
    if let Some(c) = p.flower.or(p.fruit) {
        for k in 0..3 {
            let x = 4 + k * 4 + (rand01(seed ^ 3, k, 0) * 2.0) as i32;
            line(&mut t, x, 12, x, 3, scale(p.leaf, 0.8));
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1), (-1, 1), (0, -1)] {
                t.set(x + dx, 2 + dy, shade(c, seed ^ 5, x + dx, dy));
            }
        }
    }
    t
}

/// Paddles of a prickly pear, one set on the edge of another, dotted with spines, the fruit on
/// their rims.
fn cactus(p: Palette, seed: u64) -> Tex {
    let mut t = Tex::new(S, S);
    // Ovals: (centre x, centre y, half width, half height).
    let pads = [
        (8.0, 11.5, 3.2, 4.2),
        (4.5, 6.0, 2.6, 3.6),
        (11.0, 5.0, 2.6, 3.4),
    ];
    for (k, (cx, cy, rx, ry)) in pads.into_iter().enumerate() {
        for y in 0..S as i32 {
            for x in 0..S as i32 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                let d = dx * dx + dy * dy;
                if d <= 1.0 {
                    // Darker toward the rim, a spine cluster here and there.
                    let c = if d > 0.7 {
                        scale(p.leaf, 0.8)
                    } else {
                        shade(p.leaf, seed ^ k as u64, x, y)
                    };
                    let spine = (x + 2 * y + k as i32) % 5 == 0 && rand01(seed ^ 7, x, y) < 0.6;
                    t.set(x, y, if spine { [222, 214, 180] } else { c });
                }
            }
        }
    }
    if let Some(c) = p.fruit.or(p.flower) {
        for (x, y) in [(3, 2), (5, 2), (10, 1), (12, 2)] {
            t.set(x, y, c);
            t.set(x, y + 1, scale(c, 0.8));
        }
    }
    t
}

/// A mat over the ground seen from above: moss as close-packed tiny shoots, lighter at their
/// tips; a lichen as pale, branching clumps with dark hollows between (reindeer lichen) or a
/// crust; whatever else lies flat as a speckled cover.
fn carpet(p: Palette, form: GrowthForm, seed: u64) -> Tex {
    let mut t = Tex::new(S, S);
    for y in 0..16 {
        for x in 0..16 {
            let r = rand01(seed, x, y);
            let c = match form {
                GrowthForm::Lichen => {
                    // Clumps of branched stalks: a lattice broken by noise, dark between.
                    let n = rand01(seed ^ 9, x / 2, y / 2);
                    if (x + y) % 3 == 0 && r < 0.6 || n < 0.18 {
                        scale(p.leaf, 0.62)
                    } else {
                        scale(p.leaf, 0.9 + 0.25 * r)
                    }
                }
                GrowthForm::Moss => {
                    // Shoots in rows, each a darker stem and a light tip.
                    if (x + 2 * y) % 4 == 0 {
                        scale(p.leaf, 1.18)
                    } else {
                        scale(p.leaf, 0.78 + 0.3 * r)
                    }
                }
                _ => shade(p.leaf, seed, x, y),
            };
            t.set(x, y, c);
        }
    }
    if let Some(c) = p.fruit.or(p.flower) {
        for k in 0..6 {
            let x = (rand01(seed ^ 4, k, 0) * 16.0) as i32;
            let y = (rand01(seed ^ 4, k, 1) * 16.0) as i32;
            t.set(x, y, c);
        }
    }
    t
}

/// A sprite drawn the height of a block, pressed down into the rows a plant `height_m` tall
/// fills (a third again, for it to show; at least six of the sixteen).
fn squash(t: &Tex, height_m: f32) -> Tex {
    let rows = ((height_m * S as f32 * 1.3).round() as u32).clamp(6, S);
    if rows >= S {
        return t.clone();
    }
    let mut out = Tex::new(S, S);
    for y in 0..rows {
        // The row of the full drawing this one samples (nearest, bottom-anchored).
        let from = ((y as f32 + 0.5) * S as f32 / rows as f32) as i32;
        let to = (S - rows + y) as i32;
        for x in 0..S as i32 {
            out.set_rgba(x, to, t.get(x, from.min(S as i32 - 1)));
        }
    }
    out
}

/// The understory's textures, by the blocks their plants name: `block/<block>`, and for plants
/// over a metre tall `_bottom` and `_top` as well (for blocks two high).
pub fn textures(c: &Content) -> Vec<TexEntry> {
    let mut out = Vec::new();
    for p in c.plants.iter() {
        let (Some(u), Some(sprite)) = (&p.understory, p.appearance.sprite) else {
            continue;
        };
        let name = u.block.as_str().rsplit(':').next().unwrap_or("").to_owned();
        if name.is_empty() {
            continue;
        }
        let pal = Palette {
            leaf: p.appearance.foliage.map_or([80, 120, 60], |c| c.0),
            flower: p.appearance.flower.map(|c| c.0),
            fruit: p.appearance.fruit.map(|c| c.0),
        };
        let sd = seed(&name);
        let tall = p.max_height_m > 1.0
            && !matches!(
                sprite,
                Sprite::Mushroom | Sprite::Heath | Sprite::Carpet | Sprite::Cactus
            );
        let draw = |part: u8| -> Tex {
            match sprite {
                Sprite::Bush => bush(pal, sd, part),
                Sprite::Heath => heath(pal, sd),
                Sprite::Rosette => rosette(pal, sd),
                Sprite::Umbel => umbel(pal, sd, part),
                Sprite::Spike => spike(pal, sd, part),
                Sprite::Fern => fern(pal, sd, part),
                Sprite::Mushroom => mushroom(pal, sd),
                Sprite::Clump => clump(pal, sd),
                Sprite::Creeper => creeper(pal, sd),
                Sprite::Tuft => tuft(pal, sd),
                Sprite::Carpet => carpet(pal, p.form, sd),
                Sprite::Cactus => cactus(pal, sd),
            }
        };
        // A plant well under a metre is drawn at its height in the block: a cushion a hand
        // high, a sedge to the knee (the heaths, creepers and carpets are drawn low already).
        let whole = if tall
            || matches!(
                sprite,
                Sprite::Heath | Sprite::Creeper | Sprite::Carpet | Sprite::Mushroom
            ) {
            draw(0)
        } else {
            squash(&draw(0), p.max_height_m)
        };
        out.push(TexEntry::still(&format!("block/{name}"), whole));
        if tall {
            out.push(TexEntry::still(&format!("block/{name}_bottom"), draw(1)));
            out.push(TexEntry::still(&format!("block/{name}_top"), draw(2)));
        }
    }
    out
}
