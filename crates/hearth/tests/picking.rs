//! Picking what is looked at (Amendment P §5.1, T §2.3): the first thing along the look by its
//! own drawn shape — a plant only where its sprite is not clear, its fruit told from its leaves,
//! water at its surface, the ground's patch where a dig would take it — cheaply enough to run
//! every frame (under 0.2 ms).

use std::sync::Arc;
use std::time::Instant;

use glam::DVec3;
use hearth::aim::{Aim, Part, Shapes, pick_block};
use hearth_math::{BlockPos, CubePos, Planet, PlanetSize};
use hearth_render::atlas::TextureArray;
use hearth_world::{BlockRegistry, Cube, CubeMap, RenderLayer};

struct World {
    map: CubeMap,
    reg: BlockRegistry,
    shapes: Shapes,
    atlas: Arc<TextureArray>,
}

fn world() -> World {
    let content = hearth_content::Content::load_base();
    let reg = hearth_world::datapack::load_builtin_registry().expect("blocks");
    let atlas = Arc::new(TextureArray::from_entries(&hearth_texgen::textures_for(
        Some(&content),
    )));
    let shapes = Shapes::new(&reg, atlas.clone());
    let mut map = CubeMap::new(Planet::from_size(PlanetSize::Tiny).expect("planet"));
    let air = reg.default_state("hearth:air");
    for cx in -2..2 {
        for cz in -2..2 {
            for cy in -1..2 {
                map.insert_cube(CubePos::new(cx, cy, cz), Arc::new(Cube::filled(air)), &reg);
            }
        }
    }
    // Ground at y 0, a stone lying on it, a raspberry bush beside, a pool beyond.
    let grass = reg.default_state("hearth:grass_block");
    for x in -8..8 {
        for z in -8..8 {
            map.set_block(BlockPos::new(x, 0, z), grass, &reg);
        }
    }
    map.set_block(
        BlockPos::new(1, 1, 0),
        reg.default_state("hearth:granite_cobbles"),
        &reg,
    );
    map.set_block(
        BlockPos::new(0, 1, 2),
        reg.default_state("hearth:raspberry"),
        &reg,
    );
    map.set_block(
        BlockPos::new(-3, 0, 0),
        reg.default_state("hearth:water"),
        &reg,
    );
    World {
        map,
        reg,
        shapes,
        atlas,
    }
}

#[test]
fn the_first_thing_along_the_look_is_picked_quickly() {
    let w = world();
    let pick =
        |eye: DVec3, dir: DVec3| pick_block(&w.map, &w.reg, &w.shapes, (eye, dir), 2.6, &|_| 1.0);
    let eye = DVec3::new(0.5, 2.6, 0.5);
    // Down at the ground: the grass's top.
    let (_, hit) = pick(eye, DVec3::NEG_Y).expect("ground");
    assert!(
        matches!(hit, Aim::Block { pos, top: true, .. } if pos == BlockPos::new(0, 0, 0)),
        "{hit:?}"
    );
    // Toward the stone lying there: the stone, not the ground behind it.
    let to_stone = (DVec3::new(1.7, 1.1, 0.65) - eye).normalize();
    let (_, hit) = pick(eye, to_stone).expect("stone");
    assert!(
        matches!(hit, Aim::Block { pos, .. } if pos == BlockPos::new(1, 1, 0)),
        "{hit:?}"
    );
    // Level, at nothing within reach.
    assert!(pick(eye, DVec3::X).is_none());
    // Down into the pool: its surface, as water.
    let above = DVec3::new(-2.5, 2.0, 0.5);
    let (t, hit) = pick(above, DVec3::NEG_Y).expect("water");
    assert!(
        matches!(hit, Aim::Block { pos, part: Part::Water, at, .. }
            if pos == BlockPos::new(-3, 0, 0) && at.y > 0.8 && at.y < 1.0),
        "{hit:?} at {t}"
    );
    // Cost: the worst case goes the whole reach.
    let n = 20_000;
    let t0 = Instant::now();
    let mut hits = 0;
    for i in 0..n {
        let a = i as f64 * 0.37;
        let dir = DVec3::new(a.cos(), -0.05 - 0.4 * (i % 3) as f64, a.sin()).normalize();
        hits += pick(eye, dir).is_some() as usize;
    }
    let per = t0.elapsed().as_secs_f64() / n as f64 * 1e3;
    println!("{per:.4} ms a pick ({hits} hits)");
    assert!(per < 0.2, "{per} ms a pick");
}

/// A texel of a cutout quad of the block at `pos` whose alpha is `want`'s: its middle on the
/// quad (in the world) and the quad's normal.
fn texel_point(w: &World, pos: BlockPos, want: impl Fn(u8) -> bool) -> Option<(DVec3, DVec3)> {
    let s = w.map.block(pos)?;
    let corner = DVec3::new(pos.x as f64, pos.y as f64, pos.z as f64);
    let mut found = None;
    w.shapes.models.each_quad(s, |q, layer| {
        if found.is_some() || layer != RenderLayer::Cutout {
            return;
        }
        let texels = &w.atlas.layers[q.tex.tex.layer as usize];
        // Corners 0, 1 and 3 span the quad; its texels run 0..16 across them.
        let p = q.pos.map(|c| c.as_dvec3());
        let (u0, u1, v0, v3) = (q.uv[0].x, q.uv[1].x, q.uv[0].y, q.uv[3].y);
        // Away from the middle column, where the cross's other quad stands.
        for (ty, tx) in (0..16).flat_map(|y| [0, 1, 2, 3, 12, 13, 14, 15].map(|x| (y, x))) {
            if !want(texels[ty * 16 + tx][3]) {
                continue;
            }
            let su = ((tx as f32 + 0.5 - u0) / (u1 - u0)) as f64;
            let sv = ((ty as f32 + 0.5 - v0) / (v3 - v0)) as f64;
            let at = p[0] + (p[1] - p[0]) * su + (p[3] - p[0]) * sv;
            let normal = (p[1] - p[0]).cross(p[3] - p[0]).normalize();
            found = Some((corner + at, normal));
            return;
        }
    });
    found
}

/// Whether a state's own quads show any texel of fruit or flowers.
fn has_fruit(w: &World, s: hearth_world::BlockStateId) -> bool {
    let mut fruit = false;
    w.shapes.models.each_quad(s, |q, _| {
        fruit |= w.atlas.layers[q.tex.tex.layer as usize]
            .iter()
            .any(|t| t[3] == hearth_texgen::PART_ALPHA);
    });
    fruit
}

#[test]
fn a_plant_is_met_where_it_is_seen_and_its_fruit_apart() {
    let mut w = world();
    // A plant drawn with berries in its sprite, short (all of it in one block), away from the
    // rest.
    let bush = BlockPos::new(2, 1, -3);
    let berry = [
        "hearth:bilberry",
        "hearth:lingonberry",
        "hearth:wild_strawberry",
    ]
    .iter()
    .map(|n| w.reg.default_state(n))
    .find(|&s| has_fruit(&w, s))
    .expect("a plant with berries drawn");
    w.map.set_block(bush, berry, &w.reg);
    let pick =
        |eye: DVec3, dir: DVec3| pick_block(&w.map, &w.reg, &w.shapes, (eye, dir), 2.6, &|_| 1.0);
    // Looked at square on, from a metre or so off.
    let look_at = |(p, n): (DVec3, DVec3)| {
        let eye = p + n * 1.2;
        (eye, (p - eye).normalize())
    };
    let fruit = texel_point(&w, bush, |a| a == hearth_texgen::PART_ALPHA).expect("a berry");
    let (eye, dir) = look_at(fruit);
    let (_, hit) = pick(eye, dir).expect("the berries");
    assert!(
        matches!(hit, Aim::Block { pos, part: Part::Fruit, .. } if pos == bush),
        "{hit:?}"
    );
    let leaf = texel_point(&w, bush, |a| a == 255).expect("a leaf");
    let (eye, dir) = look_at(leaf);
    let (_, hit) = pick(eye, dir).expect("the leaves");
    assert!(
        matches!(hit, Aim::Block { pos, part: Part::Whole, .. } if pos == bush),
        "{hit:?}"
    );
    // Through a clear texel the look goes on: never met there.
    let gap = texel_point(&w, bush, |a| a == 0).expect("a clear texel");
    let (eye, dir) = look_at(gap);
    if let Some((_, Aim::Block { pos, at, .. })) = pick(eye, dir) {
        assert!(
            pos != bush || (at - gap.0).length() > 0.01,
            "met at the clear texel"
        );
    }
}

/// Bare in winter, a deciduous crown is looked through as it is drawn: the leaves that have
/// fallen (`common.wgsl`'s pattern) are not met, the twigs left among them are.
#[test]
fn fallen_leaves_are_looked_through() {
    let mut w = world();
    let leaves = BlockPos::new(3, 3, 3);
    w.map
        .set_block(leaves, w.reg.default_state("hearth:beech_leaves"), &w.reg);
    // A grid of looks into the block from its south side, level.
    let looks: Vec<(DVec3, DVec3)> = (0..12)
        .flat_map(|i| (0..12).map(move |j| (i, j)))
        .map(|(i, j)| {
            let target = DVec3::new(3.04 + i as f64 * 0.08, 3.04 + j as f64 * 0.08, 3.5);
            let eye = target + DVec3::new(0.0, 0.0, 2.0);
            (eye, DVec3::NEG_Z)
        })
        .collect();
    let met = |cover: f64| {
        looks
            .iter()
            .filter(|(eye, dir)| {
                matches!(pick_block(&w.map, &w.reg, &w.shapes, (*eye, *dir), 2.6, &|_| cover),
                    Some((_, Aim::Block { pos, .. })) if pos == leaves)
            })
            .count()
    };
    let (full, bare) = (met(1.0), met(0.0));
    println!(
        "{full} of {} looks met the leaves in leaf, {bare} bare",
        looks.len()
    );
    assert!(full > looks.len() / 3, "in leaf: {full}");
    // The twigs left are some sixth of the texels: far fewer met, but some.
    assert!(bare * 3 < full && bare > 0, "bare: {bare} of {full}");
}
