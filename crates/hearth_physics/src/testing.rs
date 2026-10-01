//! A small in-memory block world for tests and tools: solid cells, slabs, water, ice, snow and
//! ladders.

use std::collections::HashMap;

use glam::DVec3;
use hearth_math::{Aabb, BlockPos};

use crate::{Ground, Terrain};

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
    /// Passable foliage that slows a body by this much (0–1).
    Foliage(f64),
}

/// A small world of cells.
#[derive(Debug, Clone, Default)]
pub struct Grid {
    pub cells: HashMap<(i32, i32, i32), Cell>,
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
            _ => Ground::default(),
        }
    }

    fn climbable(&self, p: BlockPos) -> bool {
        self.at(p) == Some(Cell::Ladder)
    }

    fn drag(&self, p: BlockPos) -> f64 {
        match self.at(p) {
            Some(Cell::Foliage(d)) => d,
            _ => 0.0,
        }
    }
}
