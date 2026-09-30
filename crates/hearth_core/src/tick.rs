//! Fixed-rate tick timing and scheduled (delayed) ticks.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::SECONDS_PER_TICK;

/// Converts variable frame time into a whole number of fixed ticks, with an interpolation
/// factor for rendering between ticks.
#[derive(Debug, Clone)]
pub struct FixedTimestep {
    accumulator: f64,
    tick_seconds: f64,
    /// Maximum ticks run per update; beyond this the simulation slows down instead of
    /// spiralling (the reference game's "can't keep up" behaviour).
    max_catch_up: u32,
    skipped_ticks: u64,
}

impl Default for FixedTimestep {
    fn default() -> Self {
        Self::new(SECONDS_PER_TICK, 10)
    }
}

impl FixedTimestep {
    pub fn new(tick_seconds: f64, max_catch_up: u32) -> Self {
        Self {
            accumulator: 0.0,
            tick_seconds,
            max_catch_up,
            skipped_ticks: 0,
        }
    }

    /// Adds elapsed real time and returns how many ticks to run now.
    pub fn advance(&mut self, dt_seconds: f64) -> u32 {
        self.accumulator += dt_seconds.max(0.0);
        let mut ticks = (self.accumulator / self.tick_seconds) as u64;
        if ticks > self.max_catch_up as u64 {
            self.skipped_ticks += ticks - self.max_catch_up as u64;
            ticks = self.max_catch_up as u64;
            self.accumulator = 0.0;
        } else {
            self.accumulator -= ticks as f64 * self.tick_seconds;
        }
        ticks as u32
    }

    /// Fraction (0..1) of the way from the last tick to the next — for interpolation.
    pub fn alpha(&self) -> f64 {
        (self.accumulator / self.tick_seconds).clamp(0.0, 1.0)
    }

    /// Ticks dropped because the simulation fell too far behind.
    pub fn skipped_ticks(&self) -> u64 {
        self.skipped_ticks
    }

    /// Seconds until the next tick is due.
    pub fn time_to_next_tick(&self) -> f64 {
        (self.tick_seconds - self.accumulator).max(0.0)
    }
}

/// A queue of payloads to process at a given game tick, ordered by (tick, priority, insertion).
#[derive(Debug, Clone)]
pub struct ScheduledTicks<T> {
    heap: BinaryHeap<Reverse<Entry<T>>>,
    seq: u64,
}

#[derive(Debug, Clone)]
struct Entry<T> {
    tick: u64,
    priority: i32,
    seq: u64,
    payload: T,
}

impl<T> PartialEq for Entry<T> {
    fn eq(&self, o: &Self) -> bool {
        (self.tick, self.priority, self.seq) == (o.tick, o.priority, o.seq)
    }
}
impl<T> Eq for Entry<T> {}
impl<T> PartialOrd for Entry<T> {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl<T> Ord for Entry<T> {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        (self.tick, self.priority, self.seq).cmp(&(o.tick, o.priority, o.seq))
    }
}

impl<T> Default for ScheduledTicks<T> {
    fn default() -> Self {
        Self {
            heap: BinaryHeap::new(),
            seq: 0,
        }
    }
}

impl<T> ScheduledTicks<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Schedules `payload` for `tick` (lower priority values run first within a tick).
    pub fn schedule(&mut self, tick: u64, priority: i32, payload: T) {
        self.seq += 1;
        self.heap.push(Reverse(Entry {
            tick,
            priority,
            seq: self.seq,
            payload,
        }));
    }

    /// Pops the next payload due at or before `now`, up to the caller's budget.
    pub fn pop_due(&mut self, now: u64) -> Option<T> {
        if self.heap.peek().is_some_and(|Reverse(e)| e.tick <= now) {
            self.heap.pop().map(|Reverse(e)| e.payload)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Removes all entries matching `pred` (e.g. when a cube unloads).
    pub fn retain(&mut self, mut pred: impl FnMut(&T) -> bool) {
        let entries = std::mem::take(&mut self.heap).into_vec();
        self.heap = entries
            .into_iter()
            .filter(|Reverse(e)| pred(&e.payload))
            .collect();
    }

    /// Iterates all pending entries as (tick, priority, payload), unordered — for saving.
    pub fn iter(&self) -> impl Iterator<Item = (u64, i32, &T)> {
        self.heap
            .iter()
            .map(|Reverse(e)| (e.tick, e.priority, &e.payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_timestep_counts_ticks() {
        let mut t = FixedTimestep::default();
        assert_eq!(t.advance(0.049), 0);
        assert_eq!(t.advance(0.002), 1);
        assert_eq!(t.advance(0.1), 2);
        assert!(t.alpha() < 1.0);
    }

    #[test]
    fn fixed_timestep_caps_catch_up() {
        let mut t = FixedTimestep::new(0.05, 10);
        assert_eq!(t.advance(5.0), 10);
        assert_eq!(t.skipped_ticks(), 90);
        assert_eq!(t.advance(0.0), 0);
    }

    #[test]
    fn scheduled_ticks_order() {
        let mut s = ScheduledTicks::new();
        s.schedule(5, 0, "late");
        s.schedule(2, 1, "early-low");
        s.schedule(2, 0, "early-high");
        s.schedule(2, 0, "early-high-2");
        assert_eq!(s.pop_due(1), None);
        assert_eq!(s.pop_due(2), Some("early-high"));
        assert_eq!(s.pop_due(2), Some("early-high-2"));
        assert_eq!(s.pop_due(2), Some("early-low"));
        assert_eq!(s.pop_due(4), None);
        s.retain(|p| *p != "late");
        assert!(s.is_empty());
    }
}
