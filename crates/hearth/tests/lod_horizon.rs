//! Distant terrain reaches the horizon (v1 §8): from a high point with LOD distance 512, a
//! meaningful share of the pixels below the horizon show terrain farther away than the
//! full-detail area (read back from the depth buffer), with aerial perspective on and off, and
//! nothing below the horizon is left empty. Skipped when no adapter exists.

use std::path::Path;

use glam::{Vec3, Vec4};
use hearth::scene::LocalWorld;
use hearth::screenshot::{ShotSpec, render_shot};
use hearth_math::PlanetSize;
use hearth_render::GpuContext;
use hearth_render::atlas::TextureArray;

#[test]
fn lod_terrain_reaches_the_horizon() {
    let Ok(ctx) = GpuContext::headless(false).or_else(|_| GpuContext::headless(true)) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let mut lw = LocalWorld::create(7, PlanetSize::Standard, 512, None).expect("world");
    let entries = hearth_texgen::textures_for(Some(&lw.content));
    let atlas = TextureArray::from_entries(&entries);
    let lod = hearth_lod::LodGen::new(&lw.reg, &entries);
    // A high point: the highest dry ground within two kilometres of the spawn.
    let (sx, sz) = lw.terrain().find_spawn(false);
    let mut best = (f32::MIN, sx, sz);
    for dz in (-2000..=2000).step_by(50) {
        for dx in (-2000..=2000).step_by(50) {
            let s = lw.terrain().sample(sx + dx, sz + dz);
            if !s.is_underwater() && s.height > best.0 {
                best = (s.height, sx + dx, sz + dz);
            }
        }
    }
    let (w, h) = (320u32, 180u32);
    for fog in [true, false] {
        let spec = ShotSpec {
            seed: 7,
            resolution: 512,
            x: Some(best.1 as f64 + 0.5),
            z: Some(best.2 as f64 + 0.5),
            above: 30.0,
            yaw: 45.0,
            pitch: 6.0,
            width: w,
            height: h,
            distance: 6,
            season: Some(hearth_content::schema::Season::Summer),
            clouds: Some(0.0),
            lod: 512,
            fog,
            ..ShotSpec::default()
        };
        lw.map = hearth_world::CubeMap::new(*lw.map.planet());
        let shot = render_shot(
            &ctx,
            &atlas,
            &lod,
            &mut lw,
            &spec,
            Path::new("unused.png"),
            None,
        )
        .expect("shot");
        assert!(
            shot.lod_tiles > 50,
            "only {} LOD tiles drawn",
            shot.lod_tiles
        );
        // Unproject every pixel to its view ray; reverse-Z depth is near / (distance along the
        // view axis), 0 for the sky.
        let cam = shot.camera;
        let inv = cam.view_proj(w as f32 / h as f32).inverse();
        let forward = cam.forward();
        let (mut below, mut far, mut empty) = (0usize, 0usize, 0usize);
        for py in 0..h {
            for px in 0..w {
                let ndc = Vec4::new(
                    (px as f32 + 0.5) / w as f32 * 2.0 - 1.0,
                    1.0 - (py as f32 + 0.5) / h as f32 * 2.0,
                    1.0,
                    1.0,
                );
                let p = inv * ndc;
                let dir = (Vec3::new(p.x, p.y, p.z) / p.w).normalize();
                // Rays a little below the horizontal still pass over the curved horizon.
                if dir.y > -0.03 {
                    continue;
                }
                below += 1;
                let depth = shot.depth[(py * w + px) as usize];
                if depth <= 0.0 {
                    empty += 1;
                    continue;
                }
                let distance = cam.near / depth / dir.dot(forward);
                if distance as f64 > shot.near_radius * 1.25 {
                    far += 1;
                }
            }
        }
        let share = far as f64 / below.max(1) as f64;
        eprintln!(
            "fog {fog}: {far} of {below} pixels below the horizon are beyond the full-detail area, {empty} empty; {} LOD tiles",
            shot.lod_tiles
        );
        let ok = below > 1000 && share > 0.3 && (empty as f64) < 0.002 * below as f64;
        if !ok {
            // Keep the frame and a map of the empty pixels for diagnosis.
            let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
            let mut mask = shot.pixels.clone();
            for (i, d) in shot.depth.iter().enumerate() {
                if *d <= 0.0 {
                    mask[i * 4..i * 4 + 4].copy_from_slice(&[255, 0, 255, 255]);
                }
            }
            let _ = hearth_render::offscreen::write_png(
                &dir.join(format!("lod_horizon_{fog}.png")),
                w,
                h,
                &shot.pixels,
            );
            let _ = hearth_render::offscreen::write_png(
                &dir.join(format!("lod_horizon_empty_{fog}.png")),
                w,
                h,
                &mask,
            );
        }
        assert!(below > 1000, "{below} pixels below the horizon");
        assert!(
            share > 0.3,
            "fog {fog}: only {:.0} % of the ground below the horizon is beyond the full-detail area",
            share * 100.0
        );
        assert!(
            (empty as f64) < 0.002 * below as f64,
            "fog {fog}: {empty} of {below} pixels below the horizon show no ground"
        );
    }
}
