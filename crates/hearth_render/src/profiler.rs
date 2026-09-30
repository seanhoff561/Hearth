//! GPU pass timing: timestamps written between passes (where the adapter can write them inside
//! a command encoder) and a few GPU statistics, read back asynchronously a few frames later so
//! the frame path never waits for the GPU. Used by the benchmark; the game runs without it.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::gpu::GpuContext;

/// Timestamps per frame at most.
const MAX_MARKS: u32 = 32;
/// Frames that may be waiting for their readback.
const RING: usize = 4;
/// Statistics (u32) copied with each frame's timestamps.
pub const STATS: usize = 16;

const IDLE: u8 = 0;
const RECORDED: u8 = 1;
const MAPPING: u8 = 2;
const READY: u8 = 3;
const FAILED: u8 = 4;

struct Slot {
    buffer: wgpu::Buffer,
    labels: Vec<&'static str>,
    has_stats: bool,
    seq: u64,
    state: Arc<AtomicU8>,
}

/// GPU times of one frame.
#[derive(Debug, Clone, Default)]
pub struct FrameGpu {
    /// Frame number (in `begin_frame` order).
    pub seq: u64,
    /// Each pass (named by the mark that ends it) and its duration in milliseconds.
    pub passes: Vec<(&'static str, f64)>,
    /// First to last mark (ms).
    pub total_ms: f64,
    /// Statistics copied at the end of the frame (terrain culling counters), if any.
    pub stats: Option<[u32; STATS]>,
}

/// Times the passes of frames on the GPU.
pub struct GpuTimer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    slots: Vec<Slot>,
    labels: Vec<&'static str>,
    current: Option<usize>,
    seq: u64,
    period_ns: f64,
    /// Frames not timed because every readback buffer was busy.
    pub skipped: u64,
}

impl GpuTimer {
    /// A timer, or `None` when the adapter cannot write timestamps between passes.
    pub fn new(ctx: &GpuContext) -> Option<Self> {
        if !ctx.caps.timestamps_inside_encoders {
            return None;
        }
        let set = ctx.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("pass timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: MAX_MARKS,
        });
        let resolve = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamp resolve"),
            size: MAX_MARKS as u64 * 8,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let slots = (0..RING)
            .map(|_| Slot {
                buffer: ctx.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("timestamp readback"),
                    size: MAX_MARKS as u64 * 8 + STATS as u64 * 4,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                labels: Vec::with_capacity(MAX_MARKS as usize),
                has_stats: false,
                seq: 0,
                state: Arc::new(AtomicU8::new(IDLE)),
            })
            .collect();
        Some(Self {
            set,
            resolve,
            slots,
            labels: Vec::with_capacity(MAX_MARKS as usize),
            current: None,
            seq: 0,
            period_ns: ctx.queue.get_timestamp_period() as f64,
            skipped: 0,
        })
    }

    /// Starts a frame: picks a free readback buffer (or skips timing this frame).
    pub fn begin_frame(&mut self) {
        self.labels.clear();
        self.current = self
            .slots
            .iter()
            .position(|s| s.state.load(Ordering::Acquire) == IDLE);
        if self.current.is_none() {
            self.skipped += 1;
        }
        self.seq += 1;
    }

    /// Writes a timestamp: the GPU work recorded since the previous mark is the pass `label`.
    pub fn mark(&mut self, enc: &mut wgpu::CommandEncoder, label: &'static str) {
        if self.current.is_none() || self.labels.len() as u32 >= MAX_MARKS {
            return;
        }
        enc.write_timestamp(&self.set, self.labels.len() as u32);
        self.labels.push(label);
    }

    /// Resolves the frame's timestamps and copies them, and `stats` (at least `STATS` u32s), to
    /// the frame's readback buffer.
    pub fn end_frame(&mut self, enc: &mut wgpu::CommandEncoder, stats: Option<&wgpu::Buffer>) {
        let Some(i) = self.current else {
            return;
        };
        let n = self.labels.len() as u32;
        let slot = &mut self.slots[i];
        if n > 0 {
            enc.resolve_query_set(&self.set, 0..n, &self.resolve, 0);
            enc.copy_buffer_to_buffer(&self.resolve, 0, &slot.buffer, 0, n as u64 * 8);
        }
        if let Some(s) = stats {
            enc.copy_buffer_to_buffer(s, 0, &slot.buffer, MAX_MARKS as u64 * 8, STATS as u64 * 4);
        }
        slot.labels.clear();
        slot.labels.extend_from_slice(&self.labels);
        slot.has_stats = stats.is_some();
        slot.seq = self.seq;
        slot.state.store(RECORDED, Ordering::Release);
    }

    /// Call after the frame's commands were submitted: starts the asynchronous readback.
    pub fn submitted(&mut self) {
        let Some(i) = self.current.take() else {
            return;
        };
        let slot = &self.slots[i];
        if slot.state.load(Ordering::Acquire) != RECORDED {
            return;
        }
        slot.state.store(MAPPING, Ordering::Release);
        let state = slot.state.clone();
        slot.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                state.store(if r.is_ok() { READY } else { FAILED }, Ordering::Release);
            });
    }

    /// Frames whose readback has arrived (without waiting), oldest first.
    pub fn collect(&mut self, ctx: &GpuContext) -> Vec<FrameGpu> {
        let _ = ctx.device.poll(wgpu::PollType::Poll);
        let mut out = Vec::new();
        for slot in &mut self.slots {
            match slot.state.load(Ordering::Acquire) {
                READY => {}
                FAILED => {
                    slot.state.store(IDLE, Ordering::Release);
                    continue;
                }
                _ => continue,
            }
            if let Ok(data) = slot.buffer.slice(..).get_mapped_range() {
                let ticks: &[u64] = bytemuck::cast_slice(&data[..MAX_MARKS as usize * 8]);
                let n = slot.labels.len();
                let ms = |a: u64, b: u64| b.saturating_sub(a) as f64 * self.period_ns * 1e-6;
                let passes = (1..n)
                    .map(|k| (slot.labels[k], ms(ticks[k - 1], ticks[k])))
                    .collect();
                let total_ms = if n > 1 {
                    ms(ticks[0], ticks[n - 1])
                } else {
                    0.0
                };
                let stats = slot.has_stats.then(|| {
                    let s: &[u32] = bytemuck::cast_slice(&data[MAX_MARKS as usize * 8..]);
                    let mut a = [0u32; STATS];
                    a.copy_from_slice(&s[..STATS]);
                    a
                });
                out.push(FrameGpu {
                    seq: slot.seq,
                    passes,
                    total_ms,
                    stats,
                });
            }
            slot.buffer.unmap();
            slot.state.store(IDLE, Ordering::Release);
        }
        out.sort_by_key(|f| f.seq);
        out
    }

    /// Waits until every frame in flight has been read back (end of a benchmark).
    pub fn drain(&mut self, ctx: &GpuContext) -> Vec<FrameGpu> {
        let _ = ctx.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        self.collect(ctx)
    }
}
