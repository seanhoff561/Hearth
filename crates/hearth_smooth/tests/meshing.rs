//! The smooth meshers (S §13 "Meshing"): watertight and crack-free across blocks, deterministic,
//! material weights summing to one, soft ground soft and rock crisp.

use std::collections::HashMap;

use glam::{IVec3, Vec3};
use hearth_smooth::{
    FILL_RANGE, Field, MAX_BLEND, Mesh, Method, Region, dequantize, mesh, quantize,
};

const ROCK: u16 = 1;
const SAND: u16 = 2;
const SOIL: u16 = 3;

fn sharpness(m: u16) -> f32 {
    match m {
        ROCK => 1.0,
        SOIL => 0.2,
        _ => 0.0,
    }
}

/// Rolling ground with a knoll, an overhang, a floating boulder and a tunnel: every kind of cell a
/// block boundary can cut through.
fn rough(p: IVec3) -> (f32, u16) {
    let v = p.as_vec3();
    let ground = 22.0
        + 3.0 * (v.x * 0.21).sin() * (v.z * 0.17).cos()
        + 2.0 * (v.x * 0.07 + v.z * 0.11).sin()
        - v.y;
    let knoll = 9.0 - v.distance(Vec3::new(30.0, 22.0, 18.0));
    let boulder = 4.5 - v.distance(Vec3::new(14.3, 33.1, 30.7));
    let tunnel = 3.2 - Vec3::new(v.x - 24.0, (v.y - 17.0) * 1.3, 0.0).length();
    let d = ground.max(knoll).max(boulder).min(-tunnel);
    let m = if v.y > 26.0 {
        ROCK
    } else if v.y > 20.0 {
        SOIL
    } else {
        SAND
    };
    (d, m)
}

fn sphere_field() -> (Field, Vec3, f32) {
    let c = Vec3::new(12.2, 11.7, 12.4);
    let r = 7.3;
    let f = Field::from_fn(IVec3::ZERO, [24, 24, 24], |p| {
        (r - p.as_vec3().distance(c), ROCK)
    });
    (f, c, r)
}

#[test]
fn fill_values_round_trip() {
    for q in -127i8..=127 {
        assert_eq!(quantize(dequantize(q)), q);
    }
    assert_eq!(dequantize(-128), dequantize(-127));
    let step = FILL_RANGE / 127.0;
    for i in -300..=300 {
        let d = i as f32 * 0.0071;
        let back = dequantize(quantize(d));
        assert!(
            (back - d.clamp(-FILL_RANGE, FILL_RANGE)).abs() <= step * 0.5 + 1e-6,
            "{d} came back {back}"
        );
    }
    assert_eq!(quantize(f32::NAN), -127);
}

#[test]
fn a_sphere_meshes_closed_and_true() {
    let (field, c, r) = sphere_field();
    for method in Method::ALL {
        let m = mesh(&field, Region::interior(&field), method, &sharpness);
        let check = m.check();
        assert!(check.sound(), "{}: {check:?}", method.name());
        assert_eq!(check.boundary_edges, 0, "{}: {check:?}", method.name());
        let edges = check.triangles * 3 / 2;
        let euler = m.vertex_count() as i64 - edges as i64 + check.triangles as i64;
        assert_eq!(
            euler,
            2,
            "{}: a sphere's Euler characteristic",
            method.name()
        );
        let errors: Vec<f32> = m
            .positions
            .iter()
            .map(|p| (p.distance(c) - r).abs())
            .collect();
        let mean = errors.iter().sum::<f32>() / errors.len() as f32;
        let max = errors.iter().cloned().fold(0.0, f32::max);
        // The relaxation pulls a convex form in a little (about half a cell's sag a pass).
        let limit = if method == Method::SurfaceNets {
            0.08
        } else {
            0.03
        };
        assert!(
            mean < limit && max < 0.15,
            "{}: error mean {mean} max {max}",
            method.name()
        );
        for (p, n) in m.positions.iter().zip(&m.normals) {
            let true_n = (*p - c).normalize();
            assert!(n.dot(true_n) > 0.97, "{}: normal {n} at {p}", method.name());
        }
    }
}

#[test]
fn a_tilted_plane_comes_out_flat() {
    let n = Vec3::new(0.3, 1.0, 0.2).normalize();
    let o = Vec3::new(10.0, 9.37, 10.0);
    let field = Field::from_fn(IVec3::ZERO, [20, 20, 20], |p| {
        (-n.dot(p.as_vec3() - o), SOIL)
    });
    for method in Method::ALL {
        let m = mesh(&field, Region::interior(&field), method, &sharpness);
        assert!(!m.is_empty());
        assert!(m.check().sound(), "{}: {:?}", method.name(), m.check());
        let worst = m
            .positions
            .iter()
            .map(|p| n.dot(*p - o).abs())
            .fold(0.0, f32::max);
        assert!(worst < 0.03, "{}: {worst} off the plane", method.name());
        for v in &m.normals {
            assert!(v.dot(n) > 0.999, "{}: normal {v}", method.name());
        }
    }
}

/// The same surface meshed whole and in 16³ blocks, each with its own apron, comes out the same:
/// the blocks' vertices on their shared cells agree bit for bit and together they make exactly
/// the whole's triangles.
fn blocks_match_the_whole(field: &Field) {
    for method in Method::ALL {
        let whole = mesh(field, Region::interior(field), method, &sharpness);
        assert!(whole.triangle_count() > 500, "{}", whole.triangle_count());
        let mut blocks = Mesh::default();
        for by in 0..3 {
            for bz in 0..3 {
                for bx in 0..3 {
                    let lo = [bx, by, bz].map(|b| 16 * b);
                    let sub = field.sub(lo, [20, 20, 20]);
                    blocks.append(&mesh(&sub, Region::interior(&sub), method, &sharpness));
                }
            }
        }
        // Every copy of a cell's vertex agrees with the whole's, bit for bit.
        let mut by_cell = HashMap::new();
        for v in 0..whole.vertex_count() {
            by_cell.insert(whole.cells[v], v);
        }
        for v in 0..blocks.vertex_count() {
            let w = by_cell[&blocks.cells[v]];
            assert_eq!(blocks.positions[v], whole.positions[w], "{}", method.name());
            assert_eq!(blocks.normals[v], whole.normals[w], "{}", method.name());
            assert_eq!(blocks.weights[v], whole.weights[w], "{}", method.name());
            assert_eq!(blocks.materials[v], whole.materials[w], "{}", method.name());
        }
        let joined = blocks.welded();
        assert_eq!(
            joined.canonical_triangles(),
            whole.canonical_triangles(),
            "{}: the blocks' triangles differ from the whole's",
            method.name()
        );
        let (a, b) = (joined.check(), whole.check());
        assert_eq!(a.boundary_edges, b.boundary_edges, "{}", method.name());
        assert_eq!(
            a.non_manifold_edges,
            b.non_manifold_edges,
            "{}",
            method.name()
        );
    }
}

#[test]
fn blocks_mesh_without_cracks() {
    let n = 2 + 16 * 3 + 2;
    let field = Field::from_fn(IVec3::new(-20, -4, 7), [n, n, n], |p| {
        rough(p + IVec3::new(20, 4, -7))
    });
    blocks_match_the_whole(&field);
}

/// A lattice value in -1..1 for a seed (a small integer hash).
fn lattice(seed: u32, x: i32, y: i32, z: i32) -> f32 {
    let mut h = seed
        ^ (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 32767.5 - 1.0
}

/// Value noise: the lattice blended smoothly between its points, `cell` samples apart.
fn noise(seed: u32, p: IVec3, cell: i32) -> f32 {
    let f = |v: i32| v.div_euclid(cell);
    let t = |v: i32| {
        let u = v.rem_euclid(cell) as f32 / cell as f32;
        u * u * (3.0 - 2.0 * u)
    };
    let (x, y, z) = (f(p.x), f(p.y), f(p.z));
    let (tx, ty, tz) = (t(p.x), t(p.y), t(p.z));
    let l = |dx, dy, dz| lattice(seed, x + dx, y + dy, z + dz);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(l(0, 0, 0), l(1, 0, 0), tx);
    let x10 = lerp(l(0, 1, 0), l(1, 1, 0), tx);
    let x01 = lerp(l(0, 0, 1), l(1, 0, 1), tx);
    let x11 = lerp(l(0, 1, 1), l(1, 1, 1), tx);
    lerp(lerp(x00, x10, ty), lerp(x01, x11, ty), tz)
}

/// Fields of random noise (overhangs, floating lumps, tunnels, every material mixed): meshed
/// whole and in blocks, they agree for every method (S §13's edge-matching on randomized fields).
#[test]
fn random_fields_mesh_without_cracks() {
    let n = 2 + 16 * 3 + 2;
    for seed in [3u32, 17, 101, 9001] {
        let field = Field::from_fn(IVec3::new(5, -9, -30), [n, n, n], |p| {
            let ground = 26.0 - (p.y + 9) as f32;
            let d = ground * 0.15 + 3.0 * noise(seed, p, 7) + 1.5 * noise(seed ^ 0x55, p, 3);
            let m = 1 + (noise(seed ^ 0xaa, p, 5) * 1.5 + 1.5) as u16 % 3;
            (d, m)
        });
        blocks_match_the_whole(&field);
    }
}

#[test]
fn meshing_is_deterministic() {
    let field = Field::from_fn(IVec3::ZERO, [36, 40, 36], rough);
    for method in Method::ALL {
        let a = mesh(&field, Region::interior(&field), method, &sharpness);
        let b = mesh(&field, Region::interior(&field), method, &sharpness);
        assert_eq!(a, b, "{}", method.name());
    }
}

#[test]
fn material_weights_sum_to_one_heaviest_first() {
    let field = Field::from_fn(IVec3::ZERO, [36, 40, 36], rough);
    for method in Method::ALL {
        let m = mesh(&field, Region::interior(&field), method, &sharpness);
        let mut blended = 0;
        for (w, ids) in m.weights.iter().zip(&m.materials) {
            let sum: f32 = w.iter().sum();
            assert!((sum - 1.0).abs() < 1e-5, "{}: weights {w:?}", method.name());
            assert!(w.iter().all(|&x| (0.0..=1.0).contains(&x)));
            assert!(
                w.windows(2).all(|p| p[0] >= p[1]),
                "{}: {w:?}",
                method.name()
            );
            for k in 1..MAX_BLEND {
                if w[k] == 0.0 {
                    assert_eq!(ids[k], ids[0], "unused slots repeat the first");
                }
            }
            if w[1] > 0.0 {
                blended += 1;
            }
        }
        assert!(
            blended > 50,
            "{}: only {blended} vertices blend",
            method.name()
        );
    }
}

/// A ridge (two faces meeting at a right angle): sharp material keeps its crest on the crease
/// line, soft material rounds it off.
#[test]
fn rock_keeps_its_crest_and_sand_rounds_it() {
    let (cx, cy) = (12.37, 12.61);
    let ridge = |p: Vec3| {
        let d1 = -((p.x - cx) + (p.y - cy)) / 2f32.sqrt();
        let d2 = ((p.x - cx) - (p.y - cy)) / 2f32.sqrt();
        d1.min(d2)
    };
    let crest = |m: &Mesh| {
        // The vertices next to the crease line: how far inside the true surface they sit.
        m.positions
            .iter()
            .filter(|p| (p.x - cx).abs() < 1.0 && (p.y - cy).abs() < 1.5)
            .map(|p| ridge(*p))
            .fold(0.0f32, |a, d| a.max(d.abs()))
    };
    for (material, sharp) in [(ROCK, true), (SAND, false)] {
        let field = Field::from_fn(IVec3::ZERO, [24, 24, 24], |p| {
            (ridge(p.as_vec3()), material)
        });
        let m = mesh(
            &field,
            Region::interior(&field),
            Method::SharpNets,
            &sharpness,
        );
        assert!(m.check().sound(), "{:?}", m.check());
        let depth = crest(&m);
        if sharp {
            assert!(depth < 0.05, "rock's crest sits {depth} inside the ridge");
        } else {
            assert!(
                depth > 0.1,
                "sand's crest should round off, sits {depth} inside"
            );
        }
    }
    let field = Field::from_fn(IVec3::ZERO, [24, 24, 24], |p| (ridge(p.as_vec3()), SAND));
    let dc = mesh(
        &field,
        Region::interior(&field),
        Method::DualContouring,
        &sharpness,
    );
    assert!(crest(&dc) < 0.05, "dual contouring keeps every crease");
}
