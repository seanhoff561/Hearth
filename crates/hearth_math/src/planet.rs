//! The wrap-aware planet geometry. This is the **only** place that implements wrapping of the X
//! axis and the Mercator latitude mapping; every other system goes through [`Planet`].
//!
//! * X is longitude and wraps modulo the circumference `C`.
//! * Z is Mercator-projected latitude: φ(z) = gd(−z / R) with R = C / 2π, so North (−Z) is
//!   positive latitude. The world spans z ∈ [−C/2, C/2] (φ up to ±85.05°).
//! * Crossing a pole edge re-enters at X + C/2 with Z mirrored and heading reversed.

use std::f64::consts::{PI, TAU};

use glam::{DVec3, IVec3};
use serde::{Deserialize, Serialize};

use crate::coords::{BlockPos, CUBE_SIZE, ColumnPos, CubePos};
use crate::direction::Direction;

/// Circumferences must be a multiple of this (the largest LOD tile size), so tiles and regions
/// tile the planet exactly and the seam never splits one.
pub const CIRCUMFERENCE_ALIGN: i32 = 4096;
/// Smallest allowed circumference.
pub const MIN_CIRCUMFERENCE: i32 = 16_384;
/// Real Earth's equatorial circumference in metres, rounded to a multiple of 4096.
pub const EARTH_CIRCUMFERENCE: i32 = 40_075_264;
/// Real Earth's mean radius in metres (used for curvature at the equivalent real scale).
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;

/// Errors from constructing a planet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanetError {
    TooSmall(i32),
    Misaligned(i32),
}

impl std::fmt::Display for PlanetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanetError::TooSmall(c) => {
                write!(
                    f,
                    "circumference {c} is below the minimum {MIN_CIRCUMFERENCE}"
                )
            }
            PlanetError::Misaligned(c) => {
                write!(
                    f,
                    "circumference {c} is not a multiple of {CIRCUMFERENCE_ALIGN}"
                )
            }
        }
    }
}

impl std::error::Error for PlanetError {}

/// Planet size presets offered on the Create World screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanetSize {
    Tiny,
    Small,
    Standard,
    Large,
    Huge,
    Vast,
    Earth,
    Custom(i32),
}

impl PlanetSize {
    pub const PRESETS: [PlanetSize; 7] = [
        PlanetSize::Tiny,
        PlanetSize::Small,
        PlanetSize::Standard,
        PlanetSize::Large,
        PlanetSize::Huge,
        PlanetSize::Vast,
        PlanetSize::Earth,
    ];

    pub fn circumference(self) -> i32 {
        match self {
            PlanetSize::Tiny => 16_384,
            PlanetSize::Small => 32_768,
            PlanetSize::Standard => 65_536,
            PlanetSize::Large => 131_072,
            PlanetSize::Huge => 262_144,
            PlanetSize::Vast => 1_048_576,
            PlanetSize::Earth => EARTH_CIRCUMFERENCE,
            PlanetSize::Custom(c) => c,
        }
    }

    /// Auto vertical scale (blocks per real metre of relief). Presets use the table values;
    /// custom sizes use the same rule the table follows: v = min(1, sqrt(38.3 · C / C_earth)),
    /// which keeps horizontal compression always stronger than vertical compression.
    pub fn auto_vertical_scale(self) -> f64 {
        match self {
            PlanetSize::Tiny => 1.0 / 8.0,
            PlanetSize::Small => 1.0 / 6.0,
            PlanetSize::Standard => 1.0 / 4.0,
            PlanetSize::Large => 1.0 / 3.0,
            PlanetSize::Huge => 1.0 / 2.0,
            PlanetSize::Vast | PlanetSize::Earth => 1.0,
            PlanetSize::Custom(c) => auto_vertical_scale_for(c),
        }
    }

    /// Stable name for saves and translation keys.
    pub fn name(self) -> &'static str {
        match self {
            PlanetSize::Tiny => "tiny",
            PlanetSize::Small => "small",
            PlanetSize::Standard => "standard",
            PlanetSize::Large => "large",
            PlanetSize::Huge => "huge",
            PlanetSize::Vast => "vast",
            PlanetSize::Earth => "earth",
            PlanetSize::Custom(_) => "custom",
        }
    }

    /// Parses a preset name (`custom:<C>` for custom sizes).
    pub fn from_name(s: &str) -> Option<PlanetSize> {
        if let Some(c) = s.strip_prefix("custom:") {
            return c.parse().ok().map(PlanetSize::Custom);
        }
        Self::PRESETS.into_iter().find(|p| p.name() == s)
    }

    /// Approximate real-time hours to sprint once around the equator (5.612 m/s).
    pub fn sprint_hours_around(self) -> f64 {
        self.circumference() as f64 / 5.612 / 3600.0
    }

    /// Horizontal scale relative to Earth (e.g. 1/610 for Standard).
    pub fn horizontal_scale(self) -> f64 {
        self.circumference() as f64 / EARTH_CIRCUMFERENCE as f64
    }
}

/// Vertical scale rule for arbitrary circumferences (see [`PlanetSize::auto_vertical_scale`]).
pub fn auto_vertical_scale_for(circumference: i32) -> f64 {
    (38.3 * circumference as f64 / EARTH_CIRCUMFERENCE as f64)
        .sqrt()
        .min(1.0)
}

/// Where an entity ends up after walking past a pole edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoleCrossing {
    pub position: DVec3,
    /// Multiply horizontal velocity components by −1 and add 180° to yaw.
    pub reverse_heading: bool,
}

/// Wrap-aware planet geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Planet {
    circumference: i32,
}

impl Planet {
    pub fn new(circumference: i32) -> Result<Self, PlanetError> {
        if circumference < MIN_CIRCUMFERENCE {
            return Err(PlanetError::TooSmall(circumference));
        }
        if circumference % CIRCUMFERENCE_ALIGN != 0 {
            return Err(PlanetError::Misaligned(circumference));
        }
        Ok(Self { circumference })
    }

    pub fn from_size(size: PlanetSize) -> Result<Self, PlanetError> {
        Self::new(size.circumference())
    }

    #[inline]
    pub fn circumference(&self) -> i32 {
        self.circumference
    }

    #[inline]
    pub fn circumference_f64(&self) -> f64 {
        self.circumference as f64
    }

    /// Radius of the projection sphere in blocks (C / 2π).
    #[inline]
    pub fn radius(&self) -> f64 {
        self.circumference as f64 / TAU
    }

    /// Number of cube columns around the equator (C / 16).
    #[inline]
    pub fn cubes_around(&self) -> i32 {
        self.circumference / CUBE_SIZE
    }

    /// |z| of the pole edges (C / 2).
    #[inline]
    pub fn pole_edge_z(&self) -> f64 {
        self.circumference as f64 * 0.5
    }

    // ------------------------------------------------------------------ wrapping

    #[inline]
    pub fn wrap_x(&self, x: i32) -> i32 {
        x.rem_euclid(self.circumference)
    }

    #[inline]
    pub fn wrap_xf(&self, x: f64) -> f64 {
        let w = x.rem_euclid(self.circumference as f64);
        // rem_euclid can return exactly C for tiny negative inputs due to rounding.
        if w >= self.circumference as f64 {
            0.0
        } else {
            w
        }
    }

    #[inline]
    pub fn wrap_cube_x(&self, cx: i32) -> i32 {
        cx.rem_euclid(self.cubes_around())
    }

    #[inline]
    pub fn wrap_block(&self, p: BlockPos) -> BlockPos {
        BlockPos::new(self.wrap_x(p.x), p.y, p.z)
    }

    #[inline]
    pub fn wrap_cube(&self, c: CubePos) -> CubePos {
        CubePos::new(self.wrap_cube_x(c.x), c.y, c.z)
    }

    #[inline]
    pub fn wrap_column(&self, c: ColumnPos) -> ColumnPos {
        ColumnPos::new(self.wrap_cube_x(c.x), c.z)
    }

    /// Shortest signed X offset from `from` to `to` (in (−C/2, C/2]).
    #[inline]
    pub fn delta_x(&self, from: f64, to: f64) -> f64 {
        let c = self.circumference as f64;
        let mut d = (to - from).rem_euclid(c);
        if d > c * 0.5 {
            d -= c;
        }
        d
    }

    /// Shortest signed block X offset.
    #[inline]
    pub fn delta_block_x(&self, from: i32, to: i32) -> i32 {
        let c = self.circumference as i64;
        let mut d = (to as i64 - from as i64).rem_euclid(c);
        if d > c / 2 {
            d -= c;
        }
        d as i32
    }

    /// Shortest signed cube X offset.
    #[inline]
    pub fn delta_cube_x(&self, from: i32, to: i32) -> i32 {
        let n = self.cubes_around() as i64;
        let mut d = (to as i64 - from as i64).rem_euclid(n);
        if d > n / 2 {
            d -= n;
        }
        d as i32
    }

    /// Shortest wrapped offset vector from `from` to `to`.
    #[inline]
    pub fn delta(&self, from: DVec3, to: DVec3) -> DVec3 {
        DVec3::new(self.delta_x(from.x, to.x), to.y - from.y, to.z - from.z)
    }

    /// Wrap-aware cube offset from `from` to `to`.
    #[inline]
    pub fn cube_delta(&self, from: CubePos, to: CubePos) -> IVec3 {
        IVec3::new(
            self.delta_cube_x(from.x, to.x),
            to.y - from.y,
            to.z - from.z,
        )
    }

    /// The canonical neighbour cube in direction `d`.
    #[inline]
    pub fn cube_neighbor(&self, c: CubePos, d: Direction) -> CubePos {
        self.wrap_cube(c.offset(d))
    }

    /// The canonical cube at `c + o`.
    #[inline]
    pub fn cube_offset(&self, c: CubePos, o: IVec3) -> CubePos {
        self.wrap_cube(CubePos::new(c.x + o.x, c.y + o.y, c.z + o.z))
    }

    /// The canonical block at `b + o`.
    #[inline]
    pub fn block_offset(&self, b: BlockPos, o: IVec3) -> BlockPos {
        self.wrap_block(b + o)
    }

    /// Returns the copy of `p` (a continuous position) nearest to `reference`, i.e. `p` shifted
    /// by a multiple of C so it can be compared with `reference` without a seam.
    #[inline]
    pub fn unwrap_near(&self, p: DVec3, reference: DVec3) -> DVec3 {
        DVec3::new(reference.x + self.delta_x(reference.x, p.x), p.y, p.z)
    }

    // ------------------------------------------------------------------ geography

    /// Latitude in radians (north positive) at world Z.
    #[inline]
    pub fn latitude(&self, z: f64) -> f64 {
        (-z / self.radius()).sinh().atan()
    }

    /// Latitude in degrees.
    #[inline]
    pub fn latitude_deg(&self, z: f64) -> f64 {
        self.latitude(z).to_degrees()
    }

    /// World Z of a latitude (radians).
    #[inline]
    pub fn z_for_latitude(&self, lat: f64) -> f64 {
        -self.radius() * lat.tan().asinh()
    }

    /// Longitude in radians in [−π, π) at world X.
    #[inline]
    pub fn longitude(&self, x: f64) -> f64 {
        let l = self.wrap_xf(x) / self.circumference as f64 * TAU;
        if l >= PI { l - TAU } else { l }
    }

    #[inline]
    pub fn longitude_deg(&self, x: f64) -> f64 {
        self.longitude(x).to_degrees()
    }

    /// World X (canonical) of a longitude in radians.
    #[inline]
    pub fn x_for_longitude(&self, lon: f64) -> f64 {
        self.wrap_xf(lon / TAU * self.circumference as f64)
    }

    /// Unit vector on the sphere for a world position. The north pole is +Y; longitude 0 is +X.
    #[inline]
    pub fn sphere_point(&self, x: f64, z: f64) -> DVec3 {
        Self::sphere_point_from_lat_lon(self.latitude(z), self.longitude(x))
    }

    /// Unit vector for a latitude/longitude pair (radians).
    #[inline]
    pub fn sphere_point_from_lat_lon(lat: f64, lon: f64) -> DVec3 {
        let (sl, cl) = lat.sin_cos();
        let (so, co) = lon.sin_cos();
        DVec3::new(cl * co, sl, cl * so)
    }

    /// Inverse of [`Self::sphere_point`]: world (x, z) of a unit vector.
    pub fn world_xz_of_sphere_point(&self, p: DVec3) -> (f64, f64) {
        let lat = p.y.clamp(-1.0, 1.0).asin();
        let lon = p.z.atan2(p.x);
        (self.x_for_longitude(lon), self.z_for_latitude(lat))
    }

    /// Mercator scale factor at Z: world blocks per block of true surface distance
    /// (sec φ = cosh(z / R)). 1 at the equator, ≈11.6 at the pole edges.
    #[inline]
    pub fn mercator_scale(&self, z: f64) -> f64 {
        (z / self.radius()).cosh()
    }

    /// True surface distance (in equatorial-scale blocks) between two world points, measured
    /// on the sphere (great-circle).
    pub fn great_circle_distance(&self, a: (f64, f64), b: (f64, f64)) -> f64 {
        let pa = self.sphere_point(a.0, a.1);
        let pb = self.sphere_point(b.0, b.1);
        let dot = pa.dot(pb).clamp(-1.0, 1.0);
        dot.acos() * self.radius()
    }

    /// Resolves a position past a pole edge to the far side of the pole: X shifts by C/2, Z is
    /// mirrored at the edge and the heading reverses. Returns `None` if `p` is inside the world.
    pub fn cross_pole(&self, p: DVec3) -> Option<PoleCrossing> {
        let edge = self.pole_edge_z();
        let z = if p.z < -edge {
            -2.0 * edge - p.z
        } else if p.z > edge {
            2.0 * edge - p.z
        } else {
            return None;
        };
        Some(PoleCrossing {
            position: DVec3::new(self.wrap_xf(p.x + self.circumference as f64 * 0.5), p.y, z),
            reverse_heading: true,
        })
    }

    /// Fraction of a day (0..1) by which local solar time at `x` is ahead of the global clock.
    /// Noon at X = 0 is midnight at X = C/2.
    #[inline]
    pub fn solar_time_offset(&self, x: f64) -> f64 {
        self.wrap_xf(x) / self.circumference as f64
    }

    /// Effective curvature radius in blocks for horizon rendering: Earth's radius scaled by the
    /// world's vertical scale, so the horizon distance/dip from any altitude matches the real
    /// view from the equivalent real altitude.
    pub fn curvature_radius(vertical_scale: f64) -> f64 {
        EARTH_RADIUS_M * vertical_scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn std_planet() -> Planet {
        Planet::from_size(PlanetSize::Standard).unwrap()
    }

    #[test]
    fn construction_rules() {
        assert!(Planet::new(16_384).is_ok());
        assert_eq!(Planet::new(8192), Err(PlanetError::TooSmall(8192)));
        assert_eq!(Planet::new(20_000), Err(PlanetError::Misaligned(20_000)));
        for p in PlanetSize::PRESETS {
            assert!(Planet::from_size(p).is_ok(), "{p:?}");
            assert_eq!(PlanetSize::from_name(p.name()), Some(p));
        }
        assert_eq!(
            PlanetSize::from_name("custom:81920"),
            Some(PlanetSize::Custom(81920))
        );
    }

    #[test]
    fn vertical_scale_rule_matches_table() {
        for p in PlanetSize::PRESETS {
            let rule = auto_vertical_scale_for(p.circumference());
            let table = p.auto_vertical_scale();
            assert!(
                (rule / table).ln().abs() < 0.2,
                "{p:?}: rule {rule} vs table {table}"
            );
        }
    }

    #[test]
    fn standard_travel_time_is_about_three_hours() {
        let h = PlanetSize::Standard.sprint_hours_around();
        assert!((3.0..3.5).contains(&h), "{h}");
    }

    #[test]
    fn wrap_x_and_deltas() {
        let p = std_planet();
        let c = p.circumference();
        assert_eq!(p.wrap_x(-1), c - 1);
        assert_eq!(p.wrap_x(c), 0);
        assert_eq!(p.wrap_x(c * 3 + 5), 5);
        assert_eq!(p.delta_block_x(c - 16, 0), 16);
        assert_eq!(p.delta_block_x(0, c - 16), -16);
        assert_eq!(p.delta_cube_x(p.cubes_around() - 1, 0), 1);
        assert!((p.delta_x(c as f64 - 0.25, 0.25) - 0.5).abs() < 1e-9);
        assert!((p.wrap_xf(-1e-12)) < c as f64);
    }

    #[test]
    fn seam_neighbors() {
        let p = std_planet();
        let last = CubePos::new(p.cubes_around() - 1, 3, -2);
        let east = p.cube_neighbor(last, Direction::East);
        assert_eq!(east, CubePos::new(0, 3, -2));
        assert_eq!(p.cube_neighbor(east, Direction::West), last);
        assert_eq!(p.cube_delta(last, east), IVec3::new(1, 0, 0));
        let b = BlockPos::new(p.circumference() - 1, 5, 0);
        assert_eq!(
            p.block_offset(b, IVec3::new(1, 0, 0)),
            BlockPos::new(0, 5, 0)
        );
    }

    #[test]
    fn unwrap_near_reference() {
        let p = std_planet();
        let c = p.circumference() as f64;
        let q = p.unwrap_near(DVec3::new(2.0, 0.0, 0.0), DVec3::new(c - 3.0, 0.0, 0.0));
        assert!((q.x - (c + 2.0)).abs() < 1e-9);
    }

    #[test]
    fn mercator_round_trip() {
        let p = std_planet();
        for i in -80..=80 {
            let lat = (i as f64).to_radians();
            let z = p.z_for_latitude(lat);
            assert!((p.latitude(z) - lat).abs() < 1e-9, "lat {i}");
        }
        // North is -Z.
        assert!(p.latitude(-1000.0) > 0.0);
        // Pole edge is at ~85.05 degrees.
        let edge = p.latitude_deg(-p.pole_edge_z());
        assert!((edge - 85.051).abs() < 0.01, "{edge}");
        // Longitude round trip and wrap.
        for x in [0.0, 1.0, 1000.5, p.circumference_f64() - 0.5] {
            let lon = p.longitude(x);
            assert!((p.x_for_longitude(lon) - x).abs() < 1e-6, "{x}");
        }
    }

    #[test]
    fn sphere_point_round_trip() {
        let p = std_planet();
        for (x, z) in [(0.0, 0.0), (12345.0, -20000.0), (60000.0, 30000.0)] {
            let s = p.sphere_point(x, z);
            assert!((s.length() - 1.0).abs() < 1e-12);
            let (x2, z2) = p.world_xz_of_sphere_point(s);
            assert!((p.delta_x(x, x2)).abs() < 1e-6 && (z - z2).abs() < 1e-6);
        }
    }

    #[test]
    fn mercator_scale_is_sec_latitude() {
        let p = std_planet();
        let z = p.z_for_latitude(60f64.to_radians());
        assert!((p.mercator_scale(z) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn crossing_the_poles() {
        let p = std_planet();
        let edge = p.pole_edge_z();
        let c = p.circumference_f64();
        assert!(p.cross_pole(DVec3::new(100.0, 5.0, edge - 1.0)).is_none());
        let north = p.cross_pole(DVec3::new(100.0, 5.0, -edge - 2.0)).unwrap();
        assert!((north.position.z - (-edge + 2.0)).abs() < 1e-9);
        assert!((north.position.x - (100.0 + c / 2.0)).abs() < 1e-9);
        let south = p.cross_pole(DVec3::new(c - 10.0, 5.0, edge + 0.5)).unwrap();
        assert!((south.position.z - (edge - 0.5)).abs() < 1e-9);
        assert!((south.position.x - (c / 2.0 - 10.0)).abs() < 1e-6);
        // Crossing twice returns to the start.
        let back = p
            .cross_pole(DVec3::new(
                north.position.x,
                5.0,
                -north.position.z - 2.0 * edge,
            ))
            .unwrap();
        assert!((p.delta_x(back.position.x, 100.0)).abs() < 1e-6);
    }

    #[test]
    fn solar_offset_opposite_side() {
        let p = std_planet();
        assert!((p.solar_time_offset(p.circumference_f64() / 2.0) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn earth_curvature_radius() {
        assert!((Planet::curvature_radius(1.0) - EARTH_RADIUS_M).abs() < 1e-6);
        assert!((Planet::curvature_radius(0.25) - EARTH_RADIUS_M / 4.0).abs() < 1e-6);
    }
}
