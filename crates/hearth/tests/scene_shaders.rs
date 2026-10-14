//! Every pipeline of the scene builds: a shader that fails to build draws nothing, silently but
//! for its line in the log. The terrain, the distant terrain, the trees and the highlight's mask
//! they are drawn into, people, figures, the sky, water, the outline, post-processing.

use hearth_math::{Planet, PlanetSize};
use hearth_render::GpuContext;
use hearth_render::atlas::TextureArray;
use hearth_render::offscreen::OFFSCREEN_FORMAT;
use hearth_render::scene::{ANISOTROPY, MIP_LEVELS, SceneRenderer};

#[test]
fn every_pipeline_of_the_scene_builds() {
    let Ok(ctx) = GpuContext::headless(false) else {
        return;
    };
    let atlas = TextureArray::from_entries(&hearth_texgen::textures_for(None));
    let planet = Planet::from_size(PlanetSize::Tiny).expect("planet");
    let _scene = SceneRenderer::new(
        &ctx,
        &atlas,
        OFFSCREEN_FORMAT,
        planet,
        MIP_LEVELS,
        ANISOTROPY,
    );
    let _ = ctx.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    assert_eq!(
        ctx.uncaught_errors(),
        0,
        "a shader or pipeline failed to build (its wgpu error is in the log)"
    );
}
