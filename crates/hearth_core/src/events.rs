//! Simple double-buffered event queues. Producers push during a tick; consumers read the
//! previous tick's events. No allocation in steady state once capacity has grown.

/// A queue of events of one type.
#[derive(Debug, Clone)]
pub struct EventQueue<T> {
    current: Vec<T>,
    previous: Vec<T>,
}

impl<T> Default for EventQueue<T> {
    fn default() -> Self {
        Self {
            current: Vec::new(),
            previous: Vec::new(),
        }
    }
}

impl<T> EventQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: T) {
        self.current.push(event);
    }

    /// Events pushed during the previous update.
    pub fn read(&self) -> &[T] {
        &self.previous
    }

    /// Events pushed so far during the current update.
    pub fn pending(&self) -> &[T] {
        &self.current
    }

    /// Swaps buffers: current events become readable, the old readable set is cleared.
    pub fn update(&mut self) {
        std::mem::swap(&mut self.current, &mut self.previous);
        self.current.clear();
    }

    /// Takes all pending events immediately (single-consumer use).
    pub fn drain(&mut self) -> std::vec::Drain<'_, T> {
        self.current.drain(..)
    }

    pub fn clear(&mut self) {
        self.current.clear();
        self.previous.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_buffering() {
        let mut q = EventQueue::new();
        q.push(1);
        q.push(2);
        assert!(q.read().is_empty());
        q.update();
        assert_eq!(q.read(), &[1, 2]);
        q.push(3);
        q.update();
        assert_eq!(q.read(), &[3]);
        q.update();
        assert!(q.read().is_empty());
    }

    #[test]
    fn drain_pending() {
        let mut q = EventQueue::new();
        q.push("a");
        let v: Vec<_> = q.drain().collect();
        assert_eq!(v, vec!["a"]);
        assert!(q.pending().is_empty());
    }
}
