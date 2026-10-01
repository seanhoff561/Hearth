//! The interface's text on the GPU: glyph ink lands where the layout puts it, at whole-pixel
//! scale, with its shadow, over a cleared frame (written to `bench-out/ui_text.png` to look at).

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
    // The first glyph, 'H', at interface (4, 3): its left stem is ink on screen pixels
    // (8..10, 6..20); the background panel shows between its stems.
    assert_eq!(at(8, 6), [255, 255, 255], "ink");
    assert_eq!(at(9, 19), [255, 255, 255], "ink, whole pixels");
    assert_eq!(at(12, 6), [30, 60, 90], "panel between the stems");
    // The shadow one interface pixel down and right of the stem's foot.
    let shadow = at(10, 20);
    assert!(shadow[0] < 40 && shadow[2] < 60, "shadow {shadow:?}");
}
