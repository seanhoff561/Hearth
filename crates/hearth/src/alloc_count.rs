//! Heap allocations counted per thread, so the benchmark can check that the frame path
//! allocates nothing in steady state. The count is one thread-local increment per allocation.
//! An allocation the system refuses is written to the crash log before the process aborts
//! (`crash::allocation_refused`).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

/// The system allocator, counting allocations.
pub struct CountingAllocator;

/// The pointer the system gave; a refusal (null) written to the crash log first.
#[inline]
fn refused(p: *mut u8, size: usize) -> *mut u8 {
    if p.is_null() {
        crate::crash::allocation_refused(size);
    }
    p
}

#[inline]
fn count() {
    // `try_with`: allocations while the thread is being torn down are not counted.
    let _ = ALLOCATIONS.try_with(|c| c.set(c.get() + 1));
}

// SAFETY: every call forwards to the system allocator with the same arguments.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: forwarded unchanged.
        refused(unsafe { System.alloc(layout) }, layout.size())
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: forwarded unchanged.
        refused(unsafe { System.alloc_zeroed(layout) }, layout.size())
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        // SAFETY: forwarded unchanged.
        refused(unsafe { System.realloc(ptr, layout, new_size) }, new_size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged.
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Heap allocations made by the calling thread so far.
pub fn thread_allocations() -> u64 {
    ALLOCATIONS.try_with(Cell::get).unwrap_or(0)
}
