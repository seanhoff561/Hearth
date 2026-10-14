//! Trees drawn as their meshes (S5): each tree's mesh stands where its blocks do, turned and
//! set off its block's middle as they are (`PlacedTree::frame`), so the trunk drawn and the
//! trunk walked into and chopped are one; a tree cut into is drawn as its blocks from then on.

use hearth::scene::LocalWorld;
use hearth::tree_draw::{self, MeshOf};
use hearth_flora::Stage;
use hearth_flora::mesh::{Detail, WOOD};
use hearth_math::PlanetSize;
use hearth_worldgen::cubegen::features::Remains;
use hearth_worldgen::vegetation::Vegetation;

#[test]
fn a_trees_mesh_stands_where_its_blocks_do() {
    let lw = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let wg = &lw.generator;
    let veg = Vegetation::default();
    let (x, z) = wg.terrain.find_spawn(false);
    let trees: Vec<_> = wg
        .features()
        .trees_in(wg, &veg, (x - 400, z - 400), (x + 400, z + 400))
        .into_iter()
        .filter(|t| t.remains == Remains::Living && t.stage >= Stage::Young)
        .take(40)
        .collect();
    assert!(trees.len() >= 10, "{} trees about the spawn", trees.len());
    for t in &trees {
        let mesh = tree_draw::mesh(
            &wg.forest,
            MeshOf::standing(t.species, t.stage, t.variant, t.remains),
            Detail::Low,
        );
        let (origin, turn) = t.frame();
        // The stem's foot as drawn: its wood within the foot's block.
        let foot: Vec<(f64, f64)> = mesh
            .vertices
            .iter()
            .filter(|v| v.normal[3] == WOOD && v.sway[3] == 0 && v.pos[1] < 0.9)
            .map(|v| {
                let [px, _, pz] = v.pos;
                (
                    origin[0] + (turn[0] * px + turn[1] * pz) as f64,
                    origin[2] + (turn[2] * px + turn[3] * pz) as f64,
                )
            })
            .collect();
        assert!(!foot.is_empty(), "a stem at the foot of {:?}", t.foot);
        let n = foot.len() as f64;
        let (cx, cz) = (
            foot.iter().map(|p| p.0).sum::<f64>() / n,
            foot.iter().map(|p| p.1).sum::<f64>() / n,
        );
        // The stem's foot as built: its wood blocks at the foot's height.
        let wood: Vec<_> = t
            .blocks()
            .filter(|(p, part)| p.y == t.foot[1] && !matches!(part, hearth_flora::Part::Leaves))
            .map(|(p, _)| p)
            .collect();
        assert!(!wood.is_empty(), "wood at the foot of {:?}", t.foot);
        let near = wood.iter().any(|p| {
            (cx - (p.x as f64 + 0.5)).abs() <= 0.75 && (cz - (p.z as f64 + 0.5)).abs() <= 0.75
        });
        assert!(
            near,
            "{:?}'s stem drawn at ({cx:.2}, {cz:.2}), its blocks at {:?} (turn {:?}, corner {})",
            t.foot, wood, t.turn, t.template.corner
        );
    }
}

/// A tree cut into is drawn as its blocks from then on (S5): a change reaching one of its blocks
/// marks it cut in the vegetation (as what it is, so the tree that grows there after is drawn
/// whole again) and its cubes are meshed again with all its blocks in them; whole, they are left
/// out of the cubes' meshes, its mesh drawn instead.
#[test]
fn a_tree_cut_into_is_meshed_as_its_blocks() {
    use hearth_flora::Part;
    use hearth_render::atlas::TextureArray;
    use hearth_render::mesh::MeshOptions;
    use hearth_render::models::BlockModels;
    let mut lw = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let (x, z) = lw.generator.terrain.find_spawn(false);
    let t = lw
        .generator
        .features()
        .trees_in(
            &lw.generator,
            &lw.vegetation,
            (x - 200, z - 200),
            (x + 200, z + 200),
        )
        .into_iter()
        .find(|t| t.remains == Remains::Living && t.stage >= Stage::Young && !t.understory)
        .expect("a tree near the spawn");
    let foot = hearth_math::BlockPos::new(t.foot[0], t.foot[1], t.foot[2]);
    lw.load_area(
        glam::DVec3::new(foot.x as f64 + 0.5, foot.y as f64, foot.z as f64 + 0.5),
        1,
        2,
        None,
    );
    let reg = lw.reg.clone();
    let atlas = TextureArray::from_entries(&hearth_texgen::textures_for(Some(&lw.content)));
    let models = BlockModels::build(&reg, &atlas);
    let quads = |lw: &LocalWorld, trees_as_blocks: bool| -> usize {
        let opts = MeshOptions {
            trees_as_blocks,
            ..Default::default()
        };
        lw.mesh(&models, &[foot.cube()], opts)
            .iter()
            .map(|m| m.quads.len() + m.models.len())
            .sum()
    };
    let (whole, as_blocks) = (quads(&lw, false), quads(&lw, true));
    assert!(
        whole < as_blocks,
        "its blocks left out: {whole} quads, {as_blocks} with them"
    );
    // A change that leaves its blocks as they grew cuts nothing.
    assert!(lw.cut_into(&[foot.up().up()]).is_empty());
    assert!(!t.is_cut(&lw.vegetation));
    // A stroke of the axe into its trunk above the foot.
    let (cut, _) = t
        .blocks()
        .find(|(p, part)| p.y == foot.y + 1 && !matches!(part, Part::Leaves))
        .expect("its trunk above its foot");
    lw.map.set_block(cut, hearth_world::BlockStateId::AIR, &reg);
    let cubes = lw.cut_into(&[cut]);
    assert!(t.is_cut(&lw.vegetation), "kept as cut");
    assert!(cubes.contains(&foot.cube()), "its cubes to be meshed again");
    let col = foot.cube().column();
    assert!(
        !lw.drawn_trees(col).iter().any(|d| d.foot == t.foot),
        "no longer drawn as its mesh"
    );
    let after = quads(&lw, false);
    assert!(
        after > whole,
        "its blocks meshed: {after} quads, {whole} while whole"
    );
    // Cut again: nothing more to do.
    assert!(lw.cut_into(&[cut]).is_empty());
}
