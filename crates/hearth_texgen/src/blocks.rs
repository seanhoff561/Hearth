//! Block textures. Every texture is a pure function of its name, so the pack is identical on
//! every machine. Tinted textures (grass, leaves, water, vines) are painted in grey and coloured
//! by climate at render time.

use crate::TexEntry;
use crate::paint::{
    Palette, Rgb, Tex, fbm, lerp, line, paint, rand01, scale, value_noise, voronoi,
};

const S: u32 = 16;

fn h(name: &str) -> u64 {
    hearth_math::hash::derive_seed(0x4ea7_7e00, name)
}

// ------------------------------------------------------------------ palettes

fn stone_pal() -> Palette {
    Palette(vec![
        [98, 98, 98],
        [112, 112, 112],
        [122, 122, 122],
        [131, 131, 131],
        [143, 143, 143],
    ])
}

fn dirt_pal() -> Palette {
    Palette(vec![
        [94, 64, 42],
        [110, 76, 50],
        [126, 88, 58],
        [140, 99, 66],
        [152, 110, 76],
    ])
}

// ------------------------------------------------------------------ generic generators

/// Noise-dithered material: fractal noise quantised to a palette.
fn noisy(seed: u64, pal: &Palette, grain: f32) -> Tex {
    paint(S, S, |x, y| pal.pick(fbm(seed, x, y, S, grain)))
}

/// Stone-like texture with a few darker cracks.
fn rocky(seed: u64, pal: &Palette, cracks: u32) -> Tex {
    let mut t = noisy(seed, pal, 0.45);
    for c in 0..cracks {
        let x0 = (rand01(seed, c as i32, 1) * 16.0) as i32;
        let y0 = (rand01(seed, c as i32, 2) * 16.0) as i32;
        let len = 2 + (rand01(seed, c as i32, 3) * 3.0) as i32;
        let dx = if rand01(seed, c as i32, 4) > 0.5 {
            1
        } else {
            -1
        };
        for k in 0..len {
            let (x, y) = (x0 + k * dx, y0 + k / 2);
            let p = t.get(x, y);
            t.set(x, y, scale([p[0], p[1], p[2]], 0.82));
        }
    }
    t
}

/// Bark: vertical stripes with knots.
fn log_side(seed: u64, dark: Rgb, light: Rgb) -> Tex {
    paint(S, S, |x, y| {
        let stripe = value_noise(seed, x as f32 * 1.0, y as f32 * 0.25, 2.0, 16.0);
        let fine = rand01(seed ^ 0xb, x, y);
        let t = stripe * 0.75 + fine * 0.25;
        let c = lerp(dark, light, t);
        if (x + (rand01(seed, x, 99) * 3.0) as i32) % 5 == 0 {
            scale(c, 0.8)
        } else {
            c
        }
    })
}

/// Log end: annual rings inside a bark border.
fn log_top(seed: u64, bark: Rgb, wood: Rgb) -> Tex {
    paint(S, S, |x, y| {
        let dx = x as f32 - 7.5;
        let dy = y as f32 - 7.5;
        let d = (dx * dx + dy * dy).sqrt() + rand01(seed, x, y) * 0.6;
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        if edge {
            scale(bark, 0.9 + rand01(seed ^ 1, x, y) * 0.2)
        } else if (d * 0.9) as i32 % 2 == 0 {
            scale(wood, 0.92)
        } else {
            scale(wood, 1.06)
        }
    })
}

/// Leaves: grey clusters with transparent gaps (tinted at render time).
fn leaves(seed: u64, holes: f32, lo: f32, hi: f32) -> Tex {
    let mut t = Tex::new(S, S);
    for y in 0..16 {
        for x in 0..16 {
            let n = fbm(seed, x, y, S, 0.5);
            if n < holes {
                continue;
            }
            let v = (lo + (hi - lo) * rand01(seed ^ 9, x, y)) * 255.0;
            let v = if (x + y) % 3 == 0 { v * 0.85 } else { v };
            t.set(x, y, [v as u8, v as u8, v as u8]);
        }
    }
    t
}

/// Irregular stones with mortar lines (cobblestone).
fn cobble(seed: u64, pal: &Palette, mortar: Rgb) -> Tex {
    paint(S, S, |x, y| {
        let (d1, d2, id) = voronoi(seed, x as f32 + 0.5, y as f32 + 0.5, 16.0, 11);
        if d2 - d1 < 1.1 {
            mortar
        } else {
            let base = ((id >> 20) % 1000) as f32 / 1000.0;
            let shade = (0.2 + base * 0.6 + rand01(seed, x, y) * 0.2).min(0.99);
            let c = pal.pick(shade);
            // Light from the top-left: brighten stone near its top edge.
            if d1 > 2.2 { scale(c, 1.05) } else { c }
        }
    })
}

/// Moss growing over a texture.
fn mossify(mut t: Tex, seed: u64, amount: f32) -> Tex {
    let moss = Palette(vec![
        [66, 92, 38],
        [80, 108, 44],
        [94, 124, 50],
        [108, 136, 58],
    ]);
    for y in 0..16 {
        for x in 0..16 {
            let n = fbm(seed, x, y, S, 0.3);
            if n < amount {
                t.set(x, y, moss.pick(rand01(seed ^ 3, x, y)));
            }
        }
    }
    t
}

// ------------------------------------------------------------------ sprites

fn blank() -> Tex {
    Tex::new(S, S)
}

/// Grass blades (grey, tinted).
fn grass_blades(seed: u64, count: i32, max_h: i32) -> Tex {
    let mut t = blank();
    for b in 0..count {
        let x0 = 1 + (rand01(seed, b, 0) * 14.0) as i32;
        let hgt = max_h / 2 + (rand01(seed, b, 1) * (max_h / 2) as f32) as i32;
        let lean = (rand01(seed, b, 2) * 3.0) as i32 - 1;
        for k in 0..hgt {
            let x = x0 + (lean * k) / hgt.max(1);
            let v = 120 + (k * 90 / hgt.max(1)) as u8;
            t.put(x, 15 - k, [v, v, v]);
        }
    }
    t
}

/// A fern: fronds branching from a central stem.
fn fern(seed: u64) -> Tex {
    let mut t = blank();
    for f in 0..3 {
        let x0 = 4 + f * 4;
        let hgt = 9 + (rand01(seed, f, 0) * 5.0) as i32;
        for k in 0..hgt {
            let y = 15 - k;
            let v = 130 + (k * 7) as u8;
            t.put(x0, y, [v, v, v]);
            if k % 2 == 1 && k < hgt - 1 {
                let w = ((hgt - k) / 3).max(1);
                for j in 1..=w {
                    t.put(x0 - j, y + j / 2, [v - 20, v - 20, v - 20]);
                    t.put(x0 + j, y + j / 2, [v - 20, v - 20, v - 20]);
                }
            }
        }
    }
    t
}

/// A flower: stem, leaves and a coloured head.
fn flower(seed: u64, petals: Rgb, center: Rgb, shape: u8) -> Tex {
    let mut t = blank();
    let stem = [58, 118, 34];
    let x = 7;
    for y in 7..16 {
        t.put(x, y, stem);
    }
    t.put(x - 1, 12, stem);
    t.put(x - 2, 11, stem);
    t.put(x + 1, 13, stem);
    match shape {
        // Round head (dandelion, poppy).
        0 => {
            for (dx, dy) in [
                (0, -1),
                (-1, 0),
                (1, 0),
                (0, 1),
                (-1, -1),
                (1, -1),
                (-1, 1),
                (1, 1),
            ] {
                t.put(x + dx, 5 + dy, petals);
            }
            t.put(x, 5, center);
            t.put(x - 1, 3, scale(petals, 0.85));
            t.put(x + 1, 3, scale(petals, 0.85));
            t.put(x, 3, petals);
        }
        // Daisy: white ray petals around a yellow disc.
        1 => {
            for (dx, dy) in [
                (0, -2),
                (-2, 0),
                (2, 0),
                (0, 2),
                (-1, -1),
                (1, -1),
                (-1, 1),
                (1, 1),
            ] {
                t.put(x + dx, 5 + dy, petals);
            }
            t.put(x, 5, center);
        }
        // Bells hanging from an arched stem (lily of the valley).
        2 => {
            for (k, y) in [4, 7, 10].iter().enumerate() {
                let bx = x + 2 + (k as i32 % 2);
                t.put(bx, *y, petals);
                t.put(bx, *y + 1, scale(petals, 0.9));
                t.put(bx - 1, *y - 1, stem);
            }
        }
        // Small clusters (bluet, aster, gentian).
        _ => {
            for (dx, dy) in [(-2, 5), (2, 4), (0, 3), (-1, 6), (1, 6)] {
                t.put(x + dx, dy, petals);
                t.put(x + dx, dy + 1, scale(petals, 0.8));
            }
            t.put(x, 4, center);
        }
    }
    let _ = seed;
    t
}

fn mushroom(cap: Rgb, dots: bool) -> Tex {
    let mut t = blank();
    let stem = [220, 210, 190];
    for y in 11..16 {
        t.put(7, y, stem);
        t.put(8, y, stem);
    }
    for (y, w) in [(8, 3), (9, 4), (10, 4)] {
        for x in 8 - w..8 + w {
            t.put(x, y, cap);
        }
    }
    if dots {
        for (x, y) in [(6, 9), (9, 8), (10, 10)] {
            t.put(x, y, [240, 240, 240]);
        }
    }
    t
}

fn sapling(seed: u64, leaf: Rgb, trunk: Rgb, conical: bool) -> Tex {
    let mut t = blank();
    for y in 9..16 {
        t.put(7, y, trunk);
        t.put(8, y, trunk);
    }
    for y in 2i32..11 {
        let w = if conical {
            (y - 2) / 2 + 1
        } else {
            3 - ((y - 6).abs() / 2)
        };
        for x in 8 - w..8 + w {
            if rand01(seed, x, y) > 0.2 {
                t.put(x, y, scale(leaf, 0.85 + rand01(seed ^ 1, x, y) * 0.3));
            }
        }
    }
    t
}

fn dead_bush(seed: u64) -> Tex {
    let mut t = blank();
    let c = [122, 86, 40];
    line(&mut t, 7, 15, 7, 8, c);
    line(&mut t, 7, 11, 3, 5, c);
    line(&mut t, 7, 10, 12, 4, c);
    line(&mut t, 5, 8, 2, 8, c);
    line(&mut t, 10, 7, 13, 9, c);
    t.put(3, 4, [140, 100, 50]);
    let _ = seed;
    t
}

fn sugar_cane() -> Tex {
    let mut t = blank();
    for &x in &[3, 8, 12] {
        for y in 0..16 {
            let joint = (y + x) % 6 == 0;
            let v = if joint { 150 } else { 185 };
            t.put(x, y, [v, v, v]);
            t.put(x + 1, y, [v - 25, v - 25, v - 25]);
        }
        t.put(x + 2, (x * 3) % 16, [170, 170, 170]);
    }
    t
}

fn kelp(seed: u64, top: bool) -> Tex {
    let mut t = blank();
    let green = [72, 118, 42];
    for y in 0..16 {
        let x = 7 + ((y as f32 * 0.8 + seed as f32).sin() * 2.0) as i32;
        t.put(x, y, green);
        t.put(x + 1, y, scale(green, 0.85));
        if y % 4 == 1 {
            t.put(x - 1, y, [92, 140, 50]);
        }
    }
    if top {
        for y in 0..4 {
            t.clear(7, y);
            t.clear(8, y);
        }
    }
    t
}

fn seagrass(seed: u64) -> Tex {
    let mut t = blank();
    for b in 0..5 {
        let x0 = 2 + b * 3;
        let hgt = 8 + (rand01(seed, b, 0) * 7.0) as i32;
        for k in 0..hgt {
            let x = x0 + ((k as f32 * 0.5 + b as f32).sin() * 1.2) as i32;
            t.put(x, 15 - k, [52 + (k * 3) as u8, 110 + (k * 4) as u8, 40]);
        }
    }
    t
}

fn cactus_side(seed: u64) -> Tex {
    paint(S, S, |x, y| {
        let rib = x % 4 == 1;
        let spine = (x % 4 == 3) && (y % 4 == (x / 4) % 4);
        if spine {
            [230, 230, 200]
        } else if rib {
            [50, 110, 40]
        } else {
            scale([72, 140, 52], 0.9 + rand01(seed, x, y) * 0.15)
        }
    })
}

fn vine(seed: u64) -> Tex {
    let mut t = blank();
    for y in 0..16 {
        for x in 0..16 {
            let n = fbm(seed, x, y, S, 0.5);
            if n > 0.52 && (x + y * 3) % 5 != 0 {
                let v = (120.0 + n * 100.0) as u8;
                t.put(x, y, [v, v, v]);
            }
        }
    }
    t
}

fn lily_pad(seed: u64) -> Tex {
    let mut t = blank();
    for y in 0..16 {
        for x in 0..16 {
            let dx = x as f32 - 7.5;
            let dy = y as f32 - 7.5;
            let d = (dx * dx + dy * dy).sqrt();
            let notch = dx > 0.0 && dy.abs() < 1.2;
            if d < 7.4 && !notch {
                let v = 150 + (rand01(seed, x, y) * 50.0) as u8;
                t.put(x, y, [v, v, v]);
            }
        }
    }
    t
}

// ------------------------------------------------------------------ the pack

/// All block textures (name without namespace, e.g. `block/stone`).
pub fn textures() -> Vec<TexEntry> {
    let mut v: Vec<TexEntry> = Vec::new();
    let mut add = |name: &str, tex: Tex| v.push(TexEntry::still(&format!("block/{name}"), tex));

    // Stone family.
    let stone = rocky(h("stone"), &stone_pal(), 5);
    add("stone", stone.clone());
    add(
        "cobblestone",
        cobble(h("cobble"), &stone_pal(), [76, 76, 76]),
    );
    add(
        "mossy_cobblestone",
        mossify(
            cobble(h("cobble"), &stone_pal(), [76, 76, 76]),
            h("moss1"),
            0.38,
        ),
    );
    // Rock types are drawn from their materials' appearance (`material.rs`).

    // Soils.
    let dirt = noisy(h("dirt"), &dirt_pal(), 0.6);
    // Soils and sediments are drawn from their materials (`material.rs`); the dirt above is
    // the base of the turf side textures.
    add(
        "moss_block",
        noisy(
            h("moss"),
            &Palette(vec![
                [72, 98, 36],
                [84, 112, 42],
                [96, 124, 48],
                [108, 136, 56],
            ]),
            0.55,
        ),
    );
    add("podzol_top", {
        let seed = h("podzol");
        paint(S, S, |x, y| {
            let n = fbm(seed, x, y, S, 0.6);
            if n > 0.62 {
                [74, 90, 40]
            } else {
                Palette(vec![
                    [72, 48, 26],
                    [88, 60, 32],
                    [102, 70, 38],
                    [116, 80, 44],
                ])
                .pick(n)
            }
        })
    });
    add("podzol_side", {
        let mut t = dirt.clone();
        for x in 0..16 {
            let depth = 2 + (rand01(h("podzs"), x, 0) * 3.0) as i32;
            for y in 0..depth {
                t.set(
                    x,
                    y,
                    lerp([88, 60, 32], [70, 88, 40], rand01(h("podzs"), x, y + 5)),
                );
            }
        }
        t
    });
    // Grass: grey top and overlay fringe (tinted), dirt side base.
    let mut grass_top = noisy(
        h("grass_top"),
        &Palette(vec![
            [132, 132, 132],
            [146, 146, 146],
            [158, 158, 158],
            [170, 170, 170],
            [182, 182, 182],
        ]),
        0.7,
    );
    grass_top.to_gray(0.5, 0.78);
    add("grass_block_top", grass_top);
    add("grass_block_side", {
        let mut t = dirt.clone();
        // Untinted darker fringe under the tinted overlay reads well at distance.
        for x in 0..16 {
            let depth = 3 + (rand01(h("gside"), x, 0) * 2.5) as i32;
            for y in 0..depth {
                t.set(x, y, [100, 100, 70]);
            }
        }
        t
    });
    add("grass_block_side_overlay", {
        let mut t = Tex::new(S, S);
        for x in 0..16 {
            let depth = 3 + (rand01(h("gside"), x, 0) * 2.5) as i32;
            for y in 0..depth {
                let v = 150 + (rand01(h("gside2"), x, y) * 40.0) as u8;
                t.set(x, y, [v, v, v]);
            }
        }
        t
    });
    add("grass_block_snow", {
        let mut t = dirt.clone();
        for x in 0..16 {
            let depth = 3 + (rand01(h("gsnow"), x, 0) * 3.0) as i32;
            for y in 0..depth {
                t.set(x, y, [236, 242, 246]);
            }
        }
        t
    });
    // Snow and ice.
    let snow_pal = Palette(vec![
        [226, 234, 240],
        [234, 240, 245],
        [242, 247, 250],
        [250, 252, 254],
    ]);
    add("snow", noisy(h("snow"), &snow_pal, 0.4));
    add("ice", {
        let mut t = paint(S, S, |x, y| {
            let streak = (x + y * 2) % 11 == 0 || (x * 2 + y) % 13 == 0;
            if streak {
                [220, 236, 252]
            } else {
                lerp([140, 178, 236], [168, 200, 245], rand01(h("ice"), x, y))
            }
        });
        for p in &mut t.px {
            p[3] = 190;
        }
        t
    });
    add(
        "packed_ice",
        paint(S, S, |x, y| {
            let n = fbm(h("pice"), x, y, S, 0.3);
            lerp([130, 168, 230], [190, 214, 248], n)
        }),
    );

    // Wood.
    let woods: [(&str, Rgb, Rgb, Rgb); 3] = [
        // name, bark dark, bark light, wood
        ("oak", [72, 56, 34], [110, 86, 52], [176, 142, 86]),
        ("birch", [196, 196, 186], [236, 236, 228], [206, 186, 124]),
        ("spruce", [52, 36, 20], [86, 62, 36], [130, 98, 58]),
    ];
    for (name, dark, light, wood) in woods {
        let seed = h(name);
        let mut side = log_side(seed, dark, light);
        if name == "birch" {
            // Dark lenticels on white bark.
            for i in 0..10 {
                let x = (rand01(seed, i, 0) * 16.0) as i32;
                let y = (rand01(seed, i, 1) * 16.0) as i32;
                side.set(x, y, [40, 40, 36]);
                side.set(x + 1, y, [40, 40, 36]);
                if i % 2 == 0 {
                    side.set(x + 2, y, [60, 60, 54]);
                }
            }
        }
        add(&format!("{name}_log"), side.clone());
        add(&format!("{name}_log_top"), log_top(seed, dark, wood));
        let (holes, lo, hi) = match name {
            "spruce" => (0.3, 0.42, 0.7),
            "birch" => (0.34, 0.5, 0.8),
            _ => (0.32, 0.46, 0.76),
        };
        add(&format!("{name}_leaves"), leaves(seed ^ 11, holes, lo, hi));
        let leaf = match name {
            "spruce" => [52, 92, 60],
            "birch" => [110, 160, 80],
            _ => [70, 130, 50],
        };
        add(
            &format!("{name}_sapling"),
            sapling(seed ^ 13, leaf, dark, name == "spruce"),
        );
    }

    // Plants and sprites.
    add("short_grass", grass_blades(h("sgrass"), 9, 11));
    add("tall_grass_bottom", grass_blades(h("tgrassb"), 11, 16));
    add("tall_grass_top", grass_blades(h("tgrasst"), 8, 12));
    add("fern", fern(h("fern")));
    add("large_fern_bottom", fern(h("lfernb")));
    add("large_fern_top", fern(h("lfernt")));
    add("short_dry_grass", grass_blades(h("sdry"), 8, 9));
    add("tall_dry_grass_bottom", grass_blades(h("tdryb"), 10, 16));
    add("tall_dry_grass_top", grass_blades(h("tdryt"), 7, 10));
    add("dead_bush", dead_bush(h("deadbush")));
    add(
        "dandelion",
        flower(h("dandelion"), [248, 220, 40], [220, 170, 20], 0),
    );
    add("poppy", flower(h("poppy"), [214, 36, 30], [40, 30, 30], 0));
    add(
        "cornflower",
        flower(h("cornflower"), [70, 100, 220], [40, 60, 160], 3),
    );
    add(
        "oxeye_daisy",
        flower(h("oxeye"), [244, 244, 240], [240, 200, 40], 1),
    );
    add(
        "lily_of_the_valley",
        flower(h("lotv"), [248, 248, 244], [230, 230, 220], 2),
    );
    add(
        "azure_bluet",
        flower(h("bluet"), [216, 226, 250], [240, 220, 120], 3),
    );
    add(
        "gentian",
        flower(h("gentian"), [40, 70, 200], [20, 40, 140], 0),
    );
    add(
        "alpine_aster",
        flower(h("aster"), [160, 110, 220], [240, 210, 80], 1),
    );
    add("brown_mushroom", mushroom([150, 110, 80], false));
    add("red_mushroom", mushroom([200, 40, 40], true));
    add("sugar_cane", sugar_cane());
    add("kelp", kelp(1, true));
    add("kelp_plant", kelp(1, false));
    add("seagrass", seagrass(h("seagrass")));
    add("tall_seagrass_bottom", seagrass(h("tseab")));
    add("tall_seagrass_top", seagrass(h("tseat")));
    add("cactus_side", cactus_side(h("cactus")));
    add(
        "cactus_top",
        paint(S, S, |x, y| {
            let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
            if d < 3.0 {
                [100, 160, 70]
            } else {
                scale([72, 140, 52], 0.9 + rand01(h("ctop"), x, y) * 0.1)
            }
        }),
    );
    add(
        "cactus_bottom",
        paint(S, S, |x, y| {
            scale([90, 150, 60], 0.9 + rand01(h("cbot"), x, y) * 0.1)
        }),
    );
    add("vine", vine(h("vine")));
    add("lily_pad", lily_pad(h("lily")));
    add(
        "moss_carpet",
        noisy(
            h("moss"),
            &Palette(vec![
                [72, 98, 36],
                [84, 112, 42],
                [96, 124, 48],
                [108, 136, 56],
            ]),
            0.55,
        ),
    );

    // Water: grey animated waves (tinted by climate at render time).
    let mut still = Vec::new();
    let mut flow = Vec::new();
    for f in 0..16 {
        let phase = f as f32 / 16.0;
        still.push(paint(S, S, |x, y| {
            let a = value_noise(0x3a7e, x as f32 + phase * 16.0, y as f32, 8.0, 16.0);
            let b = value_noise(0x3a7f, x as f32, y as f32 + phase * 16.0, 4.0, 16.0);
            let v = (150.0 + (a * 0.6 + b * 0.4) * 80.0) as u8;
            [v, v, v]
        }));
        flow.push(paint(S, S, |x, y| {
            let a = value_noise(0x3a80, x as f32, y as f32 - phase * 16.0, 4.0, 16.0);
            let v = (150.0 + a * 90.0) as u8;
            [v, v, v]
        }));
    }
    let with_alpha = |mut t: Tex| {
        for p in &mut t.px {
            p[3] = 180;
        }
        t
    };
    v.push(TexEntry {
        name: "block/water_still".into(),
        tex: with_alpha(Tex::strip(&still)),
        frames: 16,
        frame_time: 2,
    });
    v.push(TexEntry {
        name: "block/water_flow".into(),
        tex: with_alpha(Tex::strip(&flow)),
        frames: 16,
        frame_time: 1,
    });
    // Break-crack overlay stages.
    for stage in 0..10 {
        let mut t = Tex::new(S, S);
        let n = (stage + 1) * 3;
        for c in 0..n {
            let mut x = 8 + (rand01(h("destroy"), c, 0) * 8.0) as i32 - 4;
            let mut y = 8 + (rand01(h("destroy"), c, 1) * 8.0) as i32 - 4;
            let len = 2 + stage / 2;
            for k in 0..len {
                t.set_rgba(x, y, [20, 20, 20, 200]);
                let d = rand01(h("destroy"), c, k + 10);
                if d < 0.25 {
                    x += 1
                } else if d < 0.5 {
                    x -= 1
                } else if d < 0.75 {
                    y += 1
                } else {
                    y -= 1
                }
            }
        }
        v.push(TexEntry::still(&format!("block/destroy_stage_{stage}"), t));
    }
    // Missing-texture placeholder (magenta/black checker) for unknown blocks.
    v.push(TexEntry::still(
        "block/missing",
        paint(S, S, |x, y| {
            if (x / 8 + y / 8) % 2 == 0 {
                [248, 0, 248]
            } else {
                [0, 0, 0]
            }
        }),
    ));
    v
}
