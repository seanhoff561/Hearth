//! Knapping by hand (v2 §11.3): the stone seen from above as a grid of cells, and the shape
//! wanted drawn over it. Each strike at the edge drives a flake off into the stone along the
//! blow: its length follows the force, and how predictably it runs follows the stone (flint and
//! obsidian take the line they are given; quartzite and basalt stop short in steps or run on).
//! A blow that would cut the piece in two snaps it. The work's quality is how well what is left
//! fills the shape wanted.

use hearth_math::hash::Rng;

/// The shapes knapping aims for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aim {
    /// A cobble with one edge knocked sharp.
    Chopper,
    /// A teardrop worked on both faces.
    HandAxe,
    /// A triangular point.
    Point,
    /// A flake with a rounded working edge.
    Scraper,
}

impl Aim {
    /// The shape a knapping process aims for, by its content id.
    pub fn of_process(id: &str) -> Option<Aim> {
        match hearth_content::triggers::key(id) {
            "make_chopper" => Some(Aim::Chopper),
            "make_hand_axe" => Some(Aim::HandAxe),
            "knap_point" => Some(Aim::Point),
            "make_scraper" => Some(Aim::Scraper),
            _ => None,
        }
    }
}

/// What a strike did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strike {
    /// Cells taken off.
    pub removed: usize,
    /// The piece snapped.
    pub snapped: bool,
}

/// A stone being knapped.
#[derive(Debug, Clone)]
pub struct Knap {
    pub w: usize,
    pub h: usize,
    /// Cells of stone left.
    pub stone: Vec<bool>,
    /// Cells of the shape wanted.
    pub target: Vec<bool>,
    /// 0–1: how predictably flakes come off (the material's `knapping`).
    pub predictability: f32,
    pub strikes: u32,
    pub max_strikes: u32,
    pub snapped: bool,
    rng: Rng,
}

impl Knap {
    /// The shape a knapping process aims for, if it is shaped by hand.
    pub fn shape_of(process: &str) -> Option<Aim> {
        Aim::of_process(process)
    }

    pub const W: usize = 28;
    pub const H: usize = 20;

    pub fn new(aim: Aim, predictability: f32, seed: u64) -> Self {
        let (w, h) = (Self::W, Self::H);
        let mut rng = Rng::new(seed);
        // The blank: a rounded cobble, a little irregular.
        let wobble: Vec<f32> = (0..12).map(|_| rng.range_f32(0.88, 1.08)).collect();
        let mut stone = vec![false; w * h];
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        for y in 0..h {
            for x in 0..w {
                let dx = (x as f32 + 0.5 - cx) / (w as f32 * 0.47);
                let dy = (y as f32 + 0.5 - cy) / (h as f32 * 0.47);
                let a = dy.atan2(dx).rem_euclid(std::f32::consts::TAU);
                let k = a / std::f32::consts::TAU * 12.0;
                let (i, f) = (k as usize % 12, k.fract());
                let r = wobble[i] * (1.0 - f) + wobble[(i + 1) % 12] * f;
                stone[y * w + x] = dx * dx + dy * dy <= r * r;
            }
        }
        let target = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
                let (u, v) = (x / w as f32, y / h as f32);
                match aim {
                    // Keep the cobble, take one side down to a straight edge.
                    Aim::Chopper => v > 0.18 && stone[i],
                    // A teardrop: broad butt below, tapering tip above.
                    Aim::HandAxe => {
                        let half =
                            0.36 * (1.0 - (1.0 - v).powf(1.6)).max(0.0) * (v * 1.15).min(1.0);
                        (u - 0.5).abs() < half && v > 0.08 && v < 0.94
                    }
                    Aim::Point => {
                        let half = 0.3 * v;
                        (u - 0.5).abs() < half && v > 0.12 && v < 0.88
                    }
                    Aim::Scraper => {
                        let dx = (u - 0.5) / 0.32;
                        let dy = (v - 0.55) / 0.36;
                        dx * dx + dy * dy < 1.0 && v > 0.3
                    }
                }
            })
            .collect();
        let max_strikes = match aim {
            Aim::Chopper => 6,
            Aim::HandAxe => 22,
            Aim::Point => 14,
            Aim::Scraper => 10,
        };
        Self {
            w,
            h,
            stone,
            target,
            predictability: predictability.clamp(0.05, 1.0),
            strikes: 0,
            max_strikes,
            snapped: false,
            rng,
        }
    }

    pub fn at(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.w
            && (y as usize) < self.h
            && self.stone[y as usize * self.w + x as usize]
    }

    /// Cells of stone left.
    pub fn left(&self) -> usize {
        self.stone.iter().filter(|s| **s).count()
    }

    /// How well what is left fills the shape wanted (0–1): the share of the two that overlap.
    pub fn fit(&self) -> f32 {
        let both = self
            .stone
            .iter()
            .zip(&self.target)
            .filter(|(s, t)| **s && **t)
            .count();
        let either = self
            .stone
            .iter()
            .zip(&self.target)
            .filter(|(s, t)| **s || **t)
            .count();
        both as f32 / either.max(1) as f32
    }

    /// The quality of the piece if it were finished now (0–1).
    pub fn quality(&self) -> f32 {
        if self.snapped {
            return 0.0;
        }
        ((self.fit() - 0.45) / 0.45).clamp(0.0, 1.0)
    }

    pub fn finished(&self) -> bool {
        self.snapped || self.strikes >= self.max_strikes
    }

    /// Strikes at `(x, y)` (cells) driving the flake along `dir` with force 0–1.
    pub fn strike(&mut self, at: (f32, f32), dir: (f32, f32), force: f32) -> Strike {
        if self.finished() {
            return Strike {
                removed: 0,
                snapped: self.snapped,
            };
        }
        self.strikes += 1;
        let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(1e-4);
        let d = (dir.0 / len, dir.1 / len);
        // The flake's run: longer with force; coarse stone stops short (a step) or runs on.
        let wild = 1.0 - self.predictability;
        let mut run = 2.0 + force.clamp(0.0, 1.0) * 7.0;
        if self.rng.next_f32() < wild * 0.5 {
            run *= if self.rng.next_f32() < 0.5 { 0.35 } else { 1.6 };
        }
        run *= 1.0 + (self.rng.next_f32() - 0.5) * wild;
        let spread = (0.45 + (self.rng.next_f32() - 0.5) * wild).max(0.15);
        let before = self.stone.clone();
        let mut removed = 0;
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y * self.w + x;
                if !self.stone[i] {
                    continue;
                }
                let (vx, vy) = (x as f32 + 0.5 - at.0, y as f32 + 0.5 - at.1);
                let along = vx * d.0 + vy * d.1;
                let across = (vx * d.1 - vy * d.0).abs();
                if along > -0.6 && along < run && across < 0.9 + along.max(0.0) * spread {
                    self.stone[i] = false;
                    removed += 1;
                }
            }
        }
        // Cut in two: what is left is the larger part; if that is not most of the piece, it
        // snapped.
        let parts = self.parts();
        if parts.len() > 1 {
            let total: usize = parts.iter().map(|p| p.len()).sum();
            let biggest = parts
                .iter()
                .max_by_key(|p| p.len())
                .cloned()
                .unwrap_or_default();
            for (i, s) in self.stone.iter_mut().enumerate() {
                *s = *s && biggest.contains(&i);
            }
            if (biggest.len() as f32) < 0.75 * total as f32 {
                self.snapped = true;
            }
        }
        // A hard blow on a thin piece of coarse stone may snap it.
        let thin = self.left() < before.iter().filter(|s| **s).count() / 3;
        if thin && force > 0.7 && self.rng.next_f32() < 0.4 * wild {
            self.snapped = true;
        }
        Strike {
            removed,
            snapped: self.snapped,
        }
    }

    /// The connected parts of the stone (cells by index).
    fn parts(&self) -> Vec<Vec<usize>> {
        let mut seen = vec![false; self.stone.len()];
        let mut out = Vec::new();
        for start in 0..self.stone.len() {
            if !self.stone[start] || seen[start] {
                continue;
            }
            let mut part = Vec::new();
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(i) = stack.pop() {
                part.push(i);
                let (x, y) = ((i % self.w) as i32, (i / self.w) as i32);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if self.at(nx, ny) {
                        let j = ny as usize * self.w + nx as usize;
                        if !seen[j] {
                            seen[j] = true;
                            stack.push(j);
                        }
                    }
                }
            }
            out.push(part);
        }
        out
    }

    /// The edge cell nearest a point, and the way into the stone from it.
    pub fn edge_near(&self, p: (f32, f32)) -> Option<((f32, f32), (f32, f32))> {
        let mut best: Option<(f32, (i32, i32))> = None;
        for y in 0..self.h as i32 {
            for x in 0..self.w as i32 {
                if !self.at(x, y) {
                    continue;
                }
                let edge = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(dx, dy)| !self.at(x + dx, y + dy));
                if !edge {
                    continue;
                }
                let d = (x as f32 + 0.5 - p.0).powi(2) + (y as f32 + 0.5 - p.1).powi(2);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, (x, y)));
                }
            }
        }
        let (_, (x, y)) = best?;
        // Inward: square to the edge there, towards the stone close by.
        let (mut sx, mut sy) = (0.0f32, 0.0f32);
        for dy in -3..=3 {
            for dx in -3..=3 {
                if (dx, dy) != (0, 0) && dx * dx + dy * dy <= 9 && self.at(x + dx, y + dy) {
                    sx += dx as f32;
                    sy += dy as f32;
                }
            }
        }
        let e = (x as f32 + 0.5, y as f32 + 0.5);
        let l = (sx * sx + sy * sy).sqrt();
        let inward = if l > 1e-3 {
            (sx / l, sy / l)
        } else {
            (0.0, 1.0)
        };
        Some((e, inward))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Strikes all round the outside of the target, inward, as a careful knapper would.
    fn careful(k: &mut Knap) {
        let (w, h) = (k.w as f32, k.h as f32);
        let mut t = 0.0f32;
        while !k.finished() {
            // Aim at the stone's edge where it bulges most past the target.
            let mut best: Option<(usize, (f32, f32))> = None;
            for y in 0..k.h {
                for x in 0..k.w {
                    let i = y * k.w + x;
                    if k.stone[i] && !k.target[i] {
                        let excess = (0..k.h)
                            .flat_map(|yy| (0..k.w).map(move |xx| (xx, yy)))
                            .filter(|&(xx, yy)| {
                                let j = yy * k.w + xx;
                                k.stone[j]
                                    && !k.target[j]
                                    && (xx as i32 - x as i32).abs() <= 2
                                    && (yy as i32 - y as i32).abs() <= 2
                            })
                            .count();
                        if best.is_none_or(|b| excess > b.0) {
                            best = Some((excess, (x as f32 + 0.5, y as f32 + 0.5)));
                        }
                    }
                }
            }
            let Some((_, p)) = best else {
                break;
            };
            let Some((at, dir)) = k.edge_near(p) else {
                break;
            };
            // Just hard enough to take off what stands proud of the shape there.
            let mut excess = 0.0f32;
            while excess < 12.0 {
                let (x, y) = (at.0 + dir.0 * excess, at.1 + dir.1 * excess);
                let (xi, yi) = (x.floor() as i32, y.floor() as i32);
                if !k.at(xi, yi) || k.target[yi as usize * k.w + xi as usize] {
                    break;
                }
                excess += 0.5;
            }
            let force = ((excess - 2.0) / 7.0).clamp(0.0, 0.45);
            k.strike(at, dir, force);
            t += 1.0;
            let _ = (w, h, t);
        }
    }

    #[test]
    fn flint_takes_the_line_it_is_given_and_coarse_stone_does_not() {
        let mut fits = Vec::new();
        for (stone, p) in [("flint", 0.95), ("basalt", 0.35)] {
            let mut total = 0.0;
            let mut snaps = 0;
            for seed in 0..12 {
                let mut k = Knap::new(Aim::HandAxe, p, seed);
                let before = k.fit();
                careful(&mut k);
                assert!(k.strikes > 0);
                if k.snapped {
                    snaps += 1;
                } else if p > 0.9 {
                    assert!(k.fit() >= before - 0.05, "{stone}: worse than the blank");
                }
                total += k.quality();
            }
            fits.push((stone, total / 12.0, snaps));
        }
        let (flint, basalt) = (fits[0].1, fits[1].1);
        assert!(flint > basalt, "flint knaps better: {fits:?}");
        assert!(flint > 0.3, "a careful flint hand axe is decent: {fits:?}");
    }

    #[test]
    fn blows_driven_through_the_middle_snap_the_piece() {
        let mut k = Knap::new(Aim::HandAxe, 0.95, 1);
        let x = k.w as i32 / 2;
        for _ in 0..6 {
            if k.snapped {
                break;
            }
            // Down into the bottom of the groove being cut.
            let Some(y) = (0..k.h as i32).find(|&y| k.at(x, y)) else {
                break;
            };
            k.strike((x as f32 + 0.5, y as f32 + 0.5), (0.0, 1.0), 1.0);
        }
        assert!(k.snapped, "cut through the middle: {} cells left", k.left());
        assert_eq!(k.quality(), 0.0);
    }
}
