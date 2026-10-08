//! The distant ground and canopy as smooth height fields (S §5, D274, D276) and the seasons of
//! their snow and ice (D277).

use bytemuck::{Pod, Zeroable};

use crate::{Col, TILE, TINT_EVERGREEN, pack, to_linear, to_srgb};

/// The ground material slot of ground that is no natural block (`BlockRegistry::ground_slots`).
pub const NO_SLOT: u8 = u8::MAX;
/// Fixed-point heights: parts of a block (S §5).
pub const FIX: i32 = 16;
/// Corners along a tile side of its ground's height field.
pub const GROUND_SIDE: usize = TILE as usize + 1;

/// One corner of a tile's ground in 20 bytes; the vertex shader makes the grid's triangles (and
/// the skirts along its edges) of them. Packed:
/// * `y`: the surface's height, fixed-point (`FIX`), absolute;
/// * `n`: the height field's normal, X and Z as signed 12-bit fractions (Y is up, the rest),
///   and the tint kind (8);
/// * `c`: sRGB colour (24) and ground material slot (8; `NO_SLOT` where the ground is no
///   natural block, coloured as its block's texture);
/// * `m`: climate (24), of the four columns about the corner how many are water (4) and under a
///   crown (4);
/// * `s`: the seasons of snow on the ground and of ice on water (`Seasons`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct GroundVertex {
    pub y: i32,
    pub n: u32,
    pub c: u32,
    pub m: u32,
    pub s: u32,
}

impl GroundVertex {
    pub fn height(&self) -> f32 {
        self.y as f32 / FIX as f32
    }

    pub fn normal(&self) -> [f32; 3] {
        let s12 = |v: u32| ((v << 20) as i32 >> 20) as f32 / 2047.0;
        let (x, z) = (s12(self.n), s12(self.n >> 12));
        [x, (1.0 - x * x - z * z).max(0.0).sqrt(), z]
    }

    pub fn rgb(&self) -> u32 {
        self.c & 0xff_ffff
    }

    pub fn slot(&self) -> u8 {
        (self.c >> 24) as u8
    }

    pub fn kind(&self) -> u8 {
        (self.n >> 24) as u8
    }

    pub fn climate(&self) -> u32 {
        self.m & 0xff_ffff
    }

    /// Shares of the four columns about the corner that are water and under a crown.
    pub fn water(&self) -> f32 {
        (self.m >> 24 & 15) as f32 / 4.0
    }

    pub fn shade(&self) -> f32 {
        (self.m >> 28) as f32 / 4.0
    }

    pub fn seasons(&self) -> Seasons {
        Seasons(self.s)
    }
}

/// When snow lies on a place's ground and ice on its water through the year, as the seasonal
/// cover of the near terrain lays them (`hearth_env::climate::SeasonalCover`): the year
/// fractions (in 256ths) it comes and goes, snow's in the low half, ice's in the high; a span
/// from 0 to 255 lasts all year, one from 0 to 0 never comes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Seasons(pub u32);

/// What `LodGen::seasons_of` tells climates apart by: latitude (half degrees), mean
/// temperature (quarter degrees), range (half degrees), precipitation (20 mm), dry seasons
/// (tenths, winter's in the high byte), and whether it is the sea.
pub(crate) type SeasonKey = (i32, i32, i32, i32, i32, bool);

/// Snow lies as the near terrain's first layer from this depth (m; half a layer of 0.125 m);
/// ice shows from this thickness (m).
const SNOW_SHOWS_M: f64 = 0.0625;
const ICE_SHOWS_M: f64 = 0.02;

impl Seasons {
    /// The longest span of the year over which `f` (sampled at the cover model's steps) passes
    /// `limit`, in 256ths of the year.
    fn span(f: impl Fn(f64) -> f64, limit: f64) -> (u8, u8) {
        let steps = hearth_env::climate::STEPS;
        let on: Vec<bool> = (0..steps)
            .map(|k| f((k as f64 + 0.5) / steps as f64) > limit)
            .collect();
        if on.iter().all(|&b| b) {
            return (0, 255);
        }
        // The longest run, around the year's end too.
        let (mut best, mut start) = ((0, 0), None);
        for k in 0..2 * steps {
            match (on[k % steps], start) {
                (true, None) => start = Some(k),
                (false, Some(a)) => {
                    if k - a > best.1 - best.0 {
                        best = (a, k);
                    }
                    start = None;
                }
                _ => {}
            }
        }
        if best.1 == best.0 {
            return (0, 0);
        }
        let frac = |k: usize| (((k % steps) as f64 / steps as f64) * 256.0).round() as u32;
        let on = frac(best.0).min(255) as u8;
        let off = frac(best.1).min(255) as u8;
        (on, if off == on { on.wrapping_add(1) } else { off })
    }

    /// The seasons of a place's cover; `sea` for sea ice (else the ice of still water).
    pub fn of(cover: &hearth_env::climate::SeasonalCover, sea: bool) -> Self {
        let snow = Self::span(|t| cover.snow_depth_m(t), SNOW_SHOWS_M);
        let ice = if sea {
            Self::span(|t| cover.sea_ice_m(t), ICE_SHOWS_M)
        } else {
            Self::span(|t| cover.ice_m(t), ICE_SHOWS_M)
        };
        Self(snow.0 as u32 | (snow.1 as u32) << 8 | (ice.0 as u32) << 16 | (ice.1 as u32) << 24)
    }

    /// Whether snow lies, or ice, at a year fraction.
    pub fn snow(&self, year_frac: f64) -> bool {
        Self::inside(self.0 as u8, (self.0 >> 8) as u8, year_frac)
    }

    pub fn ice(&self, year_frac: f64) -> bool {
        Self::inside((self.0 >> 16) as u8, (self.0 >> 24) as u8, year_frac)
    }

    fn inside(on: u8, off: u8, year_frac: f64) -> bool {
        match (on, off) {
            (0, 0) => false,
            (0, 255) => true,
            _ => {
                let since = (year_frac - on as f64 / 256.0).rem_euclid(1.0);
                since < ((off as f64 - on as f64) / 256.0).rem_euclid(1.0)
            }
        }
    }
}

/// The ground of a tile from its columns (with their ring of neighbours): a height field whose
/// corners lie at the mean of the four columns about them (the surface through the columns'
/// middles, interpolated), each with the normal of the field there, the mean colour of the
/// columns of its kind, and how many of them are water and under a crown. Also the tile's
/// error (how far the field strays from a column's own height, blocks), its skirt (blocks) and
/// its height range (fixed-point).
pub(crate) fn ground(cs: i32, cols: &[Col]) -> (Vec<GroundVertex>, f32, f32, (i32, i32)) {
    let n = TILE + 2;
    let at = |i: i32, j: i32| &cols[((j + 1) * n + (i + 1)) as usize];
    let side = GROUND_SIDE as i32;
    let mut out = Vec::with_capacity(GROUND_SIDE * GROUND_SIDE);
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    for j in 0..side {
        for i in 0..side {
            let four = [at(i - 1, j - 1), at(i, j - 1), at(i - 1, j), at(i, j)];
            let y = (four.iter().map(|c| c.h).sum::<i32>() as f32 / 4.0).round() as i32;
            lo = lo.min(y);
            hi = hi.max(y);
            // The field's slope across the corner, from the columns on either side.
            let scale = 1.0 / (2 * cs * FIX) as f32;
            let dx = ((four[1].h + four[3].h) - (four[0].h + four[2].h)) as f32 * scale;
            let dz = ((four[2].h + four[3].h) - (four[0].h + four[1].h)) as f32 * scale;
            let len = (dx * dx + 1.0 + dz * dz).sqrt();
            let snorm = |v: f32| ((-v / len * 2047.0).round() as i32 & 0xfff) as u32;
            // Material, kind and climate from the column on the corner's far side (the same for
            // the tiles either side of an edge), colour the mean of the columns of that kind.
            let me = four[3];
            let mut sum = [0.0f32; 3];
            let mut k = 0.0;
            for c in four
                .iter()
                .filter(|c| c.kind == me.kind && c.slot == me.slot)
            {
                for (ch, s) in sum.iter_mut().enumerate() {
                    *s += to_linear((c.rgb >> (8 * ch)) as u8);
                }
                k += 1.0;
            }
            let rgb = pack(sum.map(|s| to_srgb(s / k)));
            let water = four.iter().filter(|c| c.water).count() as u32;
            let shade = four.iter().filter(|c| c.crown.is_some()).count() as u32;
            out.push(GroundVertex {
                y,
                n: snorm(dx) | snorm(dz) << 12 | (me.kind as u32) << 24,
                c: rgb | (me.slot as u32) << 24,
                m: (me.climate & 0xff_ffff) | water << 24 | shade << 28,
                s: me.seasons.0,
            });
        }
    }
    let corner = |i: i32, j: i32| out[(j * side + i) as usize].y;
    // How far a column's own height lies from the field's at its middle.
    let mut error = 0;
    for j in 0..TILE {
        for i in 0..TILE {
            let mid = corner(i, j) + corner(i + 1, j) + corner(i, j + 1) + corner(i + 1, j + 1);
            error = error.max((at(i, j).h * 4 - mid).abs());
        }
    }
    // Skirts deep enough to hide the crack against a neighbour a level coarser, whose edge runs
    // straight between every other corner of this one.
    let mut crack = 0;
    for k in 1..side - 1 {
        for (a, m, b) in [
            (corner(k - 1, 0), corner(k, 0), corner(k + 1, 0)),
            (
                corner(k - 1, side - 1),
                corner(k, side - 1),
                corner(k + 1, side - 1),
            ),
            (corner(0, k - 1), corner(0, k), corner(0, k + 1)),
            (
                corner(side - 1, k - 1),
                corner(side - 1, k),
                corner(side - 1, k + 1),
            ),
        ] {
            crack = crack.max((a - m).abs()).max((b - m).abs());
        }
    }
    let fix = FIX as f32;
    let skirt = (2 * cs) as f32 + crack as f32 / fix;
    (out, error as f32 / (4.0 * fix), skirt, (lo, hi))
}

/// How high a crown stands at a column this many columns from its edge (1 at the edge), as a
/// share of its depth from bottom to top: broadleaf crowns rounded, conifers' pointed.
fn crown_profile(kind: u8, d: i32) -> f32 {
    let (edge, next) = if kind & 3 == TINT_EVERGREEN {
        (0.45, 0.8)
    } else {
        (0.75, 0.95)
    };
    match d {
        1 => edge,
        2 => next,
        _ => 1.0,
    }
}

/// The canopy of a tile's stands of trees as a surface (S §5, §7.2's farthest step): a height
/// field through the middles of the columns `-1..=31` (so it meets the next tile's), each at
/// its crown's top shaped by the crowns' profile toward the stand's edge, or where no crown
/// stands on the ground (the shader cuts the surface where the cover falls below a half, so a
/// stand's edge slopes down to half its height); leaf colour, tint kind and climate per column.
/// Empty where no crown stands; also its height range (fixed-point).
pub(crate) fn canopy_field(cs: i32, cols: &[Col]) -> (Vec<GroundVertex>, (i32, i32)) {
    let n = TILE + 2;
    let at = |i: i32, j: i32| {
        let (i, j) = (i.clamp(-1, TILE), j.clamp(-1, TILE));
        &cols[((j + 1) * n + (i + 1)) as usize]
    };
    let side = GROUND_SIDE as i32;
    if !(-1..TILE)
        .flat_map(|j| (-1..TILE).map(move |i| (i, j)))
        .any(|(i, j)| at(i, j).crown.is_some())
    {
        return (Vec::new(), (0, 0));
    }
    // Columns from the crown's edge (the ring's own neighbours count as crowned).
    let depth = |i: i32, j: i32| {
        for d in 1..3 {
            for (di, dj) in [(d, 0), (-d, 0), (0, d), (0, -d)] {
                let (x, y) = (i + di, j + dj);
                let inside = (-1..=TILE).contains(&x) && (-1..=TILE).contains(&y);
                if inside && at(x, y).crown.is_none() {
                    return d;
                }
            }
        }
        3
    };
    let height = |i: i32, j: i32| -> i32 {
        let c = at(i, j);
        match c.crown {
            Some(cr) => ((cr.bottom as f32
                + (cr.top - cr.bottom) as f32 * crown_profile(cr.kind, depth(i, j)))
                * FIX as f32)
                .round() as i32,
            None => c.h,
        }
    };
    let mut out = Vec::with_capacity(GROUND_SIDE * GROUND_SIDE);
    let (mut lo, mut hi) = (i32::MAX, i32::MIN);
    for j in -1..side - 1 {
        for i in -1..side - 1 {
            let c = at(i, j);
            let y = height(i, j);
            lo = lo.min(y);
            hi = hi.max(y);
            let scale = 1.0 / (2 * cs * FIX) as f32;
            let dx = (height(i + 1, j) - height(i - 1, j)) as f32 * scale;
            let dz = (height(i, j + 1) - height(i, j - 1)) as f32 * scale;
            let len = (dx * dx + 1.0 + dz * dz).sqrt();
            let snorm = |v: f32| ((-v / len * 2047.0).round() as i32 & 0xfff) as u32;
            let (rgb, kind, cover) = match c.crown {
                Some(cr) => (cr.rgb, cr.kind, 4u32),
                None => {
                    // The colour of a crown beside, for the edge the surface fades to.
                    let near = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .find_map(|(di, dj)| at(i + di, j + dj).crown);
                    near.map_or((c.rgb, c.kind, 0), |cr| (cr.rgb, cr.kind, 0))
                }
            };
            out.push(GroundVertex {
                y,
                n: snorm(dx) | snorm(dz) << 12 | (kind as u32) << 24,
                c: rgb | (NO_SLOT as u32) << 24,
                m: (c.climate & 0xff_ffff) | cover << 24,
                s: c.seasons.0,
            });
        }
    }
    (out, (lo, hi))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Crown, TINT_DECIDUOUS, TINT_RGB, TileKey, mesh};

    fn flat(top: i32) -> Col {
        Col {
            top,
            h: top * FIX,
            slot: NO_SLOT,
            rgb: 0x406080,
            kind: TINT_RGB,
            water: false,
            climate: 0,
            seasons: Seasons::default(),
            crown: None,
        }
    }

    #[test]
    fn the_ground_is_a_smooth_height_field_with_skirts() {
        let key = TileKey {
            level: 1,
            x: 3,
            z: -2,
        };
        let n = (TILE + 2) as usize;
        // A slope rising 0.25 a block eastward (half a block a column), and a raised column.
        let mut cols: Vec<Col> = (0..n * n)
            .map(|k| {
                let i = (k % n) as i32 - 1;
                Col {
                    h: 10 * FIX + i * FIX / 2,
                    ..flat(10)
                }
            })
            .collect();
        cols[(17 * n) + 17].h += 4 * FIX;
        let m = mesh(key, &cols, &[]);
        assert!(m.quads.is_empty(), "no columns' tops or sides");
        assert_eq!(m.ground.len(), GROUND_SIDE * GROUND_SIDE);
        let at = |i: usize, j: usize| m.ground[j * GROUND_SIDE + i];
        // Corners midway between the columns' heights: exact on the slope.
        assert_eq!(at(0, 0).height(), 9.75);
        assert_eq!(at(1, 0).height(), 10.25);
        assert_eq!(at(32, 32).height(), 25.75);
        // The slope's normal leans west.
        let [nx, ny, nz] = at(5, 5).normal();
        let want = 1.0 / 1.0625f32.sqrt();
        assert!((nx + 0.25 * want).abs() < 1e-3 && (ny - want).abs() < 1e-3 && nz.abs() < 1e-3);
        // The raised column shares itself among its four corners: the error is what is lost.
        assert_eq!(at(16, 16).height() - at(15, 16).height(), 0.5 + 1.0);
        assert_eq!(m.error, 3.0);
        // Skirts reach two columns down, and the range covers them.
        assert_eq!(m.skirt, 2.0 * 2.0 + 0.5);
        assert_eq!(m.max_y, 26);
        assert_eq!(m.min_y, 9 - 5);
        assert_eq!(at(3, 3).rgb(), 0x406080);
        assert_eq!((at(3, 3).water(), at(3, 3).shade()), (0.0, 0.0));
    }

    #[test]
    fn far_stands_are_a_canopy_surface_over_shaded_ground() {
        let key = TileKey {
            level: 3,
            x: 0,
            z: 0,
        };
        let n = (TILE + 2) as usize;
        let mut cols = vec![flat(10); n * n];
        let oak = Crown {
            bottom: 14,
            top: 19,
            rgb: 0x205020,
            kind: TINT_DECIDUOUS,
        };
        // A broadleaf stand of 5 × 5 columns, and a lone conifer.
        for j in 10..15 {
            for i in 10..15 {
                cols[(j + 1) * n + i + 1].crown = Some(oak);
            }
        }
        let spruce = Crown {
            kind: TINT_EVERGREEN,
            ..oak
        };
        cols[(26 * n) + 26].crown = Some(spruce);
        let m = mesh(key, &cols, &[]);
        // The canopy's vertices stand at the columns' middles, from the column before the tile.
        assert_eq!(m.canopy.len(), GROUND_SIDE * GROUND_SIDE);
        let at = |i: usize, j: usize| m.canopy[(j + 1) * GROUND_SIDE + i + 1];
        // The stand: full height in its middle, rounded toward its edge, on the ground just
        // outside, where the cover is none (the shader cuts the surface halfway there).
        assert_eq!((at(12, 12).height(), at(12, 12).water()), (19.0, 1.0));
        assert_eq!(at(11, 12).height(), 14.0 + 5.0 * 0.95);
        assert_eq!(at(10, 12).height(), 14.0 + 5.0 * 0.75);
        assert_eq!((at(9, 12).height(), at(9, 12).water()), (10.0, 0.0));
        assert_eq!(
            at(9, 12).rgb(),
            0x205020,
            "the edge takes the crown's colour"
        );
        // The lone conifer is pointed: lower at its edge than a broadleaf crown.
        assert_eq!(at(25, 25).height(), 14.0 + 5.0 * 0.45);
        assert_eq!(at(25, 25).kind() & 3, TINT_EVERGREEN);
        // Far from any crown the surface lies on the ground, with no cover.
        assert_eq!((at(3, 3).height(), at(3, 3).water()), (10.0, 0.0));
        assert_eq!(m.max_y, 20);
        // The ground under the stand is shaded; no boxes.
        let shade = |i: usize, j: usize| m.ground[j * GROUND_SIDE + i].shade();
        assert_eq!((shade(12, 12), shade(10, 12), shade(5, 5)), (1.0, 0.5, 0.0));
        assert!(m.quads.is_empty());
        // No crown: no canopy.
        assert!(mesh(key, &vec![flat(10); n * n], &[]).canopy.is_empty());
    }

    #[test]
    fn snow_and_ice_come_and_go_with_the_cover_models_seasons() {
        use hearth_env::climate::{Normals, SeasonalCover};
        // A cold continental place: snow through the winter, frozen lakes.
        let cold = SeasonalCover::compute(&Normals::new(60.0, -2.0, 30.0, 600.0, 0.0, 0.0));
        let s = Seasons::of(&cold, false);
        for k in 0..73 {
            let t = (k as f64 + 0.5) / 73.0;
            if (cold.snow_depth_m(t) - SNOW_SHOWS_M).abs() > 0.03 {
                assert_eq!(
                    s.snow(t),
                    cold.snow_depth_m(t) > SNOW_SHOWS_M,
                    "snow at {t}"
                );
            }
        }
        // (The year's fractions here run from the March equinox: deep winter about 0.8.)
        assert!(s.snow(0.8) && !s.snow(0.3) && s.ice(0.8) && !s.ice(0.3));
        // A warm one: never.
        let warm = SeasonalCover::compute(&Normals::new(10.0, 26.0, 4.0, 1500.0, 0.0, 0.0));
        let s = Seasons::of(&warm, true);
        assert!((0..20).all(|k| !s.snow(k as f64 / 20.0) && !s.ice(k as f64 / 20.0)));
        // A glacier's: always.
        let ice = SeasonalCover::compute(&Normals::new(80.0, -25.0, 30.0, 300.0, 0.0, 0.0));
        assert!((0..20).all(|k| Seasons::of(&ice, false).snow(k as f64 / 20.0)));
    }
}
