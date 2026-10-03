//! The people a screen shows (a birth's mother, child and father) drawn side by side over its
//! space in the interface: by the app each frame, and by the menus' test.

use hearth_character::{Drive, Figure, FigureInstance, Show, instances, starting_garbs};
use hearth_render::GpuContext;
use hearth_render::figure::FigurePreview;

use crate::menus::{PREVIEW_HEIGHT_M, PREVIEW_SPACING_M, Preview, preview_half_width};

/// The figures a preview shows, kept between frames so they go on breathing.
#[derive(Default)]
pub struct PeoplePreview {
    figures: Vec<Figure>,
    boxes: Vec<FigureInstance>,
    renderer: Option<FigurePreview>,
}

impl PeoplePreview {
    pub fn new() -> Self {
        Self::default()
    }

    /// Draws `p` over `target` (`size` pixels, `format`), its rectangle in interface units of
    /// `scale` pixels, the figures `dt` seconds on.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        ctx: &GpuContext,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        size: (u32, u32),
        scale: f32,
        p: &Preview,
        dt: f32,
    ) {
        let n = p.people.len();
        self.figures.truncate(n);
        while self.figures.len() < n {
            let a = p.people[self.figures.len()].clone();
            self.figures.push(Figure::starting(a));
        }
        let drive = Drive {
            breaths_per_min: 12.0,
            ..Default::default()
        };
        self.boxes.clear();
        for (i, (fig, a)) in self.figures.iter_mut().zip(&p.people).enumerate() {
            fig.set_appearance(a);
            fig.dress(&starting_garbs(a));
            let pose = fig.animator.update(&fig.rig, &drive, dt);
            let x = (i as f32 - (n as f32 - 1.0) / 2.0) * PREVIEW_SPACING_M;
            instances(
                &fig.rig,
                &fig.palette,
                &pose,
                glam::Affine3A::from_rotation_translation(
                    glam::Quat::from_rotation_y(p.yaw),
                    glam::Vec3::new(x, 0.0, 0.0),
                ),
                Show::default(),
                &mut self.boxes,
            );
        }
        let rect = [
            (p.rect.x * scale) as u32,
            (p.rect.y * scale) as u32,
            (p.rect.w * scale) as u32,
            (p.rect.h * scale) as u32,
        ];
        let r = self
            .renderer
            .get_or_insert_with(|| FigurePreview::new(ctx, format));
        // Framed for the tallest person, so heights compare, and wide enough for all.
        r.render(
            ctx,
            enc,
            target,
            size,
            rect,
            &self.boxes,
            PREVIEW_HEIGHT_M,
            preview_half_width(n),
            p.light,
        );
    }
}
