//! The smooth voxel world (Amendment S, `docs/spec/amendment-s-smooth-world.md`): every voxel of
//! the 1 m grid keeps its material and gains a signed 8-bit **fill**, the distance from its
//! centre to the ground's surface (positive inside, clamped to ±[`FILL_RANGE`] voxels), so the
//! grid describes the surface to about a centimetre. The surface is the fill's zero crossing.
//!
//! [`mesh`] turns a [`Field`] of fill samples into triangles with one of three dual methods
//! (S0's prototypes, `docs/design/smooth-terrain.md`): one vertex in each cell the surface
//! crosses, a quad around each grid edge whose two samples straddle it. Vertices sit at the
//! averaged edge crossings ([`Method::SurfaceNets`]), at a feature-preserving solve of the
//! crossings' planes ([`Method::DualContouring`], [`qef`]), or between the two by the
//! materials' sharpness ([`Method::SharpNets`]), so sand stays soft while rock keeps its edges.
//! Meshing a block of the world reads two samples of apron beyond it on every side
//! ([`APRON`]) and produces exactly the triangles meshing the whole world would, so meshes meet
//! without cracks wherever they are cut.

pub mod field;
pub mod mesh;
pub mod mesher;
pub mod qef;

pub use field::{FILL_RANGE, Field, dequantize, quantize, solid};
pub use mesh::{Check, Mesh};
pub use mesher::{APRON, MAX_BLEND, Materials, Method, Region, mesh};
pub use qef::Qef;
