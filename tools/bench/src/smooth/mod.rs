//! `bench smooth`: S0's smooth-terrain prototypes (Amendment S §3.1, §4.1). Builds the eight
//! test scenes as fill fields on the 1 m grid, meshes each with the three dual methods of
//! `hearth_smooth` (and the blocky grid of today for reference), measures them (size, speed on
//! one thread and in 16³ cubes on all threads, distance and normal error against the scene's
//! true surface, mesh defects, bytes per surface cube) and renders comparison sheets on the CPU;
//! then the material-blending and biplanar-shading prototype on turf and limestone.

mod raster;
mod scenes;
mod shading;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use glam::{DVec3, IVec3, Vec3};
use hearth_smooth::{APRON, Field, Mesh, Method, Region, mesh, quantize, solid};
use hearth_ui::font::{ATLAS, Face, Font};
use rayon::prelude::*;

use crate::image::Image;
use raster::{Camera, Look, ShadowMap, downsample, field_ao, rasterize, srgb_to_linear};
use scenes::{MATERIALS, SIZE, Scene, sharpness};
use shading::{Blend, Mapping};

/// Cube side the throughput is measured in (the game's cubes).
const CUBE: usize = 16;
/// Bytes a vertex takes at S2's compact layout (S §3.2's target), and an index (16-bit within a
/// cube).
const VERTEX_BYTES: usize = 24;
const INDEX_BYTES: usize = 2;

struct Options {
    out: PathBuf,
    only: Option<Vec<String>>,
    /// A camera of one's own (eye, target) instead of each scene's, for a close look.
    camera: Option<(Vec3, Vec3)>,
    /// A suffix for the sheets' names (with `camera`).
    tag: String,
    view: (usize, usize),
    supersample: usize,
    images: bool,
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let mut o = Options {
        out: PathBuf::from("bench-out/s0"),
        only: None,
        camera: None,
        tag: String::new(),
        view: (560, 350),
        supersample: 2,
        images: true,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || {
            it.next()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{a} needs a value"))
        };
        match a.as_str() {
            "--out" => o.out = PathBuf::from(val()?),
            "--scenes" => o.only = Some(val()?.split(',').map(str::to_owned).collect()),
            "--view" => {
                let v = val()?;
                let (w, h) = v
                    .split_once('x')
                    .ok_or_else(|| anyhow::anyhow!("--view WxH"))?;
                o.view = (w.parse()?, h.parse()?);
            }
            "--supersample" => o.supersample = val()?.parse::<usize>()?.clamp(1, 4),
            "--camera" => {
                // eye x,y,z and target x,y,z: "x,y,z:x,y,z"
                let v = val()?;
                let (e, t) = v
                    .split_once(':')
                    .ok_or_else(|| anyhow::anyhow!("--camera X,Y,Z:X,Y,Z"))?;
                let parse = |s: &str| -> anyhow::Result<Vec3> {
                    let c: Vec<f32> = s
                        .split(',')
                        .map(|p| p.trim().parse())
                        .collect::<Result<_, _>>()?;
                    anyhow::ensure!(c.len() == 3, "three coordinates");
                    Ok(Vec3::new(c[0], c[1], c[2]))
                };
                o.camera = Some((parse(e)?, parse(t)?));
            }
            "--tag" => o.tag = val()?,
            "--no-images" => o.images = false,
            other => anyhow::bail!("unknown argument {other}"),
        }
    }
    std::fs::create_dir_all(&o.out)?;
    let palette: Vec<Vec3> = MATERIALS.iter().map(|m| srgb_to_linear(m.color)).collect();
    let font = Font::new();
    let threads = rayon::current_num_threads();
    let mut rows = Vec::new();
    let mut shots = Vec::new();
    let mut described = Vec::new();
    for scene in scenes::all() {
        if o.only
            .as_ref()
            .is_some_and(|s| !s.iter().any(|k| k == scene.key))
        {
            continue;
        }
        described.push((scene.key, scene.title, scene.what));
        let t0 = Instant::now();
        let field = build_field(&scene);
        println!(
            "{}: field of {} samples in {:.2}s ({} near the surface)",
            scene.key,
            field.fills().len(),
            t0.elapsed().as_secs_f64(),
            field.unsaturated()
        );
        let mut views: Vec<(String, Mesh)> =
            vec![("Blocky (the grid today)".into(), blocky(&field))];
        for method in Method::ALL {
            let start = Instant::now();
            let m = mesh(&field, Region::interior(&field), method, &sharpness);
            let ms = start.elapsed().as_secs_f64() * 1e3;
            let row = measure(&scene, &field, &m, method, ms);
            println!(
                "  {:<30} {:>7} tris {:>8.1} ms  {:>7.0} cubes/s  err {:.2}/{:.2} cm  normals {:.1}°/{:.1}°  {:?} holes {}",
                method.name(),
                row.tris,
                row.ms,
                row.cubes.per_s(),
                row.err_mean_cm,
                row.err_p99_cm,
                row.face_mean_deg,
                row.face_p95_deg,
                row.defects,
                row.holes
            );
            rows.push(row);
            views.push((method.name().to_owned(), m));
        }
        if o.images {
            let path = render_sheet(&scene, &field, &views, &palette, &font, &o)?;
            println!("  wrote {}", path.display());
            shots.push(path);
        }
    }
    let mut shading = String::new();
    if o.images
        && o.only
            .as_ref()
            .is_none_or(|s| s.iter().any(|k| k == "shading"))
    {
        let (path, text) = shading_prototype(&font, &o)?;
        println!("{text}\n  wrote {}", path.display());
        shots.push(path);
        shading = text;
    }
    let report = report(&described, &rows, threads, &shading);
    let path = o.out.join("report.md");
    std::fs::write(&path, &report)?;
    println!("{report}\nwrote {}", path.display());
    Ok(())
}

/// The scene's fill and materials over its box with the meshers' apron around it.
fn build_field(scene: &Scene) -> Field {
    let size = SIZE.map(|s| s + 2 * APRON);
    let origin = IVec3::splat(-(APRON as i32));
    let layers: Vec<(Vec<i8>, Vec<u16>)> = (0..size[1])
        .into_par_iter()
        .map(|y| {
            let mut fill = Vec::with_capacity(size[0] * size[2]);
            let mut material = Vec::with_capacity(size[0] * size[2]);
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let p = origin + IVec3::new(x as i32, y as i32, z as i32);
                    let (d, m) = scene.sample(p.as_dvec3());
                    fill.push(quantize(d as f32));
                    material.push(m);
                }
            }
            (fill, material)
        })
        .collect();
    let (mut fill, mut material) = (Vec::new(), Vec::new());
    for (f, m) in layers {
        fill.extend(f);
        material.extend(m);
    }
    Field::from_parts(origin, size, fill, material).expect("the layers fill the field")
}

/// Today's look for reference: a unit face wherever a solid voxel meets an empty one (the
/// developer-only blocky view of S §3.3).
fn blocky(field: &Field) -> Mesh {
    let r = Region::interior(field);
    let mut m = Mesh::default();
    let origin = field.origin();
    // Outward normal, then two axes u, v with u × v = n (corners counter-clockwise from outside).
    let faces: [(IVec3, Vec3, Vec3); 6] = [
        (IVec3::X, Vec3::Y, Vec3::Z),
        (IVec3::NEG_X, Vec3::Z, Vec3::Y),
        (IVec3::Y, Vec3::Z, Vec3::X),
        (IVec3::NEG_Y, Vec3::X, Vec3::Z),
        (IVec3::Z, Vec3::X, Vec3::Y),
        (IVec3::NEG_Z, Vec3::Y, Vec3::X),
    ];
    for y in r.lo[1]..r.hi[1] {
        for z in r.lo[2]..r.hi[2] {
            for x in r.lo[0]..r.hi[0] {
                if !solid(field.fill(x, y, z)) {
                    continue;
                }
                let at = IVec3::new(x as i32, y as i32, z as i32);
                for (n, u, v) in faces {
                    let q = at + n;
                    if solid(field.fill(q.x as usize, q.y as usize, q.z as usize)) {
                        continue;
                    }
                    let centre = (origin + at).as_vec3() + n.as_vec3() * 0.5;
                    let base = m.positions.len() as u32;
                    for (su, sv) in [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
                        m.positions.push(centre + u * su + v * sv);
                        m.normals.push(n.as_vec3());
                        m.materials.push([field.material(x, y, z); 4]);
                        m.weights.push([1.0, 0.0, 0.0, 0.0]);
                        m.sharpness.push(1.0);
                        m.cells.push(origin + at);
                    }
                    m.indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base,
                        base + 2,
                        base + 3,
                    ]);
                }
            }
        }
    }
    m
}

/// Cubes meshed one by one with their aprons, as the game will.
#[derive(Default)]
struct Cubes {
    surface: usize,
    /// Best of five passes over all cubes on all threads.
    seconds: f64,
    /// Best of three passes on one thread.
    seconds_one: f64,
    bytes: usize,
}

impl Cubes {
    fn per_s(&self) -> f64 {
        self.surface as f64 / self.seconds.max(1e-9)
    }

    fn per_s_one(&self) -> f64 {
        self.surface as f64 / self.seconds_one.max(1e-9)
    }
}

fn cubes(field: &Field, method: Method) -> Cubes {
    let n = SIZE.map(|s| s / CUBE);
    let list: Vec<[usize; 3]> = (0..n[1])
        .flat_map(|y| (0..n[2]).flat_map(move |z| (0..n[0]).map(move |x| [x, y, z])))
        .collect();
    let one = |c: &[usize; 3]| -> Option<Mesh> {
        let sub = field.sub(c.map(|v| v * CUBE), [CUBE + 2 * APRON; 3]);
        let f = sub.fills();
        if f.iter().all(|&q| q > 0) || f.iter().all(|&q| q <= 0) {
            return None;
        }
        let m = mesh(&sub, Region::interior(&sub), method, &sharpness);
        (!m.is_empty()).then_some(m)
    };
    let mut out = Cubes {
        seconds: f64::INFINITY,
        ..Cubes::default()
    };
    for _ in 0..5 {
        let t = Instant::now();
        let meshes: Vec<Option<Mesh>> = list.par_iter().map(one).collect();
        out.seconds = out.seconds.min(t.elapsed().as_secs_f64());
        out.surface = meshes.iter().flatten().count();
        out.bytes = meshes
            .iter()
            .flatten()
            .map(|m| m.vertex_count() * VERTEX_BYTES + m.indices.len() * INDEX_BYTES)
            .sum();
    }
    out.seconds_one = f64::INFINITY;
    for _ in 0..3 {
        let t = Instant::now();
        for c in &list {
            std::hint::black_box(one(c));
        }
        out.seconds_one = out.seconds_one.min(t.elapsed().as_secs_f64());
    }
    out
}

struct Row {
    scene: &'static str,
    method: Method,
    verts: usize,
    tris: usize,
    ms: f64,
    err_mean_cm: f64,
    err_p99_cm: f64,
    face_mean_deg: f64,
    face_p95_deg: f64,
    vertex_mean_deg: f64,
    defects: hearth_smooth::Check,
    holes: usize,
    cubes: Cubes,
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}

fn measure(scene: &Scene, field: &Field, m: &Mesh, method: Method, ms: f64) -> Row {
    let mut err: Vec<f64> = m
        .positions
        .par_iter()
        .map(|p| scene.distance(p.as_dvec3()).0.abs() * 100.0)
        .collect();
    let err_mean = err.iter().sum::<f64>() / err.len().max(1) as f64;
    let mut face: Vec<f64> = (0..m.triangle_count())
        .into_par_iter()
        .filter_map(|t| {
            let [a, b, c] = m.triangle(t);
            let n = (b - a).cross(c - a).normalize_or_zero();
            if n == Vec3::ZERO {
                return None;
            }
            let truth = scene.distance(((a + b + c) / 3.0).as_dvec3()).1;
            Some(n.as_dvec3().dot(truth).clamp(-1.0, 1.0).acos().to_degrees())
        })
        .collect();
    let face_mean = face.iter().sum::<f64>() / face.len().max(1) as f64;
    let vertex: Vec<f64> = m
        .positions
        .par_iter()
        .zip(&m.normals)
        .map(|(p, n)| {
            let truth = scene.distance(p.as_dvec3()).1;
            n.as_dvec3().dot(truth).clamp(-1.0, 1.0).acos().to_degrees()
        })
        .collect();
    Row {
        scene: scene.key,
        method,
        verts: m.vertex_count(),
        tris: m.triangle_count(),
        ms,
        err_mean_cm: err_mean,
        err_p99_cm: percentile(&mut err, 0.99),
        face_mean_deg: face_mean,
        face_p95_deg: percentile(&mut face, 0.95),
        vertex_mean_deg: vertex.iter().sum::<f64>() / vertex.len().max(1) as f64,
        defects: m.check(),
        holes: holes(m),
        cubes: cubes(field, method),
    }
}

/// Open edges away from the mesh's border cells: holes or cracks in the surface.
fn holes(m: &Mesh) -> usize {
    let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
    for t in m.indices.chunks(3) {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *edges.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let lo = IVec3::splat(-1);
    let hi = IVec3::new(SIZE[0] as i32 - 1, SIZE[1] as i32 - 1, SIZE[2] as i32 - 1);
    let border = |v: u32| {
        let c = m.cells[v as usize];
        c.cmpeq(lo).any() || c.cmpeq(hi).any()
    };
    edges
        .iter()
        .filter(|&(&(a, b), &n)| n == 1 && !(border(a) && border(b)))
        .count()
}

fn render_sheet(
    scene: &Scene,
    field: &Field,
    views: &[(String, Mesh)],
    palette: &[Vec3],
    font: &Font,
    o: &Options,
) -> anyhow::Result<PathBuf> {
    let (vw, vh) = o.view;
    let ss = o.supersample;
    // Rows: the scene lit, a close look lit, the close look's triangles. With `--camera`, that
    // camera's view lit and its triangles.
    let cameras: Vec<(Camera, bool)> = match o.camera {
        Some((eye, target)) => {
            let c = Camera::look(eye, target, 55.0, vw * ss, vh * ss);
            vec![(c, false), (c, true)]
        }
        None => {
            let wide = Camera::look(
                scene.eye.as_vec3(),
                scene.target.as_vec3(),
                55.0,
                vw * ss,
                vh * ss,
            );
            let close = Camera::look(
                scene.close.0.as_vec3(),
                scene.close.1.as_vec3(),
                55.0,
                vw * ss,
                vh * ss,
            );
            vec![(wide, false), (close, false), (close, true)]
        }
    };
    let sun = scene.sun.as_vec3().normalize();
    let water = scene.water.map(|w| w as f32);
    let centre = Vec3::new(SIZE[0] as f32, SIZE[1] as f32, SIZE[2] as f32) * 0.5;
    let tiles: Vec<Vec<Vec<[u8; 3]>>> = views
        .par_iter()
        .map(|(_, m)| {
            let ao = field_ao(field, m);
            let shadow = ShadowMap::build(m, sun, centre, 85.0, 2048);
            cameras
                .iter()
                .map(|(cam, wire)| {
                    let vis = rasterize(cam, m);
                    let look = Look {
                        sun,
                        water,
                        palette,
                        wire: *wire,
                    };
                    let lin = raster::shade(cam, &vis, m, &ao, &shadow, &look);
                    downsample(&lin, cam.w, cam.h, ss)
                })
                .collect()
        })
        .collect();
    let rows = cameras.len();
    let mut sheet = Image::new(vw * views.len(), vh * rows);
    for (col, ((name, m), column)) in views.iter().zip(&tiles).enumerate() {
        for (row, tile) in column.iter().enumerate() {
            for y in 0..vh {
                for x in 0..vw {
                    sheet.set(col * vw + x, row * vh + y, tile[y * vw + x]);
                }
            }
        }
        let text = format!("{name}: {} triangles", m.triangle_count());
        label(&mut sheet, font, col * vw + 8, 8, &text, 2);
    }
    label(&mut sheet, font, 8, vh * rows - 24, scene.title, 2);
    let path = o.out.join(format!("{}{}.png", scene.key, o.tag));
    sheet.save(&path)?;
    Ok(path)
}

/// Text in the interface's typeface (its distance fields, thresholded), white over a dark
/// shadow.
fn label(img: &mut Image, font: &Font, x: usize, y: usize, text: &str, scale: usize) {
    let s = scale as f32;
    for (off, color) in [(1.0f32, [16u8, 16, 16]), (0.0, [250, 250, 250])] {
        let mut pen = x as f32;
        for c in text.chars() {
            if let Some(g) = font.glyph(c) {
                let (qx, qy) = (pen + (g.left + off) * s, y as f32 + (g.top + off) * s);
                let (qw, qh) = (
                    (g.width * s).ceil() as usize,
                    (g.height * s).ceil() as usize,
                );
                for py in 0..qh {
                    for px in 0..qw {
                        let tx = g.x as usize + px * g.w as usize / qw;
                        let ty = g.y as usize + py * g.h as usize / qh;
                        if font.pixels[ty * ATLAS as usize + tx] >= 128 {
                            img.set((qx + px as f32) as usize, (qy + py as f32) as usize, color);
                        }
                    }
                }
            }
            pen += font.advance_in(Face::Sans, c) * s;
        }
    }
}

/// Turf and limestone on the rolling hills, close up: linear against height blending, biplanar
/// against triplanar mapping, and how much the last two differ.
fn shading_prototype(font: &Font, o: &Options) -> anyhow::Result<(PathBuf, String)> {
    let scene = scenes::all().into_iter().next().expect("the scenes");
    let field = build_field(&scene);
    let m = mesh(
        &field,
        Region::interior(&field),
        Method::SharpNets,
        &sharpness,
    );
    let ao = field_ao(&field, &m);
    let sun = scene.sun.as_vec3().normalize();
    let centre = Vec3::new(SIZE[0] as f32, SIZE[1] as f32, SIZE[2] as f32) * 0.5;
    let shadow = ShadowMap::build(&m, sun, centre, 85.0, 2048);
    let (vw, vh) = o.view;
    let ss = o.supersample;
    let (eye, target) = shading_view(&scene, &field);
    let cam = Camera::look(eye, target, 55.0, vw * ss, vh * ss);
    let vis = rasterize(&cam, &m);
    let textures = [shading::turf(), shading::limestone()];
    let run = |mapping, blend| {
        let t = Instant::now();
        let (lin, samples) =
            shading::shade(&cam, &vis, &m, &ao, &shadow, sun, &textures, mapping, blend);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        (downsample(&lin, cam.w, cam.h, ss), samples, ms)
    };
    let (linear, _, _) = run(Mapping::Biplanar, Blend::Linear);
    let (bi, bi_samples, bi_ms) = run(Mapping::Biplanar, Blend::Height);
    let (tri, tri_samples, tri_ms) = run(Mapping::Triplanar, Blend::Height);
    let mut diff = Vec::with_capacity(bi.len());
    let mut total = 0u64;
    let mut over = 0usize;
    for (a, b) in bi.iter().zip(&tri) {
        let d = (0..3).map(|k| a[k].abs_diff(b[k])).max().unwrap_or(0);
        total += d as u64;
        if d > 4 {
            over += 1;
        }
        diff.push([(d as usize * 4).min(255) as u8; 3]);
    }
    let mut sheet = Image::new(vw * 2, vh * 2);
    let panels = [
        (&linear, "Linear blend, biplanar"),
        (&bi, "Height blend, biplanar"),
        (&tri, "Height blend, triplanar"),
        (&diff, "Biplanar vs triplanar, difference x4"),
    ];
    for (k, (img, name)) in panels.iter().enumerate() {
        let (ox, oy) = ((k % 2) * vw, (k / 2) * vh);
        for y in 0..vh {
            for x in 0..vw {
                sheet.set(ox + x, oy + y, img[y * vw + x]);
            }
        }
        label(&mut sheet, font, ox + 8, oy + 8, name, 2);
    }
    let path = o.out.join("shading.png");
    sheet.save(&path)?;
    let mut text = String::new();
    let _ = writeln!(
        text,
        "Shading prototype (turf and limestone, rolling hills, height blending): biplanar takes \
         {bi_samples:.2} texture samples a pixel in {bi_ms:.0} ms, triplanar {tri_samples:.2} in \
         {tri_ms:.0} ms (CPU, {}×{}); they differ by {:.2} levels of 255 on average, {:.2} % of \
         pixels by more than 4.",
        cam.w,
        cam.h,
        total as f64 / bi.len() as f64,
        over as f64 * 100.0 / bi.len() as f64
    );
    Ok((path, text))
}

/// A close view of the rolling hills where turf meets limestone: the scarp nearest the box's
/// middle, seen from 9 m away across it.
fn shading_view(scene: &Scene, field: &Field) -> (Vec3, Vec3) {
    let mut best: Option<(f64, DVec3, DVec3)> = None;
    for z in (20..76).step_by(2) {
        for x in (20..76).step_by(2) {
            // The surface height here and its slope.
            let top = (0..SIZE[1])
                .rev()
                .find(|&y| solid(field.fill(x + APRON, y + APRON, z + APRON)))
                .unwrap_or(0) as f64;
            let p = DVec3::new(x as f64, top, z as f64);
            let (_, n) = scene.distance(p);
            if n.y > 0.75 || n.y < 0.2 {
                continue;
            }
            let dist = (p - DVec3::new(48.0, top, 48.0)).length();
            if best.is_none_or(|b| dist < b.0) {
                best = Some((dist, p, n));
            }
        }
    }
    let (p, n) = best.map_or((DVec3::new(48.0, 32.0, 48.0), DVec3::X), |b| (b.1, b.2));
    let out = DVec3::new(n.x, 0.0, n.z).normalize_or(DVec3::X);
    let eye = p + out * 8.0 + DVec3::new(0.0, 3.0, 0.0) + out.cross(DVec3::Y) * 3.0;
    (eye.as_vec3(), (p + DVec3::Y * 0.5).as_vec3())
}

fn report(scenes: &[(&str, &str, &str)], rows: &[Row], threads: usize, shading: &str) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "## S0 mesher prototypes (`bench smooth`)\n\nEight scenes of 96 × 80 × 96 m on the 1 m grid. \
         Times on this machine's CPU: whole scene on one thread, and in 16³ cubes with a 2-voxel \
         apron on {threads} threads (best of five) and on one (best of three). Errors against each scene's true surface: \
         vertex distance, and the angle between each triangle's normal and the true one at its \
         centre. Bytes at S2's compact layout ({VERTEX_BYTES} B a vertex, {INDEX_BYTES} B an \
         index).\n"
    );
    let _ = writeln!(s, "| Scene | | What it tests |\n|---|---|---|");
    for (key, title, what) in scenes {
        let _ = writeln!(s, "| `{key}` | {title} | {what} |");
    }
    let _ = writeln!(s, "\n| Material | Sharpness |\n|---|---:|");
    for m in &MATERIALS[1..] {
        let _ = writeln!(s, "| {} | {:.2} |", m.name, m.sharpness);
    }
    let _ = writeln!(
        s,
        "\n| Scene | Method | Vertices | Triangles | Whole scene ms (1 thread) | Surface cubes/s ({threads} threads) | per thread | Distance error mean / p99 cm | Face normal error mean / p95 ° | Vertex normal error mean ° | Non-manifold edges | Folded | Holes | Bytes per surface cube |\n|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
    );
    for r in rows {
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {:.1} | {:.0} | {:.0} | {:.2} / {:.2} | {:.1} / {:.1} | {:.1} | {} | {} | {} | {:.0} |",
            r.scene,
            r.method.name(),
            r.verts,
            r.tris,
            r.ms,
            r.cubes.per_s(),
            r.cubes.per_s_one(),
            r.err_mean_cm,
            r.err_p99_cm,
            r.face_mean_deg,
            r.face_p95_deg,
            r.vertex_mean_deg,
            r.defects.non_manifold_edges,
            r.defects.folded,
            r.holes,
            r.cubes.bytes as f64 / r.cubes.surface.max(1) as f64
        );
    }
    let _ = writeln!(
        s,
        "\n| Method | Triangles (all scenes) | Surface cubes/s ({threads} threads) | per thread | Distance error mean cm | Face normal error mean ° | Non-manifold | Folded | Holes | Bytes per surface cube |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
    );
    for method in Method::ALL {
        let mine: Vec<&Row> = rows.iter().filter(|r| r.method == method).collect();
        if mine.is_empty() {
            continue;
        }
        let n = mine.len() as f64;
        let surface: usize = mine.iter().map(|r| r.cubes.surface).sum();
        let secs: f64 = mine.iter().map(|r| r.cubes.seconds).sum();
        let secs_one: f64 = mine.iter().map(|r| r.cubes.seconds_one).sum();
        let bytes: usize = mine.iter().map(|r| r.cubes.bytes).sum();
        let _ = writeln!(
            s,
            "| {} | {} | {:.0} | {:.0} | {:.2} | {:.1} | {} | {} | {} | {:.0} |",
            method.name(),
            mine.iter().map(|r| r.tris).sum::<usize>(),
            surface as f64 / secs.max(1e-9),
            surface as f64 / secs_one.max(1e-9),
            mine.iter().map(|r| r.err_mean_cm).sum::<f64>() / n,
            mine.iter().map(|r| r.face_mean_deg).sum::<f64>() / n,
            mine.iter()
                .map(|r| r.defects.non_manifold_edges)
                .sum::<usize>(),
            mine.iter().map(|r| r.defects.folded).sum::<usize>(),
            mine.iter().map(|r| r.holes).sum::<usize>(),
            bytes as f64 / surface.max(1) as f64
        );
    }
    if !shading.is_empty() {
        let _ = writeln!(s, "\n{shading}");
    }
    s
}
