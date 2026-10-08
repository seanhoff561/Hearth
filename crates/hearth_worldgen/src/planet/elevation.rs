//! The macro elevation model (real-world metres). Every landform's horizontal footprint is
//! widened by the slope rule so that on small, vertically exaggerated planets mountains become
//! wider rather than spikier.

use glam::DVec3;

use crate::noise::SphereFbm;

/// Per-planet scale constants for converting real relief into footprints.
#[derive(Debug, Clone, Copy)]
pub struct Scale {
    /// Planet radius in blocks.
    pub radius_blocks: f64,
    /// Blocks per real metre of relief.
    pub vertical: f64,
    /// Horizontal scale vs Earth (C / C_earth).
    pub horizontal: f64,
}

impl Scale {
    /// Angular half-width (radians) of a landform with real nominal half-width `nominal_km`
    /// and relief `relief_m`, such that its average flank slope in blocks never exceeds
    /// `max_slope`.
    #[inline]
    pub fn footprint(&self, nominal_km: f64, relief_m: f64, max_slope: f64) -> f64 {
        let nominal_rad = nominal_km / 6371.0;
        let slope_rad = relief_m.abs() * self.vertical / (max_slope * self.radius_blocks);
        nominal_rad.max(slope_rad)
    }
}

#[inline]
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Smooth bump: 1 at t = 0, 0 for |t| ≥ 1.
#[inline]
pub fn bump(t: f64) -> f64 {
    let a = t.abs();
    if a >= 1.0 {
        0.0
    } else {
        let s = 1.0 - a * a;
        s * s
    }
}

/// Kind of the nearest plate boundary, as seen from a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryKind {
    Convergent,
    Divergent,
    Transform,
    Inactive,
}

/// Everything the elevation model needs about one cell.
#[derive(Debug, Clone, Copy)]
pub struct CellInput {
    pub p: DVec3,
    /// Signed distance to the coast in radians (positive on land).
    pub coast: f64,
    /// Distance to the nearest plate boundary (radians).
    pub boundary_dist: f64,
    pub boundary: BoundaryKind,
    /// Normalised convergence rate at that boundary (0..1+).
    pub convergence: f64,
    pub own_continental: bool,
    pub other_continental: bool,
    /// True if this cell's plate rides over the other (subduction) or gets the plateau
    /// (collision).
    pub overriding: bool,
    /// Normalised oceanic crust age (0 at ridges, 1 old).
    pub crust_age: f64,
    /// Activity (0..1) of the margin at the nearest coast (1 = subduction margin).
    pub margin_activity: f64,
    /// Distance (radians) to the nearest ancient suture inside a continent.
    pub suture_dist: f64,
    /// Contribution from hotspot volcanoes (metres of edifice height above the sea floor).
    pub hotspot: f64,
}

/// Noise sources for the elevation model.
#[derive(Debug, Clone)]
pub struct ElevationNoise {
    pub regional: SphereFbm,
    pub shield: SphereFbm,
    pub ridges: SphereFbm,
    pub strength: SphereFbm,
    pub abyssal: SphereFbm,
    pub arc_cones: SphereFbm,
    pub scour: SphereFbm,
}

impl ElevationNoise {
    pub fn new(seed: u64, scale: &Scale) -> Self {
        use hearth_math::hash::derive_seed as d;
        // Mountain ridge spacing follows the relief so slopes stay believable:
        // wavelength ≈ 3 × relief in blocks (relief ~1500 m).
        let ridge_wavelength_blocks = (3.0 * 1500.0 * scale.vertical).max(300.0);
        let ridge_freq = scale.radius_blocks / ridge_wavelength_blocks;
        Self {
            regional: SphereFbm::new(d(seed, "regional"), 5.0, 5, 0.5, 2.0),
            shield: SphereFbm::new(d(seed, "shield"), 2.5, 3, 0.5, 2.0),
            ridges: SphereFbm::new(d(seed, "ridges"), ridge_freq, 6, 0.5, 2.03),
            strength: SphereFbm::new(d(seed, "strength"), 3.0, 3, 0.5, 2.0),
            abyssal: SphereFbm::new(d(seed, "abyssal"), 30.0, 4, 0.5, 2.0),
            arc_cones: SphereFbm::new(d(seed, "arc"), 40.0, 2, 0.4, 2.0),
            scour: SphereFbm::new(d(seed, "scour"), 60.0, 2, 0.5, 2.0),
        }
    }
}

/// Output of the elevation model for one cell, split into components so the boundary-driven
/// parts can be smoothed before they are combined (removing seams where the nearest plate
/// boundary switches).
#[derive(Debug, Clone, Copy, Default)]
pub struct CellElevation {
    /// Coastal plains / shelf / slope / abyss including regional variation (metres).
    pub base: f64,
    /// Signed non-mountain tectonic relief: trenches, arcs, rifts, fault valleys (metres).
    pub tectonic: f64,
    /// Mountain envelope height (metres): collision and Andean ranges, plateaus, old ranges.
    pub uplift: f64,
    /// 1 inside a high plateau.
    pub plateau: f64,
    /// Rift depression strength (0..1).
    pub rift: f64,
    /// Local terms applied after smoothing: hotspot edifices and glacial scour (metres).
    pub local: f64,
}

/// Parameters derived from settings.
#[derive(Debug, Clone, Copy)]
pub struct ElevationParams {
    pub scale: Scale,
}

impl ElevationParams {
    /// How strong a collision must be to raise a great range or plateau: rare, as on Earth
    /// (the Himalaya and Tibet, the Andes).
    fn great_threshold(&self) -> f64 {
        0.78
    }
}

/// Envelope slope limits (blocks per block): great ranges may be steep, plateaus' edges and
/// old ranges gentle.
const RANGE_SLOPE: f64 = 0.8;
const PLATEAU_EDGE_SLOPE: f64 = 0.45;

/// Computes the components of the macro elevation of one cell.
pub fn compose(c: &CellInput, params: &ElevationParams, nz: &ElevationNoise) -> CellElevation {
    let s = &params.scale;
    let p = c.p;
    let land = c.coast > 0.0;
    let mut out = CellElevation::default();

    // ---------------------------------------------------------------- base surface
    let regional = nz.regional.sample(p);
    let shieldness = smoothstep(0.1, 0.5, nz.shield.sample(p));
    out.base = if land {
        let interior_w = s.footprint(400.0, 300.0, 0.02);
        let interior = smoothstep(0.0, interior_w, c.coast);
        // Low coastal plains rising gently inland; shields are flatter and a little higher.
        let base = 8.0 + 220.0 * interior;
        let relief = 180.0 * (1.0 - 0.6 * shieldness);
        base + regional * relief * smoothstep(0.0, interior_w * 0.3, c.coast)
            + 120.0 * shieldness * interior
    } else {
        let d = -c.coast;
        // Shelf widths: ~500 blocks on passive margins, ~100 on active ones (Standard).
        let shelf_w = lerp(
            s.footprint(80.0, 200.0, 0.1),
            s.footprint(15.0, 200.0, 0.4),
            c.margin_activity,
        );
        let abyss =
            -3300.0 - 1100.0 * c.crust_age.clamp(0.0, 1.0).sqrt() + nz.abyssal.sample(p) * 220.0;
        let slope_w = s.footprint(60.0, 3400.0, 1.3);
        if d < shelf_w {
            let t = d / shelf_w;
            -4.0 - 196.0 * t.powf(1.4)
        } else if d < shelf_w + slope_w {
            let t = (d - shelf_w) / slope_w;
            lerp(-200.0, abyss, smoothstep(0.0, 1.0, t))
        } else {
            abyss
        }
    };

    // ---------------------------------------------------------------- plate boundaries
    let strength_noise = 0.5 + 0.5 * nz.strength.sample(p);
    let strength = (c.convergence * (0.55 + 0.9 * strength_noise)).clamp(0.0, 1.4);
    let great = smoothstep(
        params.great_threshold(),
        params.great_threshold() + 0.25,
        strength,
    );
    let mut uplift = 0.0;
    let mut tectonic = 0.0;
    match c.boundary {
        BoundaryKind::Convergent => {
            if c.own_continental && c.other_continental {
                // Continental collision: a range centred on the suture, with a high plateau
                // behind it on the overriding side for the strongest collisions.
                let peak = lerp(2600.0, 4800.0, great) * strength.clamp(0.45, 1.0);
                let hw = s.footprint(150.0, peak, RANGE_SLOPE);
                uplift += peak * bump(c.boundary_dist / hw);
                if c.overriding && great > 0.05 {
                    let plateau_h = 2600.0 * great;
                    let flat = hw * 0.6 + s.footprint(450.0, 0.0, 1.0);
                    let edge = s.footprint(150.0, plateau_h, PLATEAU_EDGE_SLOPE);
                    let t = 1.0 - smoothstep(flat, flat + edge, c.boundary_dist);
                    out.plateau = t * great;
                    uplift = uplift.max(plateau_h * t);
                }
            } else if c.own_continental && !c.other_continental {
                // Andean margin: coastal range inland of the trench.
                let peak = lerp(2300.0, 4400.0, great) * strength.clamp(0.4, 1.0);
                let hw = s.footprint(110.0, peak, RANGE_SLOPE);
                let crest = hw * 0.9;
                // Only on the continent: a range never rises out of the open sea.
                let onshore = smoothstep(-0.015, 0.01, c.coast);
                uplift += peak * bump((c.boundary_dist - crest) / hw) * onshore;
            } else if !c.own_continental && c.other_continental {
                // Subducting oceanic plate: trench at the boundary.
                let depth = lerp(4500.0, 7800.0, great) * strength.clamp(0.4, 1.0);
                let tw = s.footprint(60.0, depth, 1.5);
                tectonic -= depth * bump(c.boundary_dist / tw);
            } else if c.overriding {
                // Ocean-ocean: a mostly submarine ridge; only discrete volcanic cones break
                // the surface.
                let arc_h = lerp(2800.0, 4600.0, great) * strength.clamp(0.5, 1.0);
                let hw = s.footprint(60.0, arc_h, 0.9);
                let crest = hw * 1.3;
                let cones = smoothstep(0.25, 0.75, nz.arc_cones.sample(p)).powf(1.5);
                tectonic += arc_h * bump((c.boundary_dist - crest) / hw) * (0.55 + 0.6 * cones);
            } else {
                let depth = lerp(4800.0, 8200.0, great) * strength.clamp(0.4, 1.0);
                let tw = s.footprint(60.0, depth, 1.5);
                tectonic -= depth * bump(c.boundary_dist / tw);
            }
        }
        BoundaryKind::Divergent => {
            if land {
                // Continental rift: graben with raised shoulders; long lakes form in it.
                let depth = 900.0 * strength.clamp(0.4, 1.0);
                let rw = s.footprint(40.0, depth, 0.45);
                let graben = bump(c.boundary_dist / rw);
                out.rift = graben;
                tectonic -= depth * graben;
                tectonic += 700.0 * bump((c.boundary_dist - rw * 1.6) / rw);
            } else {
                // Axial valley on the ridge crest.
                let rw = s.footprint(15.0, 400.0, 0.8);
                tectonic -= 400.0 * bump(c.boundary_dist / rw);
                tectonic += 500.0 * bump((c.boundary_dist - rw * 1.5) / rw);
            }
        }
        BoundaryKind::Transform => {
            let w = s.footprint(12.0, 250.0, 0.4);
            tectonic -= 250.0 * bump(c.boundary_dist / w);
        }
        BoundaryKind::Inactive => {}
    }

    // ---------------------------------------------------------------- old interiors
    if land {
        // Ancient sutures: rounded, eroded ranges (Appalachian-like).
        let old_h = 900.0 + 500.0 * strength_noise;
        let ow = s.footprint(90.0, old_h, 0.35);
        let old = old_h * bump(c.suture_dist / ow) * smoothstep(0.0, ow, c.coast);
        uplift = uplift.max(old);
        // Glacially scoured shields: many small basins (lakes) at high latitudes.
        let lat = p.y.asin().abs();
        let scour = smoothstep(0.75, 1.0, lat) * shieldness;
        if scour > 0.0 {
            out.local -= scour * 45.0 * smoothstep(0.0, 0.7, nz.scour.sample(p));
        }
    }
    if c.hotspot > 0.0 && !land {
        // Volcanic edifices rise from the sea floor.
        out.local += c.hotspot;
    }
    out.uplift = uplift;
    out.tectonic = tectonic;
    out
}

/// Combines (smoothed) components into the final elevation, adding ridged relief to
/// mountains: peaks rise above the envelope and valleys cut below it, while plateaus stay
/// comparatively flat.
pub fn finish(c: &CellElevation, p: DVec3, nz: &ElevationNoise) -> f64 {
    let mut e = c.base + c.tectonic + c.local;
    if c.uplift > 0.0 {
        let r = nz.ridges.ridged(p);
        let rugged = 0.5 + 0.75 * r;
        let flat = 0.92 + 0.15 * r;
        e += c.uplift * lerp(rugged, flat, c.plateau.clamp(0.0, 1.0));
    }
    e
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn std_scale() -> Scale {
        Scale {
            radius_blocks: 65536.0 / std::f64::consts::TAU,
            vertical: 0.25,
            horizontal: 65536.0 / 40_075_264.0,
        }
    }

    #[test]
    fn slope_rule_widens_small_planets() {
        let standard = std_scale();
        let tiny = Scale {
            radius_blocks: 16384.0 / std::f64::consts::TAU,
            vertical: 0.125,
            horizontal: 16384.0 / 40_075_264.0,
        };
        let earth = Scale {
            radius_blocks: 6_371_000.0,
            vertical: 1.0,
            horizontal: 1.0,
        };
        let w_std = standard.footprint(150.0, 5000.0, 0.6);
        let w_tiny = tiny.footprint(150.0, 5000.0, 0.6);
        let w_earth = earth.footprint(150.0, 5000.0, 0.6);
        assert!(w_tiny > w_std, "relatively wider on the small planet");
        assert!(
            (w_earth - 150.0 / 6371.0).abs() < 1e-12,
            "nominal width at 1:1"
        );
        // Average slope in blocks never exceeds the limit.
        let slope = 5000.0 * standard.vertical / (w_std * standard.radius_blocks);
        assert!(slope <= 0.6 + 1e-9);
    }

    #[test]
    fn ocean_profile_is_monotone_offshore() {
        let scale = std_scale();
        let params = ElevationParams { scale };
        let nz = ElevationNoise::new(1, &scale);
        let mut last = 1.0e9;
        for k in 0..200 {
            let c = CellInput {
                p: DVec3::X,
                coast: -(k as f64) * 0.002,
                boundary_dist: 1.0,
                boundary: BoundaryKind::Inactive,
                convergence: 0.0,
                own_continental: false,
                other_continental: false,
                overriding: false,
                crust_age: 1.0,
                margin_activity: 0.0,
                suture_dist: 1.0,
                hotspot: 0.0,
            };
            let e = finish(&compose(&c, &params, &nz), c.p, &nz);
            assert!(e <= last + 1e-6, "k={k} e={e} last={last}");
            last = e;
        }
        assert!(last < -3000.0, "abyss reached: {last}");
    }
}
