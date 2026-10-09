//! Light scattered under the skin, pre-integrated (Amendment T §2.2; Penner and Borshukov,
//! "Pre-Integrated Skin Shading", GPU Pro 2): how much of the light falling on a ring of skin of a
//! given radius at each angle to the light comes out at a point, through skin's measured
//! diffusion profile (d'Eon and Luebke's six Gaussians, "Advanced Techniques for Realistic
//! Real-Time Skin Rendering", GPU Gems 3, ch. 14). Red light travels furthest in the dermis, so
//! a curved surface's terminator glows red past where a hard surface would go dark; on gentle
//! curves it is Lambert's law. A table of it by the cosine to the light (across) and the
//! surface's curvature (down, as `hearth_character::anatomy::curvature_unit` stores it).

/// The table's size: cosines across, curvatures down.
pub const LUT_W: usize = 128;
pub const LUT_H: usize = 64;

/// The diffusion profile: variances (mm²) and their weights in red, green and blue (each
/// colour's weights summing to one).
const PROFILE: [(f32, [f32; 3]); 6] = [
    (0.0064, [0.233, 0.455, 0.649]),
    (0.0484, [0.100, 0.336, 0.344]),
    (0.187, [0.118, 0.198, 0.0]),
    (0.567, [0.113, 0.007, 0.007]),
    (1.99, [0.358, 0.004, 0.0]),
    (7.41, [0.078, 0.0, 0.0]),
];

/// The radius (mm) a row of the table stands for: its curvature unit `v` is the square root of
/// the curvature's share of one over 2 mm (`CURVATURE_FULL`), so the radius is 2 mm / v².
pub fn radius_mm(v: f32) -> f32 {
    2.0 / (v * v).max(1e-6)
}

/// The light coming out on a ring of skin `r_mm` in radius at `theta` (radians) from the light,
/// for red, green and blue: the cosine law (light only on the lit side) blurred along the ring by
/// the profile, each Gaussian over the arc it reaches (`2r·sin(x/2)` from the point, the chord
/// across the ring), as the ring's slice of the profile (a Gaussian of the same variance).
pub fn ring(theta: f32, r_mm: f32) -> [f32; 3] {
    let mut num = [0.0f32; 3];
    let mut den = [0.0f32; 3];
    for (var, w) in PROFILE {
        let sigma = var.sqrt();
        // The arc the Gaussian reaches (four deviations), the whole ring on a small one.
        let reach = 4.0 * sigma / (2.0 * r_mm);
        let x_max = if reach >= 1.0 {
            std::f32::consts::PI
        } else {
            2.0 * reach.asin()
        };
        const STEPS: usize = 256;
        let dx = 2.0 * x_max / STEPS as f32;
        let (mut lit, mut all) = (0.0f32, 0.0f32);
        for k in 0..STEPS {
            let x = -x_max + (k as f32 + 0.5) * dx;
            let d = 2.0 * r_mm * (0.5 * x).sin().abs();
            // The ring's slice of the profile: a Gaussian of the same variance (the constant
            // over the arc cancels; between the Gaussians it does not).
            let g = (-d * d / (2.0 * var)).exp() / (2.0 * std::f32::consts::PI * var).sqrt();
            lit += (theta + x).cos().max(0.0) * g * dx;
            all += g * dx;
        }
        for c in 0..3 {
            num[c] += w[c] * lit;
            den[c] += w[c] * all;
        }
    }
    std::array::from_fn(|c| {
        if den[c] > 0.0 {
            num[c] / den[c]
        } else {
            theta.cos().max(0.0)
        }
    })
}

/// The table, row by row (curvature unit 0 at the top: Lambert's law), each texel the light
/// out for the cosine at its middle, RGBA (alpha one).
pub fn table() -> Vec<[f32; 4]> {
    let mut out = Vec::with_capacity(LUT_W * LUT_H);
    for y in 0..LUT_H {
        let v = y as f32 / (LUT_H - 1) as f32;
        for x in 0..LUT_W {
            let cos = (x as f32 + 0.5) / LUT_W as f32 * 2.0 - 1.0;
            let theta = cos.clamp(-1.0, 1.0).acos();
            let c = if y == 0 {
                [cos.max(0.0); 3]
            } else {
                ring(theta, radius_mm(v))
            };
            out.push([c[0], c[1], c[2], 1.0]);
        }
    }
    out
}

/// A half float (IEEE 754 binary16) from a single, rounded to nearest.
pub fn to_f16(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32 - 127 + 15;
    let man = b & 0x7f_ffff;
    if exp >= 31 {
        return sign | 0x7c00;
    }
    if exp <= 0 {
        if exp < -10 {
            return sign;
        }
        let m = (man | 0x80_0000) >> (1 - exp);
        return sign | ((m + 0x1000) >> 13) as u16;
    }
    let v = ((exp as u32) << 10) | (man >> 13);
    sign | (v + ((man >> 12) & 1)) as u16
}

/// The table on the GPU: a filtered half-float texture, its view and a clamping sampler.
pub struct SkinLut {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl SkinLut {
    pub fn new(ctx: &crate::gpu::GpuContext) -> Self {
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("skin scattering"),
            size: wgpu::Extent3d {
                width: LUT_W as u32,
                height: LUT_H as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let half: Vec<u16> = table().iter().flat_map(|t| t.map(to_f16)).collect();
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&half),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((LUT_W * 8) as u32),
                rows_per_image: Some(LUT_H as u32),
            },
            wgpu::Extent3d {
                width: LUT_W as u32,
                height: LUT_H as u32,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("skin scattering"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self { view, sampler }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats_round_trip_the_tables_range() {
        assert_eq!(to_f16(1.0), 0x3c00);
        assert_eq!(to_f16(0.0), 0);
        assert_eq!(to_f16(0.5), 0x3800);
        assert_eq!(to_f16(-2.0), 0xc000);
        // A small value keeps its size within the format's precision.
        let back = |h: u16| {
            let e = ((h >> 10) & 0x1f) as i32;
            let m = (h & 0x3ff) as f32;
            if e == 0 {
                m * 2f32.powi(-24)
            } else {
                (1.0 + m / 1024.0) * 2f32.powi(e - 15)
            }
        };
        for x in [0.003f32, 0.02, 0.137, 0.71, 0.999] {
            assert!((back(to_f16(x)) - x).abs() <= x * 1e-3 + 1e-6, "{x}");
        }
    }

    #[test]
    fn a_gentle_curve_is_lamberts_law_and_a_sharp_one_glows_red_past_the_terminator() {
        // A cheek's 5 cm: within a percent or two of the cosine law.
        for deg in [0.0f32, 30.0, 60.0, 80.0] {
            let t = deg.to_radians();
            let c = ring(t, 50.0);
            for (i, v) in c.iter().enumerate() {
                assert!((v - t.cos()).abs() < 0.03, "{deg}° channel {i}: {v}");
            }
        }
        // An ear's rim or a nostril's wing (4 mm): past the terminator, red comes through and
        // blue barely.
        let past = ring(100f32.to_radians(), 4.0);
        assert!(past[0] > 0.02, "{past:?}");
        assert!(past[0] > 10.0 * past[2], "{past:?}");
        // Facing the light, the light the edge loses is red's most.
        let lit = ring(0.0, 4.0);
        assert!(lit[0] < lit[2] && lit[2] <= 1.0, "{lit:?}");
    }

    #[test]
    fn scattering_moves_light_round_the_ring_without_making_or_losing_it() {
        // Over the whole ring the light out equals the light in (the cosine's integral, 2).
        for r in [3.0f32, 8.0, 20.0] {
            let n = 720;
            let mut sum = [0.0f32; 3];
            for k in 0..n {
                let t = -std::f32::consts::PI + (k as f32 + 0.5) / n as f32 * std::f32::consts::TAU;
                let c = ring(t.abs(), r);
                for i in 0..3 {
                    sum[i] += c[i] * std::f32::consts::TAU / n as f32;
                }
            }
            for (i, s) in sum.iter().enumerate() {
                assert!((s - 2.0).abs() < 0.03, "r {r} mm, channel {i}: {s}");
            }
        }
    }

    #[test]
    fn the_table_runs_from_the_cosine_law_down_to_the_sharpest_curve() {
        let t = table();
        assert_eq!(t.len(), LUT_W * LUT_H);
        // The top row is the cosine law exactly; every texel within the light there is.
        let top_last = t[LUT_W - 1];
        assert!((top_last[0] - (1.0 - 1.0 / LUT_W as f32)).abs() < 1e-4);
        assert!(t.iter().all(|c| c.iter().all(|v| (0.0..=1.0).contains(v))));
        // The bottom row's terminator (cosine 0) is red.
        let bottom = &t[(LUT_H - 1) * LUT_W..];
        let mid = bottom[LUT_W / 2];
        assert!(mid[0] > mid[1] && mid[1] > mid[2], "{mid:?}");
    }
}
