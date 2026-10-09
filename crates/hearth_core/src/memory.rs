//! The caches' memory budget (E4.1 §4.7): together at most a share of the machine's memory
//! (forty per cent unless the options say otherwise), each kind of cache its part, so a cache
//! evicts harder rather than grows on a smaller machine.

use std::sync::atomic::{AtomicU64, Ordering};

/// The budget for every cache together (bytes), once set.
static TOTAL: AtomicU64 = AtomicU64::new(0);

/// The memory assumed where the system does not tell (bytes).
const UNKNOWN_RAM: u64 = 8 << 30;

/// The kinds of cache with a budget of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The terrain's refinement tiles, all levels.
    TerrainTiles,
    /// The generator's columns and rock.
    Columns,
    /// The neighbourhoods kept for the terrain's samples.
    Samples,
    /// The animals' regions set aside far from the player (beyond it, on disk).
    Animals,
}

impl Kind {
    /// Its share of the budget (per cent).
    fn percent(self) -> u64 {
        match self {
            Kind::TerrainTiles => 8,
            Kind::Columns => 5,
            Kind::Samples => 2,
            Kind::Animals => 2,
        }
    }

    /// Its bounds (bytes): enough to work at all, no more than it can use.
    fn bounds(self) -> (u64, u64) {
        match self {
            Kind::TerrainTiles => (48 << 20, 1 << 30),
            Kind::Columns => (32 << 20, 512 << 20),
            Kind::Samples => (16 << 20, 128 << 20),
            Kind::Animals => (16 << 20, 256 << 20),
        }
    }
}

/// Sets the budget: `share` per cent of the machine's memory (0: forty). Returns it (bytes).
pub fn init(share: u32) -> u64 {
    let (_, ram) = crate::prof::machine();
    let total = total_for(ram.unwrap_or(UNKNOWN_RAM), share);
    TOTAL.store(total, Ordering::Relaxed);
    log::info!(
        "caches' budget: {} MiB ({} % of the machine's memory)",
        total >> 20,
        if share == 0 { 40 } else { share.clamp(10, 80) }
    );
    total
}

fn total_for(ram: u64, share: u32) -> u64 {
    let share = if share == 0 { 40 } else { share.clamp(10, 80) };
    ram / 100 * share as u64
}

/// The budget for every cache together (bytes): as set, else forty per cent of the machine's.
pub fn total() -> u64 {
    match TOTAL.load(Ordering::Relaxed) {
        0 => {
            let (_, ram) = crate::prof::machine();
            total_for(ram.unwrap_or(UNKNOWN_RAM), 0)
        }
        t => t,
    }
}

/// The budget of a kind of cache (bytes).
pub fn budget(kind: Kind) -> u64 {
    budget_of(total(), kind)
}

fn budget_of(total: u64, kind: Kind) -> u64 {
    let (lo, hi) = kind.bounds();
    (total / 100 * kind.percent()).clamp(lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_follow_the_machine_within_bounds() {
        let gib = 1u64 << 30;
        // Forty per cent by default; as the options say otherwise.
        assert_eq!(total_for(16 * gib, 0), 16 * gib / 100 * 40);
        assert_eq!(total_for(16 * gib, 25), 16 * gib / 100 * 25);
        // A small machine's tiles evict harder; a large one's are kept, to a bound.
        let small = budget_of(total_for(4 * gib, 0), Kind::TerrainTiles);
        let large = budget_of(total_for(64 * gib, 0), Kind::TerrainTiles);
        assert!(small < large);
        assert!(small >= 48 << 20 && large <= gib);
        // Every kind together well within the whole.
        let total = total_for(8 * gib, 0);
        let sum: u64 = [
            Kind::TerrainTiles,
            Kind::Columns,
            Kind::Samples,
            Kind::Animals,
        ]
        .iter()
        .map(|k| budget_of(total, *k))
        .sum();
        assert!(sum < total / 4);
    }
}
