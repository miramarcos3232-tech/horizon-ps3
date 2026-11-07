#![allow(unsafe_code)]

//! Optional `GlobalAlloc` for the PPU build (feature `global-alloc`).
//!
//! [`RegionAllocator`] is a lock-free bump allocator over one region that the
//! entrypoint obtains once (for example from `Psl1ghtApi::alloc_heap_region`)
//! and passes to [`RegionAllocator::init`]. It frees only the most recent
//! allocation; everything else is reclaimed by resetting the whole region, so
//! the real client should keep long-lived data in `BumpArena`/`FixedPool`
//! structures and use this only for the small amount of `alloc` the runtime
//! needs. This library does not install it: the final executable crate does,
//! with `#[global_allocator] static HEAP: RegionAllocator = RegionAllocator::new();`.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::bounded_alloc::AllocStats;

pub struct RegionAllocator {
    base: AtomicPtr<u8>,
    len: AtomicUsize,
    cursor: AtomicUsize,
    peak: AtomicUsize,
    failed: AtomicUsize,
}

impl RegionAllocator {
    pub const fn new() -> Self {
        Self {
            base: AtomicPtr::new(ptr::null_mut()),
            len: AtomicUsize::new(0),
            cursor: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            failed: AtomicUsize::new(0),
        }
    }

    /// Gives the allocator its memory.
    ///
    /// # Safety
    /// `base..base+len` must be valid, writable, not used by anything else for
    /// the rest of the program, and this must run before the first allocation.
    pub unsafe fn init(&self, base: *mut u8, len: usize) {
        self.len.store(len, Ordering::Relaxed);
        self.cursor.store(0, Ordering::Relaxed);
        self.base.store(base, Ordering::Release);
    }

    /// Statistics, saturated to `u32` (the PS3 address space is 32-bit-sized
    /// for budgeting purposes).
    pub fn stats(&self) -> AllocStats {
        let cap = |v: usize| u32::try_from(v).unwrap_or(u32::MAX);
        AllocStats {
            capacity: cap(self.len.load(Ordering::Relaxed)),
            used: cap(self.cursor.load(Ordering::Relaxed)),
            peak: cap(self.peak.load(Ordering::Relaxed)),
            failed: cap(self.failed.load(Ordering::Relaxed)),
        }
    }

    fn try_reserve(&self, base: *mut u8, layout: Layout) -> Option<usize> {
        let len = self.len.load(Ordering::Relaxed);
        let mut cur = self.cursor.load(Ordering::Relaxed);
        loop {
            let addr = (base as usize).checked_add(cur)?;
            let pad = addr.wrapping_neg() & (layout.align() - 1);
            let start = cur.checked_add(pad)?;
            let end = start.checked_add(layout.size())?;
            if end > len {
                return None;
            }
            match self
                .cursor
                .compare_exchange_weak(cur, end, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => {
                    self.peak.fetch_max(end, Ordering::Relaxed);
                    return Some(start);
                }
                Err(actual) => cur = actual,
            }
        }
    }
}

impl Default for RegionAllocator {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl GlobalAlloc for RegionAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = self.base.load(Ordering::Acquire);
        if base.is_null() {
            return ptr::null_mut();
        }
        match self.try_reserve(base, layout) {
            // SAFETY: `start + size <= len`, and `init` guarantees the region.
            Some(start) => unsafe { base.add(start) },
            None => {
                self.failed.fetch_add(1, Ordering::Relaxed);
                ptr::null_mut()
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let base = self.base.load(Ordering::Acquire);
        if base.is_null() {
            return;
        }
        let start = (ptr as usize).wrapping_sub(base as usize);
        // Only the newest allocation can be given back.
        let _ = self.cursor.compare_exchange(
            start + layout.size(),
            start,
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
    }
}
