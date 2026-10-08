//! S0's material-blending and biplanar-shading prototype (S §4.1) on two materials, turf and
//! limestone: procedural albedo and height textures, mapped onto the smooth mesh by biplanar or
//! triplanar projection, blended by the vertices' weights linearly or by height.

use glam::{Vec2, Vec3};
use hearth_math::hash::{hash_3d, unit_f32};
use hearth_smooth::Mesh;

use super::raster::{Camera, ShadowMap, Surface, Visibility, light, srgb_to_linear, surface};

/// Texels along a texture's side.
const RES: usize = 256;
/// Metres a texture covers.
const TILE: f32 = 2.0;

/// A tiling texture: linear albedo and height (0..1).
pub struct Texture {
    albedo: Vec<Vec3>,
    height: Vec<f32>,
}

impl Texture {
    /// Bilinear albedo and height at world-plane coordinates.
    fn sample(&self, uv: Vec2) -> (Vec3, f32) {
        let p = uv / TILE * RES as f32 - 0.5;
        let f = p.floor();
        let t = p - f;
        let (x0, y0) = (f.x as i64, f.y as i64);
        let at = |dx: i64, dy: i64| {
            let x = (x0 + dx).rem_euclid(RES as i64) as usize;
            let y = (y0 + dy).rem_euclid(RES as i64) as usize;
            y * RES + x
        };
        let (i00, i10, i01, i11) = (at(0, 0), at(1, 0), at(0, 1), at(1, 1));
        let a = self.albedo[i00]
            .lerp(self.albedo[i10], t.x)
            .lerp(self.albedo[i01].lerp(self.albedo[i11], t.x), t.y);
        let h0 = self.height[i00] + (self.height[i10] - self.height[i00]) * t.x;
        let h1 = self.height[i01] + (self.height[i11] - self.height[i01]) * t.x;
        (a, h0 + (h1 - h0) * t.y)
    }
}

/// Tiling value noise: `period` lattice cells across the texture.
fn value(seed: u64, x: f32, y: f32, period: i32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (tx, ty) = (x - fx, y - fy);
    let s = |v: f32| v * v * v * (v * (v * 6.0 - 15.0) + 10.0);
    let (u, v) = (s(tx), s(ty));
    let corner = |dx: i32, dy: i32| {
        let cx = (fx as i32 + dx).rem_euclid(period);
        let cy = (fy as i32 + dy).rem_euclid(period);
        unit_f32(hash_3d(seed, cx, cy, 0))
    };
    let a = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * u;
    let b = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * u;
    a + (b - a) * v
}

/// Tiling fractal noise over the texture's unit square (0..1), from `period` cells up.
fn fbm(seed: u64, u: f32, v: f32, period: i32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut p) = (0.0, 1.0, 0.0, period);
    for o in 0..octaves {
        sum += amp * value(seed + o as u64, u * p as f32, v * p as f32, p);
        norm += amp;
        amp *= 0.5;
        p *= 2;
    }
    sum / norm
}

/// Tiling cellular noise: distances to the nearest and second-nearest feature points and the
/// nearest one's cell hash.
fn cells(seed: u64, u: f32, v: f32, period: i32) -> (f32, f32, u64) {
    let (x, y) = (u * period as f32, v * period as f32);
    let (cx, cy) = (x.floor() as i32, y.floor() as i32);
    let (mut f1, mut f2, mut id) = (f32::MAX, f32::MAX, 0);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (gx, gy) = (cx + dx, cy + dy);
            let h = hash_3d(seed, gx.rem_euclid(period), gy.rem_euclid(period), 1);
            let fp = Vec2::new(
                gx as f32 + unit_f32(h),
                gy as f32 + unit_f32(h.rotate_left(21)),
            );
            let d = fp.distance(Vec2::new(x, y));
            if d < f1 {
                f2 = f1;
                f1 = d;
                id = h;
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    (f1, f2, id)
}

fn generate(f: impl Fn(f32, f32) -> (Vec3, f32)) -> Texture {
    let mut albedo = Vec::with_capacity(RES * RES);
    let mut height = Vec::with_capacity(RES * RES);
    for y in 0..RES {
        for x in 0..RES {
            let (a, h) = f((x as f32 + 0.5) / RES as f32, (y as f32 + 0.5) / RES as f32);
            albedo.push(a);
            height.push(h.clamp(0.0, 1.0));
        }
    }
    Texture { albedo, height }
}

/// Turf: mottled greens with dry patches; tufts standing up between low gaps.
pub fn turf() -> Texture {
    let green = srgb_to_linear([78, 112, 46]);
    let dark = srgb_to_linear([46, 78, 32]);
    let dry = srgb_to_linear([128, 122, 66]);
    generate(|u, v| {
        let patch = fbm(901, u, v, 4, 4);
        let blades = fbm(902, u, v, 64, 2);
        let tufts = fbm(903, u, v, 16, 3);
        let base = dark
            .lerp(green, blades)
            .lerp(dry, ((patch - 0.55) * 3.0).clamp(0.0, 0.7));
        (base, 0.25 + 0.6 * tufts * (0.6 + 0.4 * blades))
    })
}

/// Limestone: pale blocks parted by dark joints, mottled, with a few lichen rosettes.
pub fn limestone() -> Texture {
    let stone = srgb_to_linear([186, 180, 164]);
    let grey = srgb_to_linear([150, 146, 136]);
    let joint = srgb_to_linear([86, 82, 76]);
    let lichen = srgb_to_linear([196, 160, 92]);
    generate(|u, v| {
        let (f1, f2, _) = cells(911, u, v, 5);
        let crack = ((f2 - f1) / 0.07).clamp(0.0, 1.0);
        let mottle = fbm(912, u, v, 8, 4);
        let (l1, _, lid) = cells(913, u, v, 12);
        let spot = if lid % 7 == 0 {
            (1.0 - l1 / 0.28).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let base = grey.lerp(stone, mottle).lerp(lichen, spot * 0.8);
        let albedo = joint.lerp(base, crack);
        (albedo, 0.15 + 0.75 * crack * (0.8 + 0.2 * mottle))
    })
}

/// How textures go onto the surface.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mapping {
    /// The two projections the normal faces most, blended (two samples a material).
    Biplanar,
    /// All three projections blended by the normal (three samples a material).
    Triplanar,
}

/// How materials blend where the weights overlap.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Linear,
    /// The higher texel wins where weights are close: pebbles and tufts poke through.
    Height,
}

fn project(p: Vec3, axis: usize) -> Vec2 {
    match axis {
        0 => Vec2::new(p.z, p.y),
        1 => Vec2::new(p.x, p.z),
        _ => Vec2::new(p.x, p.y),
    }
}

/// A texture on the surface at `p` with normal `n`, and the samples taken.
fn mapped(tex: &Texture, p: Vec3, n: Vec3, mapping: Mapping) -> ((Vec3, f32), u32) {
    let a = n.abs();
    match mapping {
        Mapping::Biplanar => {
            let major = if a.x > a.y && a.x > a.z {
                0
            } else if a.y > a.z {
                1
            } else {
                2
            };
            let minor = if a.x < a.y && a.x < a.z {
                0
            } else if a.y < a.z {
                1
            } else {
                2
            };
            let median = 3 - major - minor;
            let w = Vec2::new(a[major], a[median]);
            let w = ((w - 0.5773) / (1.0 - 0.5773)).clamp(Vec2::ZERO, Vec2::ONE);
            let (c1, h1) = tex.sample(project(p, major));
            let (c2, h2) = tex.sample(project(p, median));
            let total = (w.x + w.y).max(1e-6);
            (
                ((c1 * w.x + c2 * w.y) / total, (h1 * w.x + h2 * w.y) / total),
                2,
            )
        }
        Mapping::Triplanar => {
            let w = a.powf(4.0);
            let w = w / (w.x + w.y + w.z);
            let mut c = Vec3::ZERO;
            let mut h = 0.0;
            for axis in 0..3 {
                let (ca, ha) = tex.sample(project(p, axis));
                c += ca * w[axis];
                h += ha * w[axis];
            }
            ((c, h), 3)
        }
    }
}

/// Which texture a scene material uses here: 0 turf, 1 limestone.
fn layer(material: u16) -> usize {
    if material == super::scenes::LIMESTONE {
        1
    } else {
        0
    }
}

/// Shades a view with textured, blended materials. Returns linear colours and the texture
/// samples taken per shaded pixel.
#[allow(clippy::too_many_arguments)]
pub fn shade(
    cam: &Camera,
    vis: &Visibility,
    mesh: &Mesh,
    ao: &[f32],
    shadow: &ShadowMap,
    sun: Vec3,
    textures: &[Texture; 2],
    mapping: Mapping,
    blend: Blend,
) -> (Vec<Vec3>, f64) {
    let flat = [Vec3::ONE; 16];
    let mut out = vec![Vec3::ZERO; vis.w * vis.h];
    let (mut samples, mut shaded) = (0u64, 0u64);
    for y in 0..vis.h {
        for x in 0..vis.w {
            let i = y * vis.w + x;
            let t = vis.tri[i];
            if t == u32::MAX {
                let d = cam.ray(x as f32 + 0.5, y as f32 + 0.5);
                out[i] = Vec3::new(0.62, 0.72, 0.84)
                    .lerp(Vec3::new(0.22, 0.38, 0.68), d.y.clamp(0.0, 1.0).sqrt());
                continue;
            }
            let s: Surface = surface(mesh, ao, &flat, t, vis.bary[i]);
            // The pixel's weight of each texture, from its corners' material weights.
            let idx = &mesh.indices[t as usize * 3..t as usize * 3 + 3];
            let b = vis.bary[i];
            let corner = [1.0 - b.x - b.y, b.x, b.y];
            let mut w = [0.0f32; 2];
            for k in 0..3 {
                let v = idx[k] as usize;
                for (m, mw) in mesh.materials[v].iter().zip(mesh.weights[v]) {
                    w[layer(*m)] += mw * corner[k];
                }
            }
            let mut texels = [(Vec3::ZERO, 0.0f32); 2];
            for (l, tex) in textures.iter().enumerate() {
                if w[l] > 0.001 {
                    let (texel, n) = mapped(tex, s.at, s.normal, mapping);
                    texels[l] = texel;
                    samples += n as u64;
                }
            }
            let albedo = match blend {
                Blend::Linear => texels[0].0 * w[0] + texels[1].0 * w[1],
                Blend::Height => {
                    let depth = 0.2;
                    let top = (texels[0].1 + w[0]).max(texels[1].1 + w[1]) - depth;
                    let b0 = (texels[0].1 + w[0] - top).max(0.0);
                    let b1 = (texels[1].1 + w[1] - top).max(0.0);
                    (texels[0].0 * b0 + texels[1].0 * b1) / (b0 + b1).max(1e-6)
                }
            };
            // The limestone's beds: thin bands across the rock in world space (S §4.2).
            let strata = 0.88
                + 0.12
                    * ((s.at.y * std::f32::consts::TAU / 0.37).sin() * 0.6
                        + (s.at.y * 1.7).sin() * 0.4);
            let rock = w[1] / (w[0] + w[1]).max(1e-6);
            let albedo = albedo * (1.0 + (strata - 1.0) * rock);
            let height = texels[0].1 * w[0] + texels[1].1 * w[1];
            let lit = Surface {
                albedo: albedo * (0.72 + 0.28 * height),
                ..s
            };
            out[i] = light(&lit, shadow, sun, cam.eye);
            shaded += 1;
        }
    }
    (out, samples as f64 / shaded.max(1) as f64)
}
