//! A small in-memory block world for tests and tools: solid cells, slabs, water, ice, snow and
//! ladders.

use std::collections::HashMap;

use glam::DVec3;
use hearth_math::{Aabb, BlockPos};

use crate::{Ground, Plant, Terrain};

/// What fills a cell of the grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cell {
    Solid,
    /// A box of this height (m) from the block's bottom.
    Slab(f64),
    Water,
    Ice,
    Snow,
    Ladder,
    /// Passable foliage filling the block that slows a body it covers by this much (0–1).
    Foliage(f64),
    /// A plant of this density (0–1) standing this high (m, 0–1) from the block's floor.
    Plant(f64, f64),
}

/// Smooth natural ground (Amendment S §8.1) as a height field: a plane of `base` height rising
/// `grade` (rise over run along x and z), with bumps of `bump` metres every `wavelength`, and a
/// cliff of `cliff.1` metres up wherever x passes `cliff.0`; its footing's `friction`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    pub base: f64,
    pub grade: (f64, f64),
    pub bump: f64,
    pub wavelength: f64,
    pub cliff: Option<(f64, f64)>,
    pub friction: f64,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            base: 0.0,
            grade: (0.0, 0.0),
            bump: 0.0,
            wavelength: 8.0,
            cliff: None,
            friction: 0.6,
        }
    }
}

impl Surface {
    /// The ground's height at a column.
    pub fn height(&self, x: f64, z: f64) -> f64 {
        let k = std::f64::consts::TAU / self.wavelength.max(0.1);
        let mut h = self.base
            + self.grade.0 * x
            + self.grade.1 * z
            + self.bump * (x * k).sin() * (z * k).sin();
        if let Some((x0, up)) = self.cliff
            && x > x0
        {
            h += up;
        }
        h
    }
}

/// A small world of cells.
#[derive(Debug, Clone, Default)]
pub struct Grid {
    pub cells: HashMap<(i32, i32, i32), Cell>,
    /// Smooth ground under the cells, if any.
    pub surface: Option<Surface>,
}

impl Grid {
    /// A stone floor (its top at y = 0) over a square.
    pub fn floor(r: i32) -> Self {
        let mut g = Grid::default();
        g.fill((-r, -1, -r), (r, -1, r), Cell::Solid);
        g
    }

    pub fn fill(&mut self, lo: (i32, i32, i32), hi: (i32, i32, i32), c: Cell) {
        for x in lo.0..=hi.0 {
            for y in lo.1..=hi.1 {
                for z in lo.2..=hi.2 {
                    self.cells.insert((x, y, z), c);
                }
            }
        }
    }

    pub fn at(&self, p: BlockPos) -> Option<Cell> {
        self.cells.get(&(p.x, p.y, p.z)).copied()
    }

    /// Smooth ground alone.
    pub fn smooth(surface: Surface) -> Self {
        Self {
            surface: Some(surface),
            ..Self::default()
        }
    }
}

impl Terrain for Grid {
    fn boxes(&self, area: &Aabb, out: &mut Vec<Aabb>) {
        let (lo, hi) = area.block_range();
        for x in lo.x..=hi.x {
            for y in lo.y..=hi.y {
                for z in lo.z..=hi.z {
                    let p = BlockPos::new(x, y, z);
                    match self.at(p) {
                        Some(Cell::Solid | Cell::Ice | Cell::Snow) => out.push(Aabb::block(p)),
                        Some(Cell::Slab(h)) => {
                            let b = p.as_dvec3();
                            out.push(Aabb::new(b, b + DVec3::new(1.0, h, 1.0)));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn water(&self, p: BlockPos) -> f64 {
        if self.at(p) == Some(Cell::Water) {
            1.0
        } else {
            0.0
        }
    }

    fn ground(&self, p: BlockPos) -> Ground {
        match self.at(p) {
            Some(Cell::Ice) => Ground {
                friction: 0.98,
                ..Ground::default()
            },
            Some(Cell::Snow) => Ground {
                cushion: 0.5,
                ..Ground::default()
            },
            _ => match &self.surface {
                Some(s) => Ground {
                    friction: s.friction,
                    ..Ground::default()
                },
                None => Ground::default(),
            },
        }
    }

    fn depth(&self, p: DVec3) -> Option<f64> {
        let s = self.surface.as_ref()?;
        let h = s.height(p.x, p.z);
        // Across a plane's slope the depth is the distance to it; into a cliff, the distance
        // from its face too.
        let slope = (s.grade.0 * s.grade.0 + s.grade.1 * s.grade.1).sqrt();
        let mut d = (h - p.y) / (1.0 + slope * slope).sqrt();
        if let Some((x0, _)) = s.cliff
            && d > 0.0
            && p.x > x0
            && p.y > h - s.cliff.map_or(0.0, |c| c.1)
        {
            d = d.min(p.x - x0);
        }
        Some(d)
    }

    fn climbable(&self, p: BlockPos) -> bool {
        self.at(p) == Some(Cell::Ladder)
    }

    fn plant(&self, p: BlockPos) -> Option<Plant> {
        match self.at(p) {
            Some(Cell::Foliage(drag)) => Some(Plant { drag, top: 1.0 }),
            Some(Cell::Plant(drag, top)) => Some(Plant { drag, top }),
            _ => None,
        }
    }
}
