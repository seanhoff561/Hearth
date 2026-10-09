//! A small sharded, approximately-LRU cache for expensive pure computations (column samples,
//! cave systems). Values are `Arc`s so readers never hold a lock while using them.

use std::hash::Hash;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use rustc_hash::FxHashMap;

const SHARDS: usize = 32;

struct Shard<K, V> {
    map: FxHashMap<K, (Arc<V>, u64)>,
}

/// Sharded cache with approximate least-recently-used eviction.
pub struct Cache<K, V> {
    shards: Vec<Mutex<Shard<K, V>>>,
    capacity_per_shard: usize,
    clock: AtomicU64,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl<K: Hash + Eq + Copy, V> Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            shards: (0..SHARDS)
                .map(|_| {
                    Mutex::new(Shard {
                        map: FxHashMap::default(),
                    })
                })
                .collect(),
            capacity_per_shard: (capacity / SHARDS).max(4),
            clock: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    fn shard(&self, key: &K) -> &Mutex<Shard<K, V>> {
        use std::hash::Hasher;
        let mut h = rustc_hash::FxHasher::default();
        key.hash(&mut h);
        &self.shards[(h.finish() as usize >> 7) % SHARDS]
    }

    /// Returns the cached value or computes it (outside the lock) and inserts it. Concurrent
    /// misses for the same key may compute twice; results are identical because the
    /// computation is pure.
    pub fn get_or_insert_with(&self, key: K, compute: impl FnOnce() -> V) -> Arc<V> {
        let stamp = self.clock.fetch_add(1, Ordering::Relaxed);
        {
            let mut shard = self.shard(&key).lock();
            if let Some(entry) = shard.map.get_mut(&key) {
                entry.1 = stamp;
                self.hits.fetch_add(1, Ordering::Relaxed);
                return entry.0.clone();
            }
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let value = Arc::new(compute());
        let mut shard = self.shard(&key).lock();
        if shard.map.len() >= self.capacity_per_shard {
            // Evict the oldest quarter.
            let mut stamps: Vec<u64> = shard.map.values().map(|v| v.1).collect();
            let cut_idx = stamps.len() / 4;
            stamps.select_nth_unstable(cut_idx);
            let cut = stamps[cut_idx];
            shard.map.retain(|_, v| v.1 > cut);
        }
        shard
            .map
            .entry(key)
            .or_insert_with(|| (value.clone(), stamp))
            .0
            .clone()
    }

    pub fn len(&self) -> usize {
        self.shards.iter().map(|s| s.lock().map.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The sum of `f` over the values kept (their memory, E4.1 §4.7).
    pub fn sum(&self, f: impl Fn(&V) -> u64) -> u64 {
        self.shards
            .iter()
            .map(|s| s.lock().map.values().map(|(v, _)| f(v)).sum::<u64>())
            .sum()
    }

    /// (hits, misses) since creation.
    pub fn stats(&self) -> (u64, u64) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }

    pub fn clear(&self) {
        for s in &self.shards {
            s.lock().map.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caches_and_evicts() {
        let c: Cache<u32, u32> = Cache::new(64);
        let mut computed = 0;
        for k in 0..10u32 {
            c.get_or_insert_with(k, || {
                computed += 1;
                k * 2
            });
        }
        assert_eq!(*c.get_or_insert_with(3, || 999), 6);
        assert_eq!(computed, 10);
        for k in 0..10_000u32 {
            c.get_or_insert_with(k, || k);
        }
        assert!(c.len() <= 64 * 2, "bounded: {}", c.len());
    }
}
