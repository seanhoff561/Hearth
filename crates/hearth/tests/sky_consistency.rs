//! The sky drawn on the GPU (the sky-view table) and the sky irradiance the CPU computes for
//! lighting and exposure (`hearth_env::sky`) must agree, from high sun through twilight;
//! otherwise the sky and the terrain it lights drift apart in brightness. Skipped without a GPU.

use std::f64::consts::PI;

use glam::{DVec3, Mat3, Mat4, Vec2, Vec3, Vec4};
use hearth_render::GpuContext;
use hearth_render::sky::{SKYVIEW_SIZE, SkyParams, SkyRenderer};

/// Pre-exposure used for the table (keeps twilight values out of half-float denormals).
const SCALE: f32 = 1.0e4;

fn sun_at(elev_deg: f64) -> DVec3 {
    let e = elev_deg.to_radians();
    DVec3::new(0.3 * e.cos(), e.sin(), -0.954 * e.cos()).normalize()
}

/// Horizontal irradiance from the upper half of the sky-view table, per unit illuminance.
fn gpu_irradiance(ctx: &GpuContext, sky: &mut SkyRenderer, sun: DVec3, alt: f32) -> [f64; 3] {
    let params = SkyParams {
        sun_dir: sun.as_vec3(),
        sun_illuminance: SCALE,
        moon_dir: -sun.as_vec3(),
        moon_illuminance: 0.0,
        moon_phase: 0.0,
        altitude: alt,
        haze: 1.0,
        seconds: 0.0,
        turbulence: 0.0,
        star_rotation: Mat3::IDENTITY,
        star_visibility: 0.0,
        cloud_cover: 0.0,
        cloud_height: 0.0,
        cloud_offset: Vec2::ZERO,
        exposure: 1.0,
        night: 0.0,
        moon_disc: 0.0,
        direct: Vec3::ZERO,
        ambient: Vec3::ZERO,
        overcast: Vec4::ZERO,
    };
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("sky") });
    sky.update(ctx, &mut enc, &params, Mat4::IDENTITY);
    ctx.queue.submit(Some(enc.finish()));
    let texels = sky.read_skyview(ctx);
    let (w, h) = SKYVIEW_SIZE;
    let mut e = [0.0; 3];
    for j in 0..h {
        // Same mapping as `skyview_dir` in atmosphere.wgsl.
        let v = (j as f64 + 0.5) / h as f64 * 2.0 - 1.0;
        if v <= 0.0 {
            continue;
        }
        let el = v * v * PI / 2.0;
        let d_el = v * PI * 2.0 / h as f64;
        let d_az = 2.0 * PI / w as f64;
        let weight = el.sin() * el.cos() * d_el * d_az;
        for i in 0..w {
            let l = texels[(j * w + i) as usize];
            for k in 0..3 {
                e[k] += l[k] as f64 * weight / SCALE as f64;
            }
        }
    }
    e
}

#[test]
fn gpu_sky_matches_cpu_irradiance() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let mut sky = SkyRenderer::new(&ctx);
    let alt = 80.0;
    let mut report = String::new();
    let mut worst: f64 = 0.0;
    for elev in [60.0, 30.0, 10.0, 3.0, 0.0, -2.0, -4.0, -6.0, -8.0] {
        let sun = sun_at(elev);
        let gpu = gpu_irradiance(&ctx, &mut sky, sun, alt as f32);
        let cpu = hearth_env::sky::sky_irradiance(alt, sun, 1.0);
        let lux = |v: f64| v * hearth_env::sky::SUN_ILLUMINANCE;
        let ratio = gpu[1] / cpu[1].max(1e-12);
        report += &format!(
            "sun {elev:>5.1}°: GPU {:>10.4} lux  CPU {:>10.4} lux  ratio {ratio:.2}  (GPU b/g {:.2}, CPU b/g {:.2})\n",
            lux(gpu[1]),
            lux(cpu[1]),
            gpu[2] / gpu[1].max(1e-12),
            cpu[2] / cpu[1].max(1e-12),
        );
        worst = worst.max(ratio.ln().abs());
    }
    eprintln!("{report}");
    assert!(
        worst < 0.2f64.ln_1p(),
        "GPU and CPU skies disagree:\n{report}"
    );
}
