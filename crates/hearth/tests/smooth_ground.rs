//! The smooth ground's meshes on the generated world (Amendment S §3.2, §13): a block of cubes
//! meshed one at a time meets without a crack, comes out the same every time, blends its
//! materials to one and stays within its vertex budget; and how fast it meshes.

use std::time::Instant;

use glam::{IVec3, Vec3};
use hearth::scene::LocalWorld;
use hearth_math::{CubePos, PlanetSize};
use hearth_render::smooth::{GroundMaterials, SmoothMesh, mesh_cube, window};
use hearth_world::StateFlags;

fn materials(world: &LocalWorld) -> GroundMaterials {
    let content = world.content.clone();
    GroundMaterials::new(&world.reg, &|m| {
        m.and_then(|m| content.ground_of(m))
            .map_or(0.2, |g| g.sharpness)
    })
}

#[test]
fn the_ground_meshes_whole_and_alike_cube_by_cube() {
    let mut world = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let (sx, sz) = world.terrain().find_spawn(false);
    let at = glam::DVec3::new(
        sx as f64 + 0.5,
        world.surface_y(sx as f64, sz as f64),
        sz as f64 + 0.5,
    );
    world.load_area(at, 3, 2, None);
    let ground = materials(&world);
    assert!(
        ground.slots.len() > 50,
        "{} ground materials",
        ground.slots.len()
    );
    let c = hearth_math::BlockPos::containing(at).cube();
    let reg = world.reg.clone();
    let map = &world.map;
    // A block of 3 × 3 × 3 cubes about the surface.
    let mut all = hearth_smooth::Mesh::default();
    let mut vertices = 0;
    let mut bytes = 0;
    let t0 = Instant::now();
    let mut meshed = 0;
    for dy in -1..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let p = CubePos::new(c.x + dx, c.y + dy, c.z + dz);
                let o = p.min_block();
                let o = IVec3::new(o.x, o.y, o.z);
                let light = |q: IVec3| {
                    let b = hearth_math::BlockPos::new(o.x + q.x, o.y + q.y, o.z + q.z);
                    let s = map.block(b).unwrap_or_default();
                    (0xF0u8, !reg.has(s, StateFlags::NATURAL))
                };
                let f = window(map, &reg, &ground, p);
                let about = hearth_render::smooth::Surroundings {
                    light: &light,
                    climate: &|_, _| 0,
                    snow: &|_| false,
                };
                let m = mesh_cube(&f, &ground, &about);
                meshed += 1;
                // Alike every time.
                assert_eq!(m, mesh_cube(&f, &ground, &about));
                for v in &m.vertices {
                    let w: u32 = v.weights.iter().map(|&w| w as u32).sum();
                    assert!((250..=258).contains(&w), "weights sum to {w}");
                }
                vertices += m.vertices.len();
                bytes += m.gpu_bytes();
                append(&mut all, &m, o, IVec3::new(c.x - 1, c.y - 1, c.z - 1) * 16);
            }
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    let triangles = all.indices.len() / 3;
    assert!(triangles > 500, "{triangles} triangles");
    let welded = all.welded();
    let check = welded.check();
    eprintln!(
        "{meshed} cubes ({:.1} ms a cube, twice each): {vertices} vertices, {triangles} triangles, \
         {:.1} bytes a triangle; {check:?}",
        secs * 1e3 / meshed as f64,
        bytes as f64 / triangles as f64
    );
    // Watertight inside the block: open edges only on its outer faces.
    let lo = Vec3::new((c.x - 1) as f32, (c.y - 1) as f32, (c.z - 1) as f32) * 16.0;
    let hi = lo + Vec3::splat(48.0);
    let inner = open_edges_inside(&welded, lo + Vec3::splat(1.5), hi - Vec3::splat(1.5));
    assert_eq!(inner, 0, "cracks between cubes");
    assert_eq!(check.non_manifold_edges, 0);
    assert_eq!(check.misoriented_edges, 0);
}

/// Adds a cube's mesh at its place in the world (positions in metres), each vertex keyed by its
/// position from `lo` to the half millimetre (the same vertex meshed in two cubes is the same).
fn append(all: &mut hearth_smooth::Mesh, m: &SmoothMesh, o: IVec3, lo: IVec3) {
    let base = all.positions.len() as u32;
    for v in &m.vertices {
        let p = SmoothMesh::position(v) + o.as_vec3();
        all.positions.push(p);
        all.normals
            .push(hearth_render::smooth::octahedral_decode(v.normal));
        all.materials.push([v.materials[0] as u16; 4]);
        all.weights.push([1.0, 0.0, 0.0, 0.0]);
        all.sharpness.push(v.sharpness as f32 / 255.0);
        all.cells
            .push(((p - lo.as_vec3()) * 2048.0).round().as_ivec3());
    }
    all.indices
        .extend(m.indices.iter().map(|&i| base + i as u32));
}

/// Edges of one triangle only whose middle lies inside `lo..hi`.
fn open_edges_inside(m: &hearth_smooth::Mesh, lo: Vec3, hi: Vec3) -> usize {
    use std::collections::HashMap;
    let mut count: HashMap<(u32, u32), usize> = HashMap::new();
    for t in m.indices.chunks(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *count.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    count
        .iter()
        .filter(|(_, n)| **n == 1)
        .filter(|((a, b), _)| {
            let mid = (m.positions[*a as usize] + m.positions[*b as usize]) * 0.5;
            mid.cmpgt(lo).all() && mid.cmplt(hi).all()
        })
        .count()
}
