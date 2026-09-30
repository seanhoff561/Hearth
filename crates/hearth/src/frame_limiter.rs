//! Frame-rate cap with a sleep-then-spin wait for accurate pacing.

use std::time::{Duration, Instant};

use crate::app::frame_budget;

/// Paces frames to a target rate. Sleeps for most of the remaining budget and spins for the
/// last fraction of a millisecond, because OS sleep granularity is too coarse on its own.
#[derive(Debug)]
pub struct FrameLimiter {
    next_frame: Instant,
}

impl Default for FrameLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameLimiter {
    const SPIN_MARGIN: Duration = Duration::from_micros(1500);

    pub fn new() -> Self {
        Self {
            next_frame: Instant::now(),
        }
    }

    /// Blocks until the next frame is due. `None` means uncapped.
    pub fn wait(&mut self, fps_cap: Option<u32>) {
        let Some(fps) = fps_cap else {
            self.next_frame = Instant::now();
            return;
        };
        let budget = frame_budget(fps);
        let now = Instant::now();
        if self.next_frame > now {
            let remaining = self.next_frame - now;
            if remaining > Self::SPIN_MARGIN {
                std::thread::sleep(remaining - Self::SPIN_MARGIN);
            }
            while Instant::now() < self.next_frame {
                std::hint::spin_loop();
            }
            self.next_frame += budget;
        } else {
            // Running behind: don't try to catch up with a burst of frames.
            self.next_frame = now + budget;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_frame_rate() {
        let mut l = FrameLimiter::new();
        let start = Instant::now();
        for _ in 0..10 {
            l.wait(Some(200));
        }
        // 10 frames at 200 FPS ≈ 45–50 ms (the first frame is immediate).
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(40), "{elapsed:?}");
    }

    #[test]
    fn uncapped_does_not_wait() {
        let mut l = FrameLimiter::new();
        let start = Instant::now();
        for _ in 0..1000 {
            l.wait(None);
        }
        assert!(start.elapsed() < Duration::from_millis(50));
    }
}
