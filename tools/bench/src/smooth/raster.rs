//! A small CPU rasterizer for S0's comparison renders: a visibility buffer (the triangle and its
//! barycentrics at each pixel, perspective-correct, clipped at the near plane), a sun shadow
//! map, and the shading the game would give smooth ground: blended material colours, normals
//! between the smooth and the faceted by sharpness, AO from the fill field, distance haze and
//! still water.

use glam::{Vec2, Vec3};
use hearth_smooth::{Field, Mesh};

const NEAR: f32 = 0.05;
const NONE: u32 = u32::MAX;

/// A pinhole camera, y up.
#[derive(Clone, Copy)]
pub struct Camera {
    pub eye: Vec3,
    right: Vec3,
    up: Vec3,
    fwd: Vec3,
    focal: f32,
    pub w: usize,
    pub h: usize,
}

impl Camera {
    pub fn look(eye: Vec3, target: Vec3, fov_y_deg: f32, w: usize, h: usize) -> Self {
        let fwd = (target - eye).normalize();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let focal = (h as f32 * 0.5) / (fov_y_deg.to_radians() * 0.5).tan();
        Self {
            eye,
            right,
            up,
            fwd,
            focal,
            w,
            h,
        }
    }

    fn view(&self, p: Vec3) -> Vec3 {
        let v = p - self.eye;
        Vec3::new(v.dot(self.right), v.dot(self.up), v.dot(self.fwd))
    }

    fn screen(&self, v: Vec3) -> Vec2 {
        Vec2::new(
            self.w as f32 * 0.5 + v.x / v.z * self.focal,
            self.h as f32 * 0.5 - v.y / v.z * self.focal,
        )
    }

    /// The world direction through a pixel position.
    pub fn ray(&self, x: f32, y: f32) -> Vec3 {
        (self.fwd * self.focal + self.right * (x - self.w as f32 * 0.5)
            - self.up * (y - self.h as f32 * 0.5))
            .normalize()
    }
}

/// What each pixel sees: a triangle and where on it.
pub struct Visibility {
    pub w: usize,
    pub h: usize,
    pub tri: Vec<u32>,
    /// Weights of the triangle's corners 1 and 2 (corner 0 has the rest).
    pub bary: Vec<Vec2>,
    pub depth: Vec<f32>,
}

fn edge(a: Vec2, b: Vec2, p: Vec2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

pub fn rasterize(cam: &Camera, mesh: &Mesh) -> Visibility {
    let n = cam.w * cam.h;
    let mut vis = Visibility {
        w: cam.w,
        h: cam.h,
        tri: vec![NONE; n],
        bary: vec![Vec2::ZERO; n],
        depth: vec![f32::INFINITY; n],
    };
    let corners = [Vec3::X, Vec3::Y, Vec3::Z];
    for t in 0..mesh.triangle_count() {
        let p = mesh.triangle(t);
        let v = p.map(|p| cam.view(p));
        if v.iter().all(|v| v.z < NEAR) {
            continue;
        }
        // Clip against the near plane, carrying each corner's weights of the original three.
        let mut poly: Vec<(Vec3, Vec3)> = Vec::with_capacity(4);
        for k in 0..3 {
            let (a, b) = ((v[k], corners[k]), (v[(k + 1) % 3], corners[(k + 1) % 3]));
            if a.0.z >= NEAR {
                poly.push(a);
            }
            if (a.0.z >= NEAR) != (b.0.z >= NEAR) {
                let s = (NEAR - a.0.z) / (b.0.z - a.0.z);
                poly.push((a.0.lerp(b.0, s), a.1.lerp(b.1, s)));
            }
        }
        for k in 1..poly.len().saturating_sub(1) {
            let tri = [poly[0], poly[k], poly[k + 1]];
            fill(&mut vis, cam, t as u32, tri);
        }
    }
    vis
}

fn fill(vis: &mut Visibility, cam: &Camera, t: u32, tri: [(Vec3, Vec3); 3]) {
    let s = tri.map(|(v, _)| cam.screen(v));
    let iz = tri.map(|(v, _)| 1.0 / v.z);
    let area = edge(s[0], s[1], s[2]);
    if area.abs() < 1e-9 {
        return;
    }
    let lo = s[0].min(s[1]).min(s[2]).floor().max(Vec2::ZERO);
    let hi = s[0]
        .max(s[1])
        .max(s[2])
        .ceil()
        .min(Vec2::new(vis.w as f32, vis.h as f32));
    for y in lo.y as usize..hi.y as usize {
        for x in lo.x as usize..hi.x as usize {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge(s[1], s[2], p) / area;
            let w1 = edge(s[2], s[0], p) / area;
            let w2 = edge(s[0], s[1], p) / area;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let izp = w0 * iz[0] + w1 * iz[1] + w2 * iz[2];
            let z = 1.0 / izp;
            let i = y * vis.w + x;
            if z >= vis.depth[i] {
                continue;
            }
            let b =
                (tri[0].1 * (w0 * iz[0]) + tri[1].1 * (w1 * iz[1]) + tri[2].1 * (w2 * iz[2])) * z;
            vis.depth[i] = z;
            vis.tri[i] = t;
            vis.bary[i] = Vec2::new(b.y, b.z);
        }
    }
}

/// Depth toward the sun over a square of ground: a point is lit where nothing lies above it
/// along the sun's direction.
pub struct ShadowMap {
    res: usize,
    centre: Vec3,
    right: Vec3,
    up: Vec3,
    dir: Vec3,
    scale: f32,
    /// The highest point toward the sun over each texel.
    top: Vec<f32>,
}

impl ShadowMap {
    pub fn build(mesh: &Mesh, sun: Vec3, centre: Vec3, radius: f32, res: usize) -> Self {
        let dir = sun.normalize();
        let right = dir.cross(Vec3::Y).normalize_or(Vec3::X);
        let up = right.cross(dir);
        let mut map = Self {
            res,
            centre,
            right,
            up,
            dir,
            scale: res as f32 / (2.0 * radius),
            top: vec![f32::NEG_INFINITY; res * res],
        };
        for t in 0..mesh.triangle_count() {
            let p = mesh.triangle(t);
            let s = p.map(|p| map.texel(p));
            let h = p.map(|p| (p - centre).dot(dir));
            let area = edge(s[0], s[1], s[2]);
            if area.abs() < 1e-9 {
                continue;
            }
            let lo = s[0].min(s[1]).min(s[2]).floor().max(Vec2::ZERO);
            let hi = s[0].max(s[1]).max(s[2]).ceil().min(Vec2::splat(res as f32));
            for y in lo.y as usize..hi.y as usize {
                for x in lo.x as usize..hi.x as usize {
                    let q = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = edge(s[1], s[2], q) / area;
                    let w1 = edge(s[2], s[0], q) / area;
                    let w2 = edge(s[0], s[1], q) / area;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let d = w0 * h[0] + w1 * h[1] + w2 * h[2];
                    let i = y * res + x;
                    if d > map.top[i] {
                        map.top[i] = d;
                    }
                }
            }
        }
        map
    }

    fn texel(&self, p: Vec3) -> Vec2 {
        let v = p - self.centre;
        Vec2::new(
            self.res as f32 * 0.5 + v.dot(self.right) * self.scale,
            self.res as f32 * 0.5 - v.dot(self.up) * self.scale,
        )
    }

    /// How lit a point is, 0..1, averaged over the texels around it.
    pub fn light(&self, p: Vec3) -> f32 {
        let s = self.texel(p);
        let h = (p - self.centre).dot(self.dir);
        let (cx, cy) = (s.x.floor() as i64, s.y.floor() as i64);
        let mut lit = 0.0;
        let mut n = 0.0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (x, y) = (cx + dx, cy + dy);
                n += 1.0;
                if x < 0 || y < 0 || x >= self.res as i64 || y >= self.res as i64 {
                    lit += 1.0;
                    continue;
                }
                if h >= self.top[y as usize * self.res + x as usize] - 0.12 {
                    lit += 1.0;
                }
            }
        }
        lit / n
    }
}

/// Ambient occlusion at each vertex from the fill field: how much of the space along its normal
/// is ground (three samples, as S2's mesh-time voxel AO).
pub fn field_ao(field: &Field, mesh: &Mesh) -> Vec<f32> {
    let origin = field.origin().as_vec3();
    mesh.positions
        .iter()
        .zip(&mesh.normals)
        .map(|(p, n)| {
            let local = *p - origin;
            let mut occlusion = 0.0;
            let mut total = 0.0;
            for (step, weight) in [(0.4f32, 1.0f32), (0.85, 0.7), (1.4, 0.45)] {
                let open = -field.sample(local + *n * step);
                occlusion += weight * (step - open).max(0.0);
                total += weight * step;
            }
            (1.0 - 1.6 * occlusion / total).clamp(0.15, 1.0)
        })
        .collect()
}

/// How a view is shaded.
pub struct Look<'a> {
    /// Toward the sun.
    pub sun: Vec3,
    pub water: Option<f32>,
    /// Linear albedo by material id.
    pub palette: &'a [Vec3],
    /// Draw the triangles' edges over a paler surface.
    pub wire: bool,
}

pub fn srgb_to_linear(c: [u8; 3]) -> Vec3 {
    Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32).map(|v| (v / 255.0).powf(2.2))
}

fn sky(dir: Vec3) -> Vec3 {
    let t = dir.y.clamp(0.0, 1.0).powf(0.5);
    Vec3::new(0.62, 0.72, 0.84).lerp(Vec3::new(0.22, 0.38, 0.68), t)
}

/// A mesh's surface at a pixel: position, shading normal, albedo, AO.
pub struct Surface {
    pub at: Vec3,
    pub normal: Vec3,
    /// The interpolated smooth normal.
    pub smooth: Vec3,
    pub albedo: Vec3,
    pub ao: f32,
}

/// The surface a visibility-buffer pixel sees.
pub fn surface(mesh: &Mesh, ao: &[f32], palette: &[Vec3], t: u32, b: Vec2) -> Surface {
    let i = &mesh.indices[t as usize * 3..t as usize * 3 + 3];
    let w = [1.0 - b.x - b.y, b.x, b.y];
    let mut at = Vec3::ZERO;
    let mut smooth = Vec3::ZERO;
    let mut albedo = Vec3::ZERO;
    let mut sharp = 0.0;
    let mut occl = 0.0;
    for k in 0..3 {
        let v = i[k] as usize;
        at += mesh.positions[v] * w[k];
        smooth += mesh.normals[v] * w[k];
        sharp += mesh.sharpness[v] * w[k];
        occl += ao[v] * w[k];
        for (m, mw) in mesh.materials[v].iter().zip(mesh.weights[v]) {
            albedo += palette[*m as usize] * (mw * w[k]);
        }
    }
    let smooth = smooth.normalize_or(Vec3::Y);
    let [p0, p1, p2] = [0, 1, 2].map(|k| mesh.positions[i[k] as usize]);
    let mut face = (p1 - p0).cross(p2 - p0).normalize_or(smooth);
    if face.dot(smooth) < 0.0 {
        face = -face;
    }
    Surface {
        at,
        normal: smooth
            .lerp(face, sharp.clamp(0.0, 1.0))
            .normalize_or(smooth),
        smooth,
        albedo,
        ao: occl,
    }
}

/// Sunlight, sky light and haze on a surface seen from `eye`.
pub fn light(s: &Surface, shadow: &ShadowMap, sun: Vec3, eye: Vec3) -> Vec3 {
    let n = s.normal;
    let lit = if n.dot(sun) > 0.0 {
        shadow.light(s.at + s.smooth * 0.15)
    } else {
        0.0
    };
    let direct = Vec3::new(1.0, 0.93, 0.82) * 1.9 * n.dot(sun).max(0.0) * lit;
    let hemi = Vec3::new(0.1, 0.09, 0.07).lerp(Vec3::new(0.24, 0.3, 0.42), n.y * 0.5 + 0.5);
    let c = s.albedo * (direct + hemi * s.ao * s.ao);
    haze(c, (s.at - eye).length(), (s.at - eye).normalize())
}

fn haze(c: Vec3, dist: f32, dir: Vec3) -> Vec3 {
    let f = 1.0 - (-dist / 900.0).exp();
    c.lerp(sky(dir) * 1.05, f)
}

/// Shades a view (linear colour per pixel).
pub fn shade(
    cam: &Camera,
    vis: &Visibility,
    mesh: &Mesh,
    ao: &[f32],
    shadow: &ShadowMap,
    look: &Look,
) -> Vec<Vec3> {
    let mut out = vec![Vec3::ZERO; vis.w * vis.h];
    for y in 0..vis.h {
        for x in 0..vis.w {
            let i = y * vis.w + x;
            let dir = cam.ray(x as f32 + 0.5, y as f32 + 0.5);
            let t = vis.tri[i];
            let mut c = if t == NONE {
                sky(dir)
            } else {
                let mut s = surface(mesh, ao, look.palette, t, vis.bary[i]);
                if look.wire {
                    s.albedo = s.albedo.lerp(Vec3::splat(0.32), 0.45);
                }
                let mut c = light(&s, shadow, look.sun, cam.eye);
                if look.wire && near_edge(cam, mesh, t, x, y) {
                    c *= 0.18;
                }
                c
            };
            if let Some(level) = look.water {
                c = water(
                    cam,
                    dir,
                    level,
                    c,
                    (t != NONE).then_some(vis.depth[i]),
                    look.sun,
                );
            }
            out[i] = c;
        }
    }
    out
}

/// Still water over the pixel: the ground seen through it fades with depth into the water's
/// colour, and the sky reflects at grazing angles.
fn water(cam: &Camera, dir: Vec3, level: f32, below: Vec3, depth: Option<f32>, sun: Vec3) -> Vec3 {
    if cam.eye.y <= level || dir.y >= -1e-4 {
        return below;
    }
    let t = (cam.eye.y - level) / -dir.y;
    // `depth` is along the view axis; the ray's length to the ground is longer by 1 / cos.
    let ground = depth.map(|z| z / dir.dot(cam.fwd));
    if let Some(g) = ground
        && g <= t
    {
        return below;
    }
    let through = ground.map_or(30.0, |g| g - t);
    let keep = (-through * 0.45).exp();
    let tint = Vec3::new(0.035, 0.075, 0.08) * (0.6 + 0.4 * sun.y);
    let body = below * keep * Vec3::new(0.75, 0.9, 0.88) + tint * (1.0 - keep);
    let fresnel = 0.02 + 0.98 * (1.0 + dir.y).powi(5);
    let reflected = sky(Vec3::new(dir.x, -dir.y, dir.z));
    haze(body.lerp(reflected, fresnel), t, dir)
}

/// Whether a pixel lies within about a pixel of its triangle's edges on screen.
fn near_edge(cam: &Camera, mesh: &Mesh, t: u32, x: usize, y: usize) -> bool {
    let p = mesh.triangle(t as usize);
    let v = p.map(|p| cam.view(p));
    if v.iter().any(|v| v.z < NEAR) {
        return false;
    }
    let s = v.map(|v| cam.screen(v));
    let q = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
    (0..3).any(|k| {
        let (a, b) = (s[k], s[(k + 1) % 3]);
        let ab = b - a;
        let len = ab.length();
        len > 0.0 && edge(a, b, q).abs() / len < 1.1
    })
}

/// Tone maps linear colour to sRGB bytes (a filmic curve).
pub fn to_srgb(c: Vec3) -> [u8; 3] {
    let x = c * 0.9;
    let y = (x * (x * 2.51 + 0.03)) / (x * (x * 2.43 + 0.59) + 0.14);
    let g = y.clamp(Vec3::ZERO, Vec3::ONE).map(|v| v.powf(1.0 / 2.2));
    [
        (g.x * 255.0 + 0.5) as u8,
        (g.y * 255.0 + 0.5) as u8,
        (g.z * 255.0 + 0.5) as u8,
    ]
}

/// Averages `factor`² pixels into one.
pub fn downsample(src: &[Vec3], w: usize, h: usize, factor: usize) -> Vec<[u8; 3]> {
    let (ow, oh) = (w / factor, h / factor);
    let mut out = Vec::with_capacity(ow * oh);
    let n = (factor * factor) as f32;
    for y in 0..oh {
        for x in 0..ow {
            let mut sum = Vec3::ZERO;
            for dy in 0..factor {
                for dx in 0..factor {
                    sum += src[(y * factor + dy) * w + x * factor + dx];
                }
            }
            out.push(to_srgb(sum / n));
        }
    }
    out
}
