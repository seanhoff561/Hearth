//! Every pipeline of the world's scene builds without a validation error (a shader that does
//! not compile, or a vertex layout its shader does not match, fails here rather than when the
//! game starts). Skipped when no adapter exists.

use hearth::scene::LocalWorld;
use hearth_math::PlanetSize;
use hearth_render::GpuContext;
use hearth_render::atlas::TextureArray;
use hearth_render::offscreen::OFFSCREEN_FORMAT;
use hearth_render::scene::SceneRenderer;

#[test]
fn the_scenes_pipelines_build() {
    let Ok(ctx) = GpuContext::headless(false).or_else(|_| GpuContext::headless(true)) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let world = LocalWorld::create(11, PlanetSize::Tiny, 256, None).expect("world");
    let atlas = TextureArray::from_entries(&hearth_texgen::textures_for(Some(&world.content)));
    let scope = ctx.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut scene = SceneRenderer::new(&ctx, &atlas, OFFSCREEN_FORMAT, *world.map.planet(), 2, 1);
    // A tree's mesh and an instance of it, as the tree pass takes them.
    let sp = hearth_flora::Templates::from_content(&world.content);
    let oak = sp.index_of("hearth:english_oak").unwrap_or(0);
    let sk =
        hearth_flora::template::variant_skeleton(&sp.species[oak], hearth_flora::Stage::Young, 1);
    let mesh = hearth_flora::mesh::mesh(
        &sk,
        hearth_flora::mesh::Options::new(
            hearth_flora::mesh::Detail::Reduced,
            sp.species[oak].form.leaf,
            false,
            sp.species[oak].form.foliage_density,
        ),
        1,
    );
    scene.trees.upload(&ctx, 1, &mesh);
    let instance = hearth_render::trees::TreeInstance {
        origin: [0.0, -2.0, -12.0],
        bark: 0x40_5060,
        turn: [1.0, 0.0, 0.0, 1.0],
        leaf: 0x30_6020 | 2 << 24,
        climate: 0,
        light: 15,
        pad: 0,
    };
    scene
        .trees
        .prepare(&ctx, &[(1, vec![instance])], glam::Vec3::new(4.0, 0.0, 1.0));
    let err = pollster::block_on(scope.pop());
    assert!(err.is_none(), "{err:?}");
    assert_eq!(scene.trees.stats.instances, 1);
    assert!(scene.trees.stats.triangles > 1000);
}
