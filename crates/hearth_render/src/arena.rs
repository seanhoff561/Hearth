//! Sub-allocated GPU storage: a first-fit allocator of element ranges and the growing storage
//! buffer it manages (the terrain's and the distant terrain's meshes live in these).

use crate::gpu::GpuContext;

/// First-fit free-list allocator over `[0, capacity)`.
#[derive(Debug, Clone)]
pub struct RangeAllocator {
    capacity: u32,
    /// Free ranges sorted by offset: (offset, len).
    free: Vec<(u32, u32)>,
    used: u32,
}

impl RangeAllocator {
    pub fn new(capacity: u32) -> Self {
        Self {
            capacity,
            free: vec![(0, capacity)],
            used: 0,
        }
    }

    pub fn alloc(&mut self, len: u32) -> Option<u32> {
        if len == 0 {
            return Some(0);
        }
        let i = self.free.iter().position(|(_, l)| *l >= len)?;
        let (off, l) = self.free[i];
        if l == len {
            self.free.remove(i);
        } else {
            self.free[i] = (off + len, l - len);
        }
        self.used += len;
        Some(off)
    }

    pub fn free(&mut self, off: u32, len: u32) {
        if len == 0 {
            return;
        }
        self.used -= len;
        let i = self.free.partition_point(|(o, _)| *o < off);
        self.free.insert(i, (off, len));
        // Coalesce with neighbours.
        if i + 1 < self.free.len() && self.free[i].0 + self.free[i].1 == self.free[i + 1].0 {
            self.free[i].1 += self.free[i + 1].1;
            self.free.remove(i + 1);
        }
        if i > 0 && self.free[i - 1].0 + self.free[i - 1].1 == self.free[i].0 {
            self.free[i - 1].1 += self.free[i].1;
            self.free.remove(i);
        }
    }

    /// Extends the capacity (after the backing buffer grew).
    pub fn grow(&mut self, new_capacity: u32) {
        let extra = new_capacity - self.capacity;
        let old = self.capacity;
        self.capacity = new_capacity;
        self.used += extra;
        self.free(old, extra);
    }

    pub fn used(&self) -> u32 {
        self.used
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }
}

/// A storage buffer of fixed-size elements with a range allocator; grows by doubling.
pub(crate) struct Arena {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) alloc: RangeAllocator,
    stride: u64,
    label: &'static str,
    /// Uses beyond storage (an index buffer's).
    extra: wgpu::BufferUsages,
}

impl Arena {
    /// Video memory the arena's buffer takes (bytes).
    pub(crate) fn bytes(&self) -> u64 {
        self.stride * self.alloc.capacity() as u64
    }

    pub(crate) fn new(
        device: &wgpu::Device,
        label: &'static str,
        stride: u64,
        capacity: u32,
    ) -> Self {
        Self::with_usage(device, label, stride, capacity, wgpu::BufferUsages::empty())
    }

    pub(crate) fn with_usage(
        device: &wgpu::Device,
        label: &'static str,
        stride: u64,
        capacity: u32,
        extra: wgpu::BufferUsages,
    ) -> Self {
        Self {
            buffer: Self::make(device, label, stride, capacity, extra),
            alloc: RangeAllocator::new(capacity),
            stride,
            label,
            extra,
        }
    }

    fn make(
        device: &wgpu::Device,
        label: &str,
        stride: u64,
        capacity: u32,
        extra: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: stride * capacity as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
                | extra,
            mapped_at_creation: false,
        })
    }

    /// Allocates `len` elements, growing the buffer if needed. Returns (offset, grew).
    pub(crate) fn alloc(&mut self, ctx: &GpuContext, len: u32) -> (u32, bool) {
        if let Some(off) = self.alloc.alloc(len) {
            return (off, false);
        }
        let max_elems = (ctx.device.limits().max_storage_buffer_binding_size / self.stride) as u32;
        let mut cap = self.alloc.capacity();
        while cap < max_elems
            && self.alloc.capacity() - self.alloc.used() + (cap - self.alloc.capacity()) < len
        {
            cap = (cap * 2).min(max_elems);
        }
        cap = (cap.max(self.alloc.capacity() * 2)).min(max_elems);
        if cap <= self.alloc.capacity() {
            log::error!("{} arena is full ({} elements)", self.label, cap);
            return (u32::MAX, false);
        }
        let new_buf = Self::make(&ctx.device, self.label, self.stride, cap, self.extra);
        let mut enc = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("arena grow"),
            });
        enc.copy_buffer_to_buffer(
            &self.buffer,
            0,
            &new_buf,
            0,
            self.stride * self.alloc.capacity() as u64,
        );
        ctx.queue.submit(Some(enc.finish()));
        self.buffer = new_buf;
        self.alloc.grow(cap);
        log::info!(
            "{} arena grew to {} MiB",
            self.label,
            (self.stride * cap as u64) >> 20
        );
        let off = self.alloc.alloc(len).unwrap_or(u32::MAX);
        (off, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_reuses_and_coalesces() {
        let mut a = RangeAllocator::new(100);
        let x = a.alloc(30).unwrap();
        let y = a.alloc(30).unwrap();
        let z = a.alloc(30).unwrap();
        assert_eq!((x, y, z), (0, 30, 60));
        assert!(a.alloc(20).is_none());
        a.free(y, 30);
        assert_eq!(a.alloc(20), Some(30));
        a.free(30, 20);
        a.free(x, 30);
        a.free(z, 30);
        assert_eq!(a.used(), 0);
        assert_eq!(a.alloc(100), Some(0), "fully coalesced");
        a.free(0, 100);
        a.grow(200);
        assert_eq!(a.alloc(200), Some(0));
    }
}
