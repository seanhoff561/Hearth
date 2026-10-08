//! The interface's text on the GPU: distance-field glyphs land where the layout puts them, as
//! tall as they should be, with sharp edges and a shadow, over a cleared frame (written to
//! `bench-out/ui_text.png` to look at).

use hearth_render::GpuContext;
use hearth_render::offscreen::{OFFSCREEN_FORMAT, OffscreenTarget, write_png};
use hearth_render::ui::UiRenderer;
use hearth_ui::{DrawList, Font, Rgba};

#[test]
fn text_draws_crisp_ink_and_shadow() {
    let Ok(ctx) = GpuContext::headless(false) else {
        eprintln!("skipped: no GPU adapter");
        return;
    };
    let (w, h) = (480, 140);
    let target = OffscreenTarget::new(&ctx, w, h);
    let font = Font::new();
    let mut ui = UiRenderer::new(&ctx, OFFSCREEN_FORMAT, &font.pixels);
    let scale = 2;
    let mut list = DrawList::new(scale);
    list.rect(0.0, 0.0, 240.0, 70.0, Rgba::rgb(30, 60, 90));
    let lines = [
        "Hearth 0.1 · 1093 fps",
        "Body: satisfied, not thirsty, chilly, rested",
        "core 36.82 °C · wind 3.5 m/s · 12% in water",
        "Quick brown fox jumps over the lazy dog: g j p q y",
        "ÄBC àéèêçñöüß ¿¡ ±×— …",
    ];
    for (k, l) in lines.iter().enumerate() {
        list.text_shadowed(&font, l, 4.0, 3.0 + 12.0 * k as f32, Rgba::WHITE);
    }
    let mut enc = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ui test"),
        });
    {
        let _clear = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    ui.draw(&ctx, &mut enc, &target.color_view, (w, h), &list);
    ctx.queue.submit(Some(enc.finish()));
    let px = target.read_rgba(&ctx);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench-out");
    std::fs::create_dir_all(&out).ok();
    write_png(&out.join("ui_text.png"), w, h, &px).expect("png");
    let at = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2]]
    };
    let inked = |c: [u8; 3]| c[0] > 200 && c[1] > 200 && c[2] > 200;
    // The first glyph, 'H', at interface (4, 3): its ink's box on screen.
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 4..26 {
        for x in 6..24 {
            if inked(at(x, y)) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    assert!(x0 < x1 && y0 < y1, "no ink where the H should be");
    // Capitals 6.5 interface pixels tall: 13 screen pixels at scale 2, the top near y 7.
    let tall = y1 - y0 + 1;
    assert!((11..=15).contains(&tall), "the H is {tall} px tall");
    assert!((6..=9).contains(&y0), "the H's top at {y0}");
    // The left stem is ink down its height; between the stems above the bar, the panel.
    let stem = (y0..=y1)
        .filter(|&y| inked(at(x0, y)) || inked(at(x0 + 1, y)))
        .count();
    assert!(
        stem as u32 >= tall - 2,
        "the stem inked on {stem} of {tall} rows"
    );
    let mid = (x0 + x1) / 2;
    let gap = at(mid, y0 + 2);
    assert!(
        gap[2] < 140 && gap[0] < 100,
        "panel between the stems: {gap:?}"
    );
    // Sharp: from the panel to full ink across the stem's outer edge within three pixels.
    let row = (y0 + y1) / 2;
    let start = (x0.saturating_sub(4)..x0)
        .rfind(|&x| at(x, row)[0] < 60)
        .unwrap_or(x0 - 4);
    assert!(x0 - start <= 3, "the edge spreads over {} px", x0 - start);
    // The shadow, down and right of the ink: beside the stem, darker than the panel.
    let shadow = (x0 + 1..x0 + 5)
        .map(|x| at(x, y0 + 3))
        .find(|c| !inked(*c))
        .expect("the stem's right edge");
    assert!(shadow[2] < 70, "shadow {shadow:?}");
}
