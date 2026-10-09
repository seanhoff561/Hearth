//! The sun's shadows (R1a; RGA-1's first gap): cascaded shadow maps.
//!
//! Five cascades share one depth texture array. Four lie about the camera for the full-detail
//! world, 12 to 320 blocks in radius (a texel of 1.2 cm to 31 cm at 2048), and one as wide as
//! the distant terrain is drawn for its hills and mountains. Each is a sphere about the camera,
//! not a slice of the view, so it holds its texels however the camera turns, and its centre is
//! snapped to its texels in the light's space so they do not crawl as the camera walks: the
//! shadows' edges stand still. The light they are drawn for moves on only when the sun has
//! turned a little (a quarter of a degree, about a minute of the day) and then all at once, or
//! the snapping would be undone by the light's own slow turn. The outer cascades are redrawn
//! every few frames (the camera moves little in between), each sampled with the matrix it was
//! drawn with, moved by how far the camera has gone since.
//!
//! The terrain casts with its own vertex shaders (`TerrainRenderer::render_shadows`): a
//! cascade's globals are the frame's with the light's view-projection in place of the camera's.
//! Solid things cast only their faces turned away from the sun, so a lit face never compares
//! against itself; plants and models cast both sides. The full-detail world casts into the near
//! cascades, the distant terrain beyond it into the far one (`LodRenderer::draw_shadow`), and a
//! point takes the darker of its near cascade and the far one: a mountain's shadow reaches the
//! valley the player stands in.

use glam::{DMat3, DVec3, Mat4, Vec3};

use crate::gpu::GpuContext;

/// The shadow maps' depth format (standard depth: 0 nearest the sun).
pub const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// The cascades: four for the full-detail world, one for the distant terrain.
pub const CASCADES: usize = 5;
/// The cascades drawn from the full-detail terrain (the rest from the distant one).
pub const NEAR_CASCADES: usize = 4;
/// The near cascades' radii about the camera (blocks).
pub const NEAR_RADII: [f32; NEAR_CASCADES] = [12.0, 40.0, 120.0, 320.0];
/// The far cascade's radius when nothing sets it, and its bounds (blocks).
pub const FAR_RADIUS: f32 = 4096.0;
const FAR_RADIUS_RANGE: (f32, f32) = (1024.0, 12_000.0);
/// How far toward the sun beyond a cascade's sphere casters are taken (blocks): a cliff or a
/// tree up there still shades it.
pub const REACH_UP: [f32; CASCADES] = [200.0, 300.0, 400.0, 600.0, 4000.0];
/// A cascade is redrawn sooner when the camera has gone this share of its radius since.
const MOVED: f64 = 0.1;
/// The light the maps are drawn for moves on when the sun has turned this far (radians).
const TURNED: f32 = 0.004;

/// Shadows by quality: the maps' size and how often the outer cascades are redrawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowQuality {
    Off,
    /// 1024² a cascade.
    Low,
    /// 2048² a cascade.
    Medium,
    /// 2048², the outer cascades redrawn twice as often.
    High,
}

impl ShadowQuality {
    pub fn size(self) -> u32 {
        match self {
            ShadowQuality::Off => 1,
            ShadowQuality::Low => 1024,
            ShadowQuality::Medium | ShadowQuality::High => 2048,
        }
    }

    /// Frames between a cascade's redraws, and the frame of the cycle it is drawn on (so the
    /// outer ones do not all fall on the same frame).
    fn schedule(self) -> ([u64; CASCADES], [u64; CASCADES]) {
        match self {
            ShadowQuality::High => ([1, 1, 2, 4, 8], [0, 0, 1, 2, 5]),
            _ => ([1, 1, 4, 8, 16], [0, 0, 1, 2, 5]),
        }
    }
}

impl From<hearth_core::options::Quality> for ShadowQuality {
    fn from(q: hearth_core::options::Quality) -> Self {
        use hearth_core::options::Quality;
        match q {
            Quality::Off => ShadowQuality::Off,
            Quality::Low => ShadowQuality::Low,
            Quality::Medium => ShadowQuality::Medium,
            Quality::High => ShadowQuality::High,
        }
    }
}

/// One cascade as it was last drawn.
#[derive(Debug, Clone, Copy)]
pub struct Cascade {
    /// Its radius (blocks).
    pub radius: f32,
    /// Its centre (world, snapped to its texels in the light's space).
    pub center: DVec3,
    /// The camera's position when it was drawn, and the view-projection then (from points
    /// relative to that camera to the map's clip space).
    pub camera: DVec3,
    pub view_proj: Mat4,
    /// Drawn at least once with the present light.
    pub drawn: bool,
    /// To be drawn this frame.
    pub due: bool,
}

impl Default for Cascade {
    fn default() -> Self {
        Self {
            radius: 1.0,
            center: DVec3::ZERO,
            camera: DVec3::ZERO,
            view_proj: Mat4::IDENTITY,
            drawn: false,
            due: false,
        }
    }
}

/// The shadow maps and their cascades.
pub struct ShadowMaps {
    pub quality: ShadowQuality,
    pub size: u32,
    texture: wgpu::Texture,
    /// The whole array, for sampling.
    pub array_view: wgpu::TextureView,
    /// One view a cascade, to draw into.
    pub layer_views: Vec<wgpu::TextureView>,
    /// A placeholder array bound while the maps themselves are drawn into.
    pub dummy_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    /// The depths themselves, nearest (for finding what casts a shadow).
    pub depth_sampler: wgpu::Sampler,
    pub cascades: [Cascade; CASCADES],
    /// The far cascade's radius (blocks): as far as the distant terrain is drawn.
    pub far_radius: f32,
    /// The light the maps are drawn for (toward it), and its rotation: rows right, up and back.
    light: Option<Vec3>,
    basis: DMat3,
    frame: u64,
    /// Shadows this frame (a light above the horizon and shadows on).
    pub active: bool,
}

/// The light's rotation: rows right, up and back (toward the light), so the light looks down
/// its −z.
fn light_basis(light: Vec3) -> DMat3 {
    let back = light.as_dvec3().normalize();
    let up_hint = if back.y.abs() > 0.99 {
        DVec3::Z
    } else {
        DVec3::Y
    };
    let right = up_hint.cross(back).normalize();
    let up = back.cross(right);
    DMat3::from_cols(right, up, back).transpose()
}

impl ShadowMaps {
    pub fn new(ctx: &GpuContext, quality: ShadowQuality) -> Self {
        let size = quality.size();
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow maps"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: CASCADES as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let array_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("shadow maps"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layer_views = (0..CASCADES as u32)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("shadow cascade"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let dummy = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow placeholder"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let dummy_view = dummy.create_view(&wgpu::TextureViewDescriptor {
            label: Some("shadow placeholder"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let depth_sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow depths"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        Self {
            quality,
            size,
            texture,
            array_view,
            layer_views,
            dummy_view,
            sampler,
            depth_sampler,
            cascades: [Cascade::default(); CASCADES],
            far_radius: FAR_RADIUS,
            light: None,
            basis: DMat3::IDENTITY,
            frame: 0,
            active: false,
        }
    }

    /// A cascade's radius (blocks).
    pub fn radius(&self, i: usize) -> f32 {
        if i < NEAR_CASCADES {
            NEAR_RADII[i]
        } else {
            self.far_radius
                .clamp(FAR_RADIUS_RANGE.0, FAR_RADIUS_RANGE.1)
        }
    }

    /// Plans this frame's cascades for a camera and a light (the direction toward it): which
    /// are redrawn and with what matrices. `force` redraws them all (a picture, a teleport).
    pub fn plan(&mut self, camera: DVec3, light: Vec3, force: bool) {
        self.frame += 1;
        self.active = self.quality != ShadowQuality::Off && light.y > 0.0;
        if !self.active {
            for c in &mut self.cascades {
                c.due = false;
            }
            return;
        }
        // The light moves on in steps, every cascade redrawn for it at once.
        let mut force = force;
        if self.light.is_none_or(|l| l.angle_between(light) > TURNED) {
            self.light = Some(light);
            self.basis = light_basis(light);
            force = true;
        }
        let basis = self.basis;
        let rot = basis.as_mat3();
        let (every, phase) = self.quality.schedule();
        for i in 0..CASCADES {
            let r = self.radius(i);
            let c = &mut self.cascades[i];
            let moved = (camera - c.camera).length() > r as f64 * MOVED;
            let turn = self.frame % every[i] == phase[i] % every[i];
            c.due = force || !c.drawn || moved || turn || c.radius != r;
            if !c.due {
                continue;
            }
            // The centre snapped to the cascade's texels in the light's plane.
            let texel = 2.0 * r as f64 / self.size as f64;
            let ls = basis * camera;
            let snapped = DVec3::new(
                (ls.x / texel).round() * texel,
                (ls.y / texel).round() * texel,
                ls.z,
            );
            let center = basis.transpose() * snapped;
            // Points relative to the camera into the light's space about the centre.
            let view = Mat4::from_mat3(rot) * Mat4::from_translation((camera - center).as_vec3());
            let proj =
                glam::camera::rh::proj::directx::orthographic(-r, r, -r, r, -(r + REACH_UP[i]), r);
            *c = Cascade {
                radius: r,
                center,
                camera,
                view_proj: proj * view,
                drawn: true,
                due: true,
            };
        }
    }

    /// The cascades to draw this frame.
    pub fn due(&self) -> impl Iterator<Item = usize> + '_ {
        (0..CASCADES).filter(|&i| self.active && self.cascades[i].due)
    }

    /// The matrices to sample each cascade with this frame: from points relative to the camera
    /// now into the map as it was drawn.
    pub fn sample_matrices(&self, camera: DVec3) -> [Mat4; CASCADES] {
        std::array::from_fn(|i| {
            let c = &self.cascades[i];
            c.view_proj * Mat4::from_translation((camera - c.camera).as_vec3())
        })
    }

    /// Whether a camera-relative box `lo..hi` (blocks) can cast into cascade `i` as planned for
    /// a camera at `camera`: within its square across the light and above its far side.
    pub fn may_cast(&self, i: usize, camera: DVec3, lo: Vec3, hi: Vec3) -> bool {
        let c = &self.cascades[i];
        let r = c.radius;
        let mid = (lo + hi) * 0.5;
        let half = (hi - lo).length() * 0.5;
        let p = self.basis.as_mat3() * (mid + (camera - c.center).as_vec3());
        p.x.abs() <= r + half
            && p.y.abs() <= r + half
            && p.z <= r + REACH_UP[i] + half
            && p.z >= -r - half
    }

    /// The camera-relative box (blocks; low and high corners) holding all that can cast into
    /// cascade `i` as planned for a camera at `camera`, `margin` wider all round: its square
    /// across the light from below its sphere to its reach toward the sun.
    pub fn reach_box(&self, i: usize, camera: DVec3, margin: f32) -> (Vec3, Vec3) {
        let c = &self.cascades[i];
        let r = c.radius + margin;
        let to_world = self.basis.as_mat3().transpose();
        let centre = (c.center - camera).as_vec3();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for k in 0..8 {
            let corner = Vec3::new(
                if k & 1 == 0 { -r } else { r },
                if k & 2 == 0 { -r } else { r },
                if k & 4 == 0 { -r } else { r + REACH_UP[i] },
            );
            let w = to_world * corner + centre;
            lo = lo.min(w);
            hi = hi.max(w);
        }
        (lo, hi)
    }

    /// The direction toward the light the maps are drawn for.
    pub fn light(&self) -> Option<Vec3> {
        self.light
    }

    /// The world size of a texel of cascade `i` (blocks).
    pub fn texel(&self, i: usize) -> f32 {
        2.0 * self.cascades[i].radius / self.size as f32
    }

    pub fn bytes(&self) -> u64 {
        (self.size as u64).pow(2) * 4 * CASCADES as u64
    }

    /// Keeps the texture alive (it is only reached through its views).
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat3;

    #[test]
    fn the_light_basis_looks_down_toward_the_ground() {
        let light = Vec3::new(0.3, 0.8, 0.5).normalize();
        let b = light_basis(light).as_mat3();
        // The light's back axis points at the light; its forward (−z) down along the rays.
        assert!((b * light - Vec3::Z).length() < 1e-5);
        // Orthonormal.
        assert!((b * b.transpose()).abs_diff_eq(Mat3::IDENTITY, 1e-5));
    }

    fn maps(quality: ShadowQuality) -> Option<ShadowMaps> {
        match GpuContext::headless(true) {
            Ok(ctx) => Some(ShadowMaps::new(&ctx, quality)),
            Err(_) => {
                eprintln!("skipped: no GPU adapter");
                None
            }
        }
    }

    #[test]
    fn the_light_moves_on_in_steps_and_the_outer_cascades_take_turns() {
        let Some(mut m) = maps(ShadowQuality::Medium) else {
            return;
        };
        let camera = DVec3::new(1.0e6, 120.0, -3.0e5);
        let sun = Vec3::new(0.3, 0.6, 0.74).normalize();
        m.plan(camera, sun, false);
        assert_eq!(m.due().count(), CASCADES, "all drawn at first");
        // A still camera under a still sun: the near two every frame, the others by turns,
        // the third and fourth never together.
        let mut drawn = [0; CASCADES];
        for _ in 0..16 {
            m.plan(camera, sun, false);
            let due: Vec<usize> = m.due().collect();
            assert!(due.contains(&0) && due.contains(&1), "{due:?}");
            assert!(!(due.contains(&2) && due.contains(&3)), "{due:?}");
            for i in due {
                drawn[i] += 1;
            }
        }
        assert_eq!(drawn, [16, 16, 4, 2, 1]);
        // The sun turning a little: the maps keep their light; more, all redrawn for the new.
        let nudged = (sun + Vec3::new(0.001, 0.0, 0.0)).normalize();
        m.plan(camera, nudged, false);
        assert_eq!(m.light(), Some(sun));
        let turned = (sun + Vec3::new(0.01, 0.0, 0.0)).normalize();
        m.plan(camera, turned, false);
        assert_eq!(m.light(), Some(turned));
        assert_eq!(m.due().count(), CASCADES);
        // The sun down: nothing drawn.
        m.plan(camera, Vec3::new(0.3, -0.1, 0.9).normalize(), false);
        assert!(!m.active && m.due().count() == 0);
    }

    #[test]
    fn casters_are_taken_toward_the_sun_and_not_beside_the_cascade() {
        let Some(mut m) = maps(ShadowQuality::Low) else {
            return;
        };
        let camera = DVec3::new(-2.0e5, 64.0, 7.0e5);
        let sun = Vec3::new(-0.2, 0.7, 0.4).normalize();
        m.plan(camera, sun, true);
        let cube = |at: Vec3| (at, at + Vec3::splat(16.0));
        // A cube 150 blocks up toward the sun shades the ground about the camera.
        let (lo, hi) = cube(sun * 150.0 - Vec3::splat(8.0));
        assert!(m.may_cast(0, camera, lo, hi));
        // One 100 blocks to the side lies outside the nearest cascade, inside the third.
        let side = sun.cross(Vec3::Y).normalize() * 100.0;
        let (lo, hi) = cube(side - Vec3::splat(8.0));
        assert!(!m.may_cast(0, camera, lo, hi));
        assert!(m.may_cast(2, camera, lo, hi));
        // One deep below the cascade's sphere casts into nothing it holds.
        let (lo, hi) = cube(-sun * 200.0);
        assert!(!m.may_cast(1, camera, lo, hi));
    }

    #[test]
    fn every_caster_lies_in_its_cascade_s_box() {
        let Some(mut m) = maps(ShadowQuality::Medium) else {
            return;
        };
        let camera = DVec3::new(3.0e5, 90.0, -4.0e5);
        let sun = Vec3::new(0.5, 0.35, -0.2).normalize();
        m.plan(camera, sun, true);
        for i in 0..NEAR_CASCADES {
            let (lo, hi) = m.reach_box(i, camera, 0.0);
            let mut cast = 0;
            // Points over the box and 20 blocks about it.
            for k in 0u32..4000 {
                let f = |s: u32| {
                    let mut x = k.wrapping_mul(0x9e37_79b1) ^ s.wrapping_mul(0x85eb_ca77);
                    x ^= x >> 15;
                    x = x.wrapping_mul(0x2c1b_3c6d);
                    x ^= x >> 12;
                    (x % 1000) as f32 / 1000.0
                };
                let at = lo - 20.0 + (hi - lo + 40.0) * Vec3::new(f(1), f(2), f(3));
                if m.may_cast(i, camera, at, at) {
                    cast += 1;
                    assert!(
                        at.cmpge(lo - 1e-3).all() && at.cmple(hi + 1e-3).all(),
                        "{i}: {at}"
                    );
                }
            }
            assert!(cast > 0, "cascade {i} found casters");
        }
    }

    #[test]
    fn a_cascade_holds_its_texels_as_the_camera_walks() {
        // The snapped centre moves by whole texels: a point fixed in the world lands on the same
        // place within its texel however far the camera goes.
        let light = Vec3::new(0.4, 0.7, -0.2).normalize();
        let basis = light_basis(light);
        let texel = 2.0 * NEAR_RADII[1] as f64 / 2048.0;
        let world_point = DVec3::new(1234.567, 64.25, -987.5);
        let mut frac: Option<(f64, f64)> = None;
        for k in 0..20 {
            let camera = DVec3::new(1200.0 + k as f64 * 0.37, 70.0, -990.0 + k as f64 * 0.11);
            let ls = basis * camera;
            let snapped = DVec3::new(
                (ls.x / texel).round() * texel,
                (ls.y / texel).round() * texel,
                ls.z,
            );
            let p = basis * world_point - snapped;
            let f = ((p.x / texel).rem_euclid(1.0), (p.y / texel).rem_euclid(1.0));
            if let Some(f0) = frac {
                assert!(
                    (f.0 - f0.0).abs() < 1e-6 && (f.1 - f0.1).abs() < 1e-6,
                    "{f:?} {f0:?}"
                );
            }
            frac = Some(f);
        }
    }
}
