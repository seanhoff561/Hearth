//! A container's grid: items placed by cell, either way round, never overlapping, never more than
//! the container holds; containers inside containers.

use hearth_content::schema::item::ContainerSpec;
use serde::{Deserialize, Serialize};

use crate::registry::Items;
use crate::stack::Stack;

/// An item in a grid: its top-left cell, and whether it lies turned a quarter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placed {
    pub x: u8,
    pub y: u8,
    #[serde(default)]
    pub turned: bool,
    pub stack: Stack,
}

/// Why a thing will not go somewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Misfit {
    /// Unknown to the content.
    Unknown,
    /// Out of the grid's bounds.
    Outside,
    /// Something is already there.
    Overlaps,
    /// More than the container holds.
    TooHeavy,
    /// It holds nothing (a water skin holds water, not things).
    NoRoom,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Container {
    pub items: Vec<Placed>,
}

/// The cells (x, y, w, h) an item of `footprint` covers at (x, y).
fn cells(footprint: (u8, u8), x: u8, y: u8, turned: bool) -> (u32, u32, u32, u32) {
    let (w, h) = if turned {
        (footprint.1, footprint.0)
    } else {
        footprint
    };
    (x as u32, y as u32, w as u32, h as u32)
}

fn overlap(a: (u32, u32, u32, u32), b: (u32, u32, u32, u32)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

impl Container {
    /// Everything in it (kg).
    pub fn mass(&self, items: &Items) -> f32 {
        self.items.iter().map(|p| p.stack.mass(items)).sum()
    }

    /// The cells an item covers.
    pub fn covers(&self, items: &Items, i: usize) -> Option<(u32, u32, u32, u32)> {
        let p = self.items.get(i)?;
        let k = p.stack.kind(items)?;
        Some(cells(k.footprint, p.x, p.y, p.turned))
    }

    /// The item covering a cell.
    pub fn at(&self, items: &Items, x: u8, y: u8) -> Option<usize> {
        (0..self.items.len()).find(|&i| {
            self.covers(items, i)
                .is_some_and(|c| overlap(c, (x as u32, y as u32, 1, 1)))
        })
    }

    /// Whether `stack` goes at (x, y) in a container of `spec`, leaving out the item `except`
    /// (the one being moved).
    pub fn fits(
        &self,
        items: &Items,
        spec: &ContainerSpec,
        stack: &Stack,
        x: u8,
        y: u8,
        turned: bool,
        except: Option<usize>,
    ) -> Result<(), Misfit> {
        let kind = stack.kind(items).ok_or(Misfit::Unknown)?;
        if spec.grid.0 == 0 || spec.grid.1 == 0 {
            return Err(Misfit::NoRoom);
        }
        let c = cells(kind.footprint, x, y, turned);
        if c.0 + c.2 > spec.grid.0 as u32 || c.1 + c.3 > spec.grid.1 as u32 {
            return Err(Misfit::Outside);
        }
        for i in 0..self.items.len() {
            if Some(i) == except {
                continue;
            }
            if self.covers(items, i).is_some_and(|o| overlap(o, c)) {
                return Err(Misfit::Overlaps);
            }
        }
        let leaving = except.map_or(0.0, |i| self.items[i].stack.mass(items));
        if self.mass(items) - leaving + stack.mass(items) > spec.max_kg + 1e-4 {
            return Err(Misfit::TooHeavy);
        }
        Ok(())
    }

    /// Puts `stack` at (x, y), or gives it back with why not.
    #[allow(clippy::result_large_err)]
    pub fn put(
        &mut self,
        items: &Items,
        spec: &ContainerSpec,
        stack: Stack,
        x: u8,
        y: u8,
        turned: bool,
    ) -> Result<(), (Stack, Misfit)> {
        match self.fits(items, spec, &stack, x, y, turned, None) {
            Ok(()) => {
                self.items.push(Placed {
                    x,
                    y,
                    turned,
                    stack,
                });
                Ok(())
            }
            Err(m) => Err((stack, m)),
        }
    }

    /// Puts `stack` wherever it goes: first onto stacks of its kind with room, then into the
    /// first free place either way round. Gives back what is left over.
    pub fn put_anywhere(
        &mut self,
        items: &Items,
        spec: &ContainerSpec,
        mut stack: Stack,
    ) -> Result<(), Stack> {
        let Some(kind) = stack.kind(items) else {
            return Err(stack);
        };
        let per_cell = kind.per_cell();
        let unit = kind.mass_kg;
        // Top up stacks of the same kind.
        if per_cell > 1 {
            for i in 0..self.items.len() {
                if stack.count == 0 {
                    break;
                }
                let p = &self.items[i];
                if !p.stack.joins(&stack) || p.stack.count >= per_cell {
                    continue;
                }
                let room_kg = (spec.max_kg - self.mass(items)).max(0.0);
                let by_weight = if unit > 0.0 {
                    (room_kg / unit + 1e-4).floor() as u16
                } else {
                    u16::MAX
                };
                let n = (per_cell - p.stack.count).min(stack.count).min(by_weight);
                self.items[i].stack.count += n;
                stack.count -= n;
            }
            if stack.count == 0 {
                return Ok(());
            }
        }
        // New places, a cell's worth at a time.
        while stack.count > 0 {
            let n = stack.count.min(per_cell);
            let mut part = stack.clone();
            part.count = n;
            let mut placed = false;
            'search: for turned in [false, true] {
                for y in 0..spec.grid.1 {
                    for x in 0..spec.grid.0 {
                        if self.fits(items, spec, &part, x, y, turned, None).is_ok() {
                            self.items.push(Placed {
                                x,
                                y,
                                turned,
                                stack: part.clone(),
                            });
                            placed = true;
                            break 'search;
                        }
                    }
                }
            }
            if !placed {
                return Err(stack);
            }
            stack.count -= n;
        }
        Ok(())
    }

    /// Takes an item out.
    pub fn take(&mut self, i: usize) -> Option<Stack> {
        (i < self.items.len()).then(|| self.items.remove(i).stack)
    }

    /// Takes `n` from a stack (all of it if `n` is as many as there are).
    pub fn take_some(&mut self, i: usize, n: u16) -> Option<Stack> {
        let p = self.items.get_mut(i)?;
        if n >= p.stack.count {
            return self.take(i);
        }
        p.stack.count -= n;
        let mut part = p.stack.clone();
        part.count = n.max(1);
        part.inside = None;
        Some(part)
    }
}
