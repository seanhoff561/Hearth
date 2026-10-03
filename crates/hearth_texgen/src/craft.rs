//! Textures of the things people make and gather from (V2-5): useful plants (nettles, hazel,
//! bramble), hearths and fire, racks, lamps, beds, and loose spoil.

use crate::TexEntry;
use crate::paint::{Tex, fbm, lerp, line, paint, rand01, scale};

const S: u32 = 16;

fn h(name: &str) -> u64 {
    hearth_math::hash::derive_seed(0xc4af_7000, name)
}

fn blank() -> Tex {
    Tex::new(S, S)
}

/// Stinging nettle: an upright stem with pairs of toothed, dark leaves.
fn nettle(seed: u64) -> Tex {
    let mut t = blank();
    for (k, x0) in [5, 10].into_iter().enumerate() {
        let top = 2 + (rand01(seed, k as i32, 0) * 3.0) as i32;
        line(&mut t, x0, 15, x0, top, [64, 92, 40]);
        let mut y = 14;
        while y > top + 1 {
            let leaf = [46 + (y as u8) * 2, 86 + (y as u8) * 2, 34];
            for j in 1..=3 {
                let c = if j == 3 { scale(leaf, 0.8) } else { leaf };
                t.put(x0 - j, y - j / 2, c);
                t.put(x0 + j, y - j / 2, c);
            }
            // Teeth along the leaf edge.
            t.put(x0 - 2, y - 2, scale(leaf, 0.7));
            t.put(x0 + 2, y - 2, scale(leaf, 0.7));
            y -= 3;
        }
        t.put(x0, top - 1, [80, 110, 50]);
    }
    t
}

/// Bramble: arching thorny canes, leaves in threes, dark berries.
fn bramble(seed: u64) -> Tex {
    let mut t = blank();
    for c in 0..3 {
        let x0 = 2 + c * 5;
        let lean = if c % 2 == 0 { 1 } else { -1 };
        for k in 0..13 {
            let x = x0 + lean * (k * k) / 40;
            let y = 15 - k;
            t.put(x, y, [92, 52, 54]);
            if k % 3 == 1 {
                // Thorn.
                t.put(x + lean, y, [150, 110, 90]);
            }
            if k % 4 == 2 {
                let leaf = [44, 80 + (k as u8) * 3, 38];
                for (dx, dy) in [(-1, 0), (-2, -1), (1, 0), (2, -1), (0, -1), (0, -2)] {
                    t.put(x + dx, y + dy, leaf);
                }
            }
        }
        // Berries near the tips.
        let bx = x0 + lean * 3;
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let dark = rand01(seed, c, dx + dy * 2) < 0.5;
            t.put(
                bx + dx,
                3 + dy,
                if dark { [34, 18, 40] } else { [70, 26, 58] },
            );
        }
    }
    t
}

/// Hazel, lower half: grey-brown stems rising from the stool, leaves along them.
fn hazel_bottom(seed: u64) -> Tex {
    let mut t = blank();
    for s in 0..4 {
        let x0 = 5 + s * 2;
        let lean = s - 2;
        for k in 0..16 {
            let x = x0 + (lean * k) / 12;
            t.put(x, 15 - k, [120, 100, 82]);
        }
    }
    for i in 0..18 {
        let x = 1 + (rand01(seed, i, 0) * 14.0) as i32;
        let y = 1 + (rand01(seed, i, 1) * 10.0) as i32;
        let g = 0.8 + rand01(seed, i, 2) * 0.4;
        let leaf = scale([70, 116, 46], g);
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1), (-1, 0)] {
            t.put(x + dx, y + dy, leaf);
        }
    }
    t
}

/// Hazel, upper half: a rounded mass of broad leaves with a few clusters of nuts.
fn hazel_top(seed: u64) -> Tex {
    let mut t = blank();
    for y in 2..16 {
        for x in 0..16 {
            let dx = x as f32 - 7.5;
            let dy = (y as f32 - 10.0) * 1.2;
            if dx * dx + dy * dy > 64.0 {
                continue;
            }
            let n = fbm(seed, x, y, S, 0.5);
            if n < 0.32 {
                continue;
            }
            let g = 0.75 + rand01(seed ^ 5, x, y) * 0.45;
            t.put(x, y, scale([72, 120, 48], g));
        }
    }
    for i in 0..3 {
        let x = 3 + (rand01(seed, i, 7) * 10.0) as i32;
        let y = 6 + (rand01(seed, i, 8) * 7.0) as i32;
        t.put(x, y, [150, 120, 60]);
        t.put(x + 1, y, [130, 100, 50]);
    }
    t
}

/// Hearth stones: rounded grey stones blackened by fire.
fn campfire_stones(seed: u64) -> Tex {
    paint(S, S, |x, y| {
        let n = fbm(seed, x, y, S, 0.4);
        let soot = rand01(seed ^ 2, x, y) < 0.25;
        let c = lerp([88, 86, 84], [132, 128, 122], n);
        if soot { scale(c, 0.55) } else { c }
    })
}

/// Logs laid in the hearth: bark with charred patches.
fn campfire_logs(seed: u64) -> Tex {
    paint(S, S, |x, y| {
        let grain = ((y as f32 * 1.7 + rand01(seed, x / 4, 0) * 3.0).sin() * 0.5 + 0.5) * 0.25;
        let char = fbm(seed ^ 9, x, y, S, 0.5) > 0.62;
        if char {
            [38, 30, 26]
        } else {
            scale([110, 80, 52], 0.85 + grain)
        }
    })
}

/// Ash: pale grey powder with a few dark crumbs.
fn campfire_ash(seed: u64) -> Tex {
    paint(S, S, |x, y| {
        let n = rand01(seed, x, y);
        if n < 0.08 {
            [40, 36, 34]
        } else {
            scale([150, 146, 140], 0.85 + n * 0.25)
        }
    })
}

/// Glowing coals: red and orange in the black.
fn embers(seed: u64, phase: f32) -> Tex {
    paint(S, S, |x, y| {
        let n = fbm(seed, x, y, S, 0.5);
        let flicker = rand01(seed ^ (phase * 97.0) as u64, x, y) * 0.3;
        let glow = (n - 0.35 + flicker).clamp(0.0, 1.0);
        if glow < 0.15 {
            [28, 22, 20]
        } else {
            lerp([120, 24, 10], [255, 160, 40], glow)
        }
    })
}

/// Flames: tongues of fire, white-yellow at the root, red at the tips. Frame `f` of `n`.
fn flames(seed: u64, f: i32, n: i32) -> Tex {
    let mut t = blank();
    let phase = f as f32 / n as f32 * std::f32::consts::TAU;
    for tongue in 0..5 {
        let x0 = 2.0 + tongue as f32 * 3.0;
        let hgt = 9.0 + rand01(seed, tongue, 0) * 6.0 + (phase + tongue as f32).sin() * 2.0;
        for k in 0..hgt as i32 {
            let fy = k as f32 / hgt;
            let sway = (phase * 1.3 + fy * 3.0 + tongue as f32).sin() * 1.2 * fy;
            let half = ((1.0 - fy) * 2.2).max(0.5);
            let xc = x0 + sway;
            let c = if fy < 0.25 {
                [255, 246, 190]
            } else if fy < 0.55 {
                [255, 196, 64]
            } else if fy < 0.85 {
                [240, 120, 30]
            } else {
                [200, 60, 20]
            };
            let lo = (xc - half).round() as i32;
            let hi = (xc + half).round() as i32;
            for x in lo..=hi {
                t.set(x, 15 - k, c);
            }
        }
    }
    t
}

/// Poles of a rack: weathered wood.
fn drying_rack(seed: u64) -> Tex {
    paint(S, S, |x, y| {
        let n = rand01(seed, x / 2, y);
        scale([128, 100, 66], 0.8 + n * 0.3)
    })
}

/// A lamp: a hollowed stone holding pale fat around a wick.
fn fat_lamp(seed: u64) -> Tex {
    paint(S, S, |x, y| {
        let dx = x as f32 - 7.5;
        let dy = y as f32 - 7.5;
        let r = (dx * dx + dy * dy).sqrt();
        if r < 1.5 {
            [50, 36, 26]
        } else if r < 5.0 {
            [228, 214, 180]
        } else {
            scale([118, 114, 108], 0.85 + rand01(seed, x, y) * 0.3)
        }
    })
}

/// A heap of dry grass.
fn grass_bed(seed: u64) -> Tex {
    let mut t = Tex::filled(S, S, [150, 126, 70]);
    for b in 0..40 {
        let x0 = (rand01(seed, b, 0) * 16.0) as i32;
        let y0 = (rand01(seed, b, 1) * 16.0) as i32;
        let dx = (rand01(seed, b, 2) * 8.0) as i32 - 4;
        let c = scale([206, 180, 108], 0.8 + rand01(seed, b, 3) * 0.3);
        line(&mut t, x0, y0, x0 + dx, y0 + 3, c);
    }
    t
}

/// Hides with the fur up.
fn fur_bed(seed: u64) -> Tex {
    paint(S, S, |x, y| {
        let n = fbm(seed, x, y, S, 0.6);
        let tip = rand01(seed ^ 4, x, y) < 0.2;
        let c = lerp([96, 70, 46], [160, 126, 88], n);
        if tip { scale(c, 1.25) } else { c }
    })
}

/// A sleeping nest bent from a crown's branches: twigs woven over, the leaves still green on
/// them, browning where they broke.
fn leaf_nest(seed: u64) -> Tex {
    let mut t = paint(S, S, |x, y| {
        let n = fbm(seed, x, y, S, 0.5);
        lerp([62, 72, 34], [96, 104, 46], n)
    });
    for b in 0..26 {
        let x0 = (rand01(seed, b, 0) * 16.0) as i32;
        let y0 = (rand01(seed, b, 1) * 16.0) as i32;
        let dx = (rand01(seed, b, 2) * 10.0) as i32 - 5;
        let dy = (rand01(seed, b, 3) * 6.0) as i32 - 3;
        let c = scale([92, 66, 40], 0.8 + rand01(seed, b, 4) * 0.35);
        line(&mut t, x0, y0, x0 + dx, y0 + dy, c);
    }
    for b in 0..30 {
        let x = (rand01(seed ^ 9, b, 0) * 16.0) as i32;
        let y = (rand01(seed ^ 9, b, 1) * 16.0) as i32;
        let c = if rand01(seed ^ 9, b, 2) < 0.25 {
            [138, 116, 58]
        } else {
            scale([74, 108, 40], 0.85 + rand01(seed ^ 9, b, 3) * 0.3)
        };
        line(&mut t, x, y, x + 1, y, c);
    }
    t
}

/// Loose earth thrown up from digging: clods and crumbs.
fn spoil(seed: u64) -> Tex {
    paint(S, S, |x, y| {
        let n = fbm(seed, x, y, S, 0.35);
        let r = rand01(seed ^ 7, x, y);
        let c = lerp([84, 58, 38], [128, 94, 62], n);
        if r < 0.1 {
            scale(c, 0.65)
        } else if r > 0.93 {
            scale(c, 1.2)
        } else {
            c
        }
    })
}

pub fn textures() -> Vec<TexEntry> {
    let mut v: Vec<TexEntry> = Vec::new();
    let mut add = |name: &str, tex: Tex| v.push(TexEntry::still(&format!("block/{name}"), tex));
    add("nettle", nettle(h("nettle")));
    add("bramble", bramble(h("bramble")));
    add("hazel_bottom", hazel_bottom(h("hazelb")));
    add("hazel_top", hazel_top(h("hazelt")));
    add("campfire", campfire_stones(h("hearth")));
    add("campfire_stones", campfire_stones(h("hearth")));
    add("campfire_logs", campfire_logs(h("hearthlogs")));
    add("campfire_ash", campfire_ash(h("ash")));
    add("drying_rack", drying_rack(h("rack")));
    add("fat_lamp", fat_lamp(h("lamp")));
    add("grass_bed", grass_bed(h("grassbed")));
    add("fur_bed", fur_bed(h("furbed")));
    add("leaf_nest", leaf_nest(h("leafnest")));
    add("spoil", spoil(h("spoil")));
    let frames = 8;
    let flame: Vec<Tex> = (0..frames)
        .map(|f| flames(h("flames"), f, frames))
        .collect();
    v.push(TexEntry {
        name: "block/flames".into(),
        tex: Tex::strip(&flame),
        frames: frames as u32,
        frame_time: 3,
    });
    let glow: Vec<Tex> = (0..4).map(|f| embers(h("embers"), f as f32)).collect();
    v.push(TexEntry {
        name: "block/embers".into(),
        tex: Tex::strip(&glow),
        frames: 4,
        frame_time: 6,
    });
    v
}
