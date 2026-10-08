//! Fill (Amendment S §2.1): how far each voxel's centre lies inside (+) or outside (−) the
//! ground's surface, in voxels, quantized to 8 bits over ±1.5 voxels. The surface is the zero
//! crossing, which lets the 1 m grid hold a surface to some centimetres.
//!
//! Only cubes the surface passes through keep a fill array; elsewhere a voxel's fill follows
//! from what it is: natural ground full, anything else empty (`Fill::of_state`). Anything but
//! natural ground is outside it; natural ground may be less than half full, its surface below
//! the voxel's centre (dug, or a thin layer). A voxel whose state changes takes the fill its
//! new state says (`UNSET`) until its fill is set.

use hearth_math::CUBE_VOLUME;

use crate::block::{BlockRegistry, BlockStateId, StateFlags};

/// The depth in voxels the 8 bits span either side of the surface.
pub const RANGE: f32 = 1.5;
/// Fully inside the ground, and fully out of it.
pub const FULL: i8 = 127;
pub const EMPTY: i8 = -127;
/// Stored for a voxel whose state changed and whose fill was not set since: its state says it.
pub const UNSET: i8 = i8::MIN;

/// One cube's fill, in index order (`y << 8 | z << 4 | x`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fill(pub Box<[i8; CUBE_VOLUME]>);

impl Fill {
    /// The quantized value of a depth in voxels (positive inside the ground).
    pub fn quantize(depth: f32) -> i8 {
        (depth / RANGE * FULL as f32)
            .round()
            .clamp(EMPTY as f32, FULL as f32) as i8
    }

    /// The depth in voxels of a quantized value.
    pub fn depth(q: i8) -> f32 {
        q as f32 / FULL as f32 * RANGE
    }

    /// The fill a voxel has from what it is alone.
    pub fn of_state(reg: &BlockRegistry, s: BlockStateId) -> i8 {
        if reg.has(s, StateFlags::NATURAL) {
            FULL
        } else {
            EMPTY
        }
    }

    /// A cube's fill from its states alone.
    pub fn of_states(reg: &BlockRegistry, states: &[BlockStateId]) -> Self {
        let mut f = Box::new([EMPTY; CUBE_VOLUME]);
        for (q, s) in f.iter_mut().zip(states) {
            *q = Self::of_state(reg, *s);
        }
        Fill(f)
    }

    /// A value made to agree with what the voxel is: a natural voxel inside the ground, any
    /// other outside, by at least the least step (the generator's rule: its states are ground
    /// where the centre is).
    pub fn agree(q: i8, natural: bool) -> i8 {
        if natural { q.max(1) } else { q.min(-1) }
    }

    /// A stored value read for a voxel of what it is now: unset, its state's; anything but
    /// natural ground outside.
    pub fn read(q: i8, natural: bool) -> i8 {
        match (q, natural) {
            (UNSET, true) => FULL,
            (UNSET, false) => EMPTY,
            (q, true) => q,
            (q, false) => q.min(-1),
        }
    }

    /// Whether every voxel is wholly inside or wholly outside: the surface does not pass
    /// through, and the states alone say the same.
    pub fn is_trivial(&self) -> bool {
        self.0.iter().all(|&q| q == FULL || q == EMPTY)
    }

    pub fn heap_bytes(&self) -> usize {
        CUBE_VOLUME
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantization_round_trips_within_a_step() {
        let step = RANGE / FULL as f32;
        for i in -300..=300 {
            let d = i as f32 * 0.005;
            let back = Fill::depth(Fill::quantize(d));
            let want = d.clamp(-RANGE, RANGE);
            assert!((back - want).abs() <= step * 0.5 + 1e-6, "{d}: {back}");
        }
        assert_eq!(Fill::quantize(9.0), FULL);
        assert_eq!(Fill::quantize(-9.0), EMPTY);
        // About a centimetre a step.
        assert!(step < 0.012);
    }

    #[test]
    fn agreement_keeps_the_sign_of_what_the_voxel_is() {
        assert_eq!(Fill::agree(-40, true), 1);
        assert_eq!(Fill::agree(40, true), 40);
        assert_eq!(Fill::agree(40, false), -1);
        assert_eq!(Fill::agree(0, false), -1);
    }
}
