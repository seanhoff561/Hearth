//! Every tree species drawn as the tree pass draws it (S5), against the sky of a temperate place
//! (10 °C on the year, 16 °C between its months, 800 mm of rain): the default test checks each
//! shows, its leaves gone in winter if it is deciduous and kept if not; `species_sheets` (run by
//! hand) writes `bench-out/trees/<species>.png`, a young, a mature and an old tree side by side
//! at one scale in spring, summer, autumn and winter, and `bench-out/trees/summer.png` and
//! `winter.png`, every species grown, one to a cell in the forest's order.

use std::path::PathBuf;

use glam::{DVec3, Vec3};
use hearth::tree_draw::{self, TreeLook};
use hearth_flora::Stage;
use hearth_flora::mesh::Detail;
use hearth_render::GpuContext;
use hearth_render::atlas::TextureArray;
use hearth_render::camera::Camera;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};
use hearth_render::scene::{Environment, SceneRenderer};
use hearth_worldgen::trees::Forest;

/// The seasons by the shader's year (0 the spring equinox): leaves new, full, turning, gone.
const SEASONS: [(&str, f32); 4] = [
    ("spring", 0.15),
    ("summer", 0.35),
    ("autumn", 0.64),
    ("winter", 0.85),
];
const AGES: [Stage; 3] = [Stage::Young, Stage::Mature, Stage::Old];

/// The trees' world without the world: the scene's sky, light and seasons, and the tree pass.
struct Studio {
    ctx: GpuContext,
    scene: SceneRenderer,
    forest: Forest,
    looks: Vec<TreeLook>,
    climate: u32,
    /// The meshes kept, by key, and their bounds (m).
    bounds: rustc_hash::FxHashMap<u64, (Vec3, Vec3)>,
}

/// Trees to draw in one picture: (species, stage, where its foot is, m).
type Stand = Vec<(usize, Stage, Vec3)>;

impl Studio {
    fn new() -> Option<Self> {
        let ctx = GpuContext::headless(false).ok()?;
        let content = hearth_content::Content::load_base();
        let reg = hearth_world::datapack::load_builtin_registry().expect("blocks");
        let textures = hearth_texgen::textures_for(Some(&content));
        let atlas = TextureArray::from_entries(&textures);
        let colors = hearth_lod::BlockColors::new(&reg, &textures);
        let forest = Forest::new(&reg, &content);
        let looks = tree_draw::looks(&forest, &colors);
        let planet = hearth_worldgen::WorldGenSettings {
            seed: 1,
            planet_size: hearth_math::PlanetSize::Earth,
            grid_resolution: 0,
        }
        .sanitized()
        .planet()
        .expect("planet");
        let scene = SceneRenderer::new(&ctx, &atlas, OFFSCREEN_FORMAT, planet, 2, 1);
        let climate =
            hearth_env::tint::encode(10.0, 16.0, 800.0, hearth_env::tint::DryType::None, false);
        Some(Self {
            ctx,
            scene,
            forest,
            looks,
            climate,
            bounds: Default::default(),
        })
    }

    /// A tree's mesh kept in the tree pass (made the first time), and its bounds.
    fn keep(&mut self, sp: usize, stage: Stage, detail: Detail) -> (Vec3, Vec3) {
        let key = tree_draw::mesh_key(sp, stage, 0, detail);
        if let Some(b) = self.bounds.get(&key) {
            return *b;
        }
        let mesh = tree_draw::mesh(&self.forest, sp, stage, 0, detail);
        self.scene.trees.upload(&self.ctx, key, &mesh);
        self.bounds.insert(key, (mesh.min, mesh.max));
        (mesh.min, mesh.max)
    }

    /// Lays trees side by side at their bounds' widths apart, centred on x = 0, the first on the
    /// picture's left (east, the camera looking south), and the camera back far enough to see
    /// them whole in a picture of `size`; the trees' meshes kept.
    fn frame(
        &mut self,
        trees: &[(usize, Stage)],
        detail: Detail,
        size: (u32, u32),
    ) -> (Stand, Camera) {
        let mut stand = Vec::new();
        let (mut x, mut top, mut deep) = (0.0f32, 1.0f32, 0.0f32);
        for &(sp, stage) in trees.iter().rev() {
            let (min, max) = self.keep(sp, stage, detail);
            let gap = 0.15 * (max.y - min.y).max(1.0);
            let foot = x - min.x + gap * 0.5;
            x = foot + max.x + gap * 0.5;
            top = top.max(max.y);
            deep = deep.max(-min.z).max(max.z);
            stand.push((sp, stage, Vec3::new(foot, 0.0, 0.0)));
        }
        let fov = 36.0f32;
        let tan = (fov.to_radians() * 0.5).tan();
        let aspect = size.0 as f32 / size.1 as f32;
        let eye = top * 0.47;
        // Back far enough for the tallest's top and the row's width, and the crowns' depth on.
        let reach = ((top * 1.06 - eye) / tan).max(x * 0.54 / (tan * aspect)) + deep;
        for t in &mut stand {
            t.2 += Vec3::new(-x * 0.5, 0.0, reach.max(4.0));
        }
        let camera = Camera {
            pos: DVec3::new(0.0, eye as f64, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            fov_y: fov,
            near: 0.5,
            ..Camera::default()
        };
        (stand, camera)
    }

    /// Draws a stand in a season (the shader's year, `SEASONS`) into `target`'s `size` pixels.
    fn draw(
        &mut self,
        stand: &Stand,
        camera: &Camera,
        detail: Detail,
        year: f32,
        target: &OffscreenTarget,
        size: (u32, u32),
    ) -> Vec<u8> {
        let mut batches: Vec<(u64, Vec<hearth_render::trees::TreeInstance>)> = Vec::new();
        for &(sp, stage, foot) in stand {
            let key = tree_draw::mesh_key(sp, stage, 0, detail);
            let origin = foot - camera.pos.as_vec3();
            let one = tree_draw::instance(self.looks[sp], origin, self.climate, 0, (15, 0), 0);
            match batches.iter_mut().find(|b| b.0 == key) {
                Some(b) => b.1.push(one),
                None => batches.push((key, vec![one])),
            }
        }
        self.scene.trees.prepare(&self.ctx, &batches, Vec3::ZERO);
        let env = Environment {
            sun_dir: Vec3::new(-0.45, 0.55, -0.7).normalize(),
            year_frac: year,
            wind: 0.0,
            wind_speed_m_s: 0.0,
            aerial_perspective: false,
            ..Environment::default()
        };
        self.scene
            .prepare(&self.ctx, camera, size, &env, f32::INFINITY);
        let mut enc = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.scene.render(
            &self.ctx,
            &mut enc,
            &target.color_view,
            &target.depth.view,
            size,
        );
        self.ctx.queue.submit([enc.finish()]);
        self.scene.submitted();
        target.read_rgba(&self.ctx)
    }
}

/// How many of a picture's pixels a stand covers: those that differ from the sky behind it.
fn covered(px: &[u8], sky: &[u8]) -> usize {
    px.chunks_exact(4)
        .zip(sky.chunks_exact(4))
        .filter(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .take(3)
                .map(|(p, q)| (*p as i32 - *q as i32).abs())
                .sum::<i32>()
                > 24
        })
        .count()
}

/// Copies a picture of `size` into a sheet `width` pixels wide at `at`.
fn paste(sheet: &mut [u8], width: u32, px: &[u8], size: (u32, u32), at: (u32, u32)) {
    for y in 0..size.1 {
        let from = (y * size.0 * 4) as usize;
        let to = (((at.1 + y) * width + at.0) * 4) as usize;
        sheet[to..to + (size.0 * 4) as usize]
            .copy_from_slice(&px[from..from + (size.0 * 4) as usize]);
    }
}

fn out_dir() -> PathBuf {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench-out/trees");
    std::fs::create_dir_all(&out).ok();
    out
}

#[test]
fn every_species_shows_and_keeps_its_leaves_as_it_should() {
    let Some(mut s) = Studio::new() else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let size = (192, 192);
    let target = OffscreenTarget::new(&s.ctx, size.0, size.1);
    let (summer, winter) = (SEASONS[1].1, SEASONS[3].1);
    let mut wrong = Vec::new();
    for sp in 0..s.forest.templates.species.len() {
        let id = s.forest.templates.species[sp].id.clone();
        let deciduous = s.forest.templates.species[sp].deciduous;
        // The leaves' tint says the same as the species: a deciduous tree's turns and falls.
        let tint_deciduous = (s.looks[sp].leaf >> 24) & 3 == 2;
        if deciduous != tint_deciduous {
            wrong.push(format!(
                "{id}: deciduous {deciduous}, its leaves' tint deciduous {tint_deciduous}"
            ));
        }
        let (stand, camera) = s.frame(&[(sp, Stage::Mature)], Detail::Reduced, size);
        let sky = s.draw(&Vec::new(), &camera, Detail::Reduced, summer, &target, size);
        let in_summer = covered(
            &s.draw(&stand, &camera, Detail::Reduced, summer, &target, size),
            &sky,
        );
        let sky = s.draw(&Vec::new(), &camera, Detail::Reduced, winter, &target, size);
        let in_winter = covered(
            &s.draw(&stand, &camera, Detail::Reduced, winter, &target, size),
            &sky,
        );
        let all = (size.0 * size.1) as f32;
        println!(
            "{id}: {:.1} % of the picture in summer, {:.1} % in winter{}",
            100.0 * in_summer as f32 / all,
            100.0 * in_winter as f32 / all,
            if deciduous { " (deciduous)" } else { "" }
        );
        // A pine or a palm framed whole is a narrow thing in a square picture.
        if (in_summer as f32) < all * 0.01 {
            wrong.push(format!("{id}: barely shows in summer ({in_summer} pixels)"));
        }
        let kept = in_winter as f32 / in_summer.max(1) as f32;
        if deciduous && kept > 0.8 {
            wrong.push(format!(
                "{id}: deciduous, yet keeps {:.0} % of its cover in winter",
                kept * 100.0
            ));
        }
        if !deciduous && kept < 0.85 {
            wrong.push(format!(
                "{id}: evergreen, yet keeps only {:.0} % of its cover in winter",
                kept * 100.0
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
#[ignore = "a review's tool: run by hand"]
fn species_sheets() {
    let Some(mut s) = Studio::new() else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let out = out_dir();
    let cell = (640, 420);
    let target = OffscreenTarget::new(&s.ctx, cell.0, cell.1);
    let n = s.forest.templates.species.len();
    // `HEARTH_SHEET_SPECIES=norway_spruce,english_oak`: those species' sheets alone.
    let only: Option<Vec<String>> = std::env::var("HEARTH_SHEET_SPECIES")
        .ok()
        .map(|v| v.split(',').map(|s| s.trim().to_owned()).collect());
    for sp in 0..n {
        let id = s.forest.templates.species[sp].id.clone();
        let name = id.rsplit(':').next().unwrap_or(&id).to_owned();
        if only.as_ref().is_some_and(|o| !o.contains(&name)) {
            continue;
        }
        let trees: Vec<(usize, Stage)> = AGES.iter().map(|&a| (sp, a)).collect();
        let (stand, camera) = s.frame(&trees, Detail::Full, cell);
        let (w, h) = (cell.0 * 2, cell.1 * 2);
        let mut sheet = vec![0u8; (w * h * 4) as usize];
        for (k, (_, year)) in SEASONS.iter().enumerate() {
            let px = s.draw(&stand, &camera, Detail::Full, *year, &target, cell);
            paste(
                &mut sheet,
                w,
                &px,
                cell,
                ((k as u32 % 2) * cell.0, (k as u32 / 2) * cell.1),
            );
        }
        write_png(&out.join(format!("{name}.png")), w, h, &sheet).expect("png");
        println!("{sp:2} {name}");
    }
    if only.is_some() {
        return;
    }
    // Every species grown, one to a cell, five to a row.
    let cell = (300, 300);
    let target = OffscreenTarget::new(&s.ctx, cell.0, cell.1);
    let rows = n.div_ceil(5) as u32;
    for (season, year) in [SEASONS[1], SEASONS[3]] {
        let (w, h) = (cell.0 * 5, cell.1 * rows);
        let mut sheet = vec![0u8; (w * h * 4) as usize];
        for sp in 0..n {
            let (stand, camera) = s.frame(&[(sp, Stage::Mature)], Detail::Full, cell);
            let px = s.draw(&stand, &camera, Detail::Full, year, &target, cell);
            paste(
                &mut sheet,
                w,
                &px,
                cell,
                ((sp as u32 % 5) * cell.0, (sp as u32 / 5) * cell.1),
            );
        }
        write_png(&out.join(format!("{season}.png")), w, h, &sheet).expect("png");
    }
}
