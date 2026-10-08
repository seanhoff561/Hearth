//! A posed body as boxes for the GPU: one instance per box (a rounded part is three
//! overlapping boxes), each a placement of the unit cube and a colour.

use bytemuck::{Pod, Zeroable};
use glam::{Affine3A, Vec3};

use crate::animate::Pose;
use crate::appearance::{Appearance, linear_to_srgb, srgb_to_linear};
use crate::rig::{Region, Rig, Stuff};

/// A box: rows of its 3×4 placement (the unit cube −½..½ to camera-relative space), its
/// colour (sRGB, alpha unused), and the light where the body is (sky 0–15, block 0–15).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct FigureInstance {
    pub rows: [[f32; 4]; 3],
    pub color: [u8; 4],
    pub light: [u8; 4],
    pub pad: [u32; 2],
}

/// The colours of a person's stuff (sRGB).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub skin: [u8; 3],
    pub lips: [u8; 3],
    pub hair: [u8; 3],
    pub brow: [u8; 3],
    pub shadow: [u8; 3],
    pub sclera: [u8; 3],
    pub iris: [u8; 3],
    pub freckle: [u8; 3],
    pub cloth: [u8; 3],
}

impl Palette {
    pub fn of(a: &Appearance) -> Self {
        let skin = a.skin_linear();
        let hair = a.hair_linear();
        let mix = |x: [f32; 3], y: [f32; 3], t: f32| -> [f32; 3] {
            std::array::from_fn(|k| x[k] + (y[k] - x[k]) * t)
        };
        Self {
            skin: linear_to_srgb(skin),
            lips: linear_to_srgb([skin[0] * 0.8, skin[1] * 0.58, skin[2] * 0.6]),
            hair: a.hair_color,
            brow: linear_to_srgb(hair.map(|v| v * 0.8)),
            shadow: linear_to_srgb(mix(skin, hair, 0.45)),
            sclera: [232, 228, 220],
            iris: a.eyes.srgb(),
            // Freckles: the skin's own melanin, gathered (darker and warmer).
            freckle: linear_to_srgb([skin[0] * 0.62, skin[1] * 0.48, skin[2] * 0.38]),
            cloth: a.loincloth.srgb(),
        }
    }

    fn of_stuff(&self, s: Stuff) -> [u8; 3] {
        match s {
            Stuff::Skin => self.skin,
            Stuff::Lips => self.lips,
            Stuff::Hair => self.hair,
            Stuff::Brow => self.brow,
            Stuff::Shadow => self.shadow,
            Stuff::Sclera => self.sclera,
            Stuff::Iris => self.iris,
            Stuff::Freckle => self.freckle,
            Stuff::Cloth => self.cloth,
            Stuff::Dyed(c) => c,
        }
    }
}

/// What to leave out and how the body is lit.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Show {
    /// The head and neck (seen from inside them in first person).
    pub hide_head: bool,
    /// Sky light and block light (0–15) where the body is.
    pub sky_light: u8,
    pub block_light: u8,
    /// Only the garments' boxes (the body itself drawn as a sculpted mesh, E7).
    pub clothes_only: bool,
}

/// Appends the boxes of a posed body placed by `place` (the figure's frame to camera-relative
/// space).
pub fn instances(
    rig: &Rig,
    palette: &Palette,
    pose: &Pose,
    place: Affine3A,
    show: Show,
    out: &mut Vec<FigureInstance>,
) {
    let joints = pose.joints(rig);
    let light = [show.sky_light.min(15), show.block_light.min(15), 0, 0];
    let body: &[crate::rig::Part] = if show.clothes_only { &[] } else { &rig.parts };
    for p in body.iter().chain(&rig.clothes) {
        if show.hide_head && p.region == Region::Head {
            continue;
        }
        // A hood covers the hair (not the beard).
        if rig.hooded
            && p.joint == crate::rig::Joint::Head
            && matches!(p.stuff, Stuff::Hair | Stuff::Shadow)
            && p.center.y > 0.02 * rig.dims.stature
        {
            continue;
        }
        let m = place * joints[p.joint.index()];
        let [r, g, b] = palette.of_stuff(p.stuff);
        let color = [r, g, b, 255];
        let mut push = |size: Vec3| {
            let box_m =
                m * Affine3A::from_scale_rotation_translation(size, glam::Quat::IDENTITY, p.center);
            let c = box_m.matrix3;
            let t = box_m.translation;
            out.push(FigureInstance {
                rows: [
                    [c.x_axis.x, c.y_axis.x, c.z_axis.x, t.x],
                    [c.x_axis.y, c.y_axis.y, c.z_axis.y, t.y],
                    [c.x_axis.z, c.y_axis.z, c.z_axis.z, t.z],
                ],
                color,
                light,
                pad: [0; 2],
            });
        };
        let s = p.size;
        let r2 = (2.0 * p.round).min(s.x.min(s.y).min(s.z) * 0.6);
        if r2 > 0.0 {
            push(Vec3::new(s.x, s.y - r2, s.z - r2));
            push(Vec3::new(s.x - r2, s.y, s.z - r2));
            push(Vec3::new(s.x - r2, s.y - r2, s.z));
        } else {
            push(s);
        }
    }
}

/// The linear albedo of an instance's colour (for tests and tools).
pub fn albedo(i: &FigureInstance) -> [f32; 3] {
    srgb_to_linear([i.color[0], i.color[1], i.color[2]])
}

/// A box wearing a coat: the unit cube placed by `place`, its faces read from the figures'
/// coat texture at the place the two words give (`hearth_fauna::skin`: the unwrap's corner,
/// and its size in pixels with the top bit set), in the light where it is.
pub fn skinned(place: Affine3A, skin: [u32; 2], light: (u8, u8)) -> FigureInstance {
    let mut i = solid(place, [255, 255, 255], light);
    i.pad = skin;
    i
}

/// A plain box: the unit cube placed (and scaled) by `place` into camera-relative space, of a
/// colour (sRGB), in the light where it is (sky, block: 0–15).
pub fn solid(place: Affine3A, color: [u8; 3], light: (u8, u8)) -> FigureInstance {
    let c = place.matrix3;
    let t = place.translation;
    FigureInstance {
        rows: [
            [c.x_axis.x, c.y_axis.x, c.z_axis.x, t.x],
            [c.x_axis.y, c.y_axis.y, c.z_axis.y, t.y],
            [c.x_axis.z, c.y_axis.z, c.z_axis.z, t.z],
        ],
        color: [color[0], color[1], color[2], 255],
        light: [light.0.min(15), light.1.min(15), 0, 0],
        pad: [0; 2],
    }
}
