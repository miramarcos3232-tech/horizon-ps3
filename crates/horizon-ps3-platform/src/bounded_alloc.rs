//! Bounded, allocation-free memory helpers for the PS3 port.
//!
//! Nothing here owns memory: [`BumpArena`] hands out aligned *offsets* inside a
//! region the caller owns (an XDR block, an RSX mapping, a static array), and
//! [`FixedPool`] hands out slot indices. Both track usage, peak and failures so
//! the platform layer can report them as telemetry. They are plain safe Rust and
//! are tested on the host.

/// Usage counters shared by every allocator in this module.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AllocStats {
    /// Total bytes (or slots) the allocator may hand out.
    pub capacity: u32,
    /// Bytes (or slots) currently in use.
    pub used: u32,
    /// Highest `used` value ever observed.
    pub peak: u32,
    /// Number of requests that were refused.
    pub failed: u32,
}

/// Why an allocation request was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocError {
    OutOfMemory,
    InvalidAlignment,
    ZeroSize,
}

/// A bump allocator over `capacity` bytes, with marks for scoped release.
#[derive(Clone, Copy, Debug)]
pub struct BumpArena {
    capacity: u32,
    cursor: u32,
    peak: u32,
    failed: u32,
}

impl BumpArena {
    pub const fn new(capacity: u32) -> Self {
        Self {
            capacity,
            cursor: 0,
            peak: 0,
            failed: 0,
        }
    }

    /// Reserves `size` bytes aligned to `align` (a power of two) and returns the
    /// byte offset from the start of the region.
    pub fn alloc(&mut self, size: u32, align: u32) -> Result<u32, AllocError> {
        if size == 0 {
            return Err(AllocError::ZeroSize);
        }
        if align == 0 || !align.is_power_of_two() {
            return Err(AllocError::InvalidAlignment);
        }
        let start = match self.cursor.checked_add(align - 1) {
            Some(v) => v & !(align - 1),
            None => return self.refuse(),
        };
        let end = match start.checked_add(size) {
            Some(v) if v <= self.capacity => v,
            _ => return self.refuse(),
        };
        self.cursor = end;
        if end > self.peak {
            self.peak = end;
        }
        Ok(start)
    }

    fn refuse(&mut self) -> Result<u32, AllocError> {
        self.failed = self.failed.saturating_add(1);
        Err(AllocError::OutOfMemory)
    }

    /// Current position, to be passed to [`release_to`](Self::release_to).
    pub const fn mark(&self) -> u32 {
        self.cursor
    }

    /// Frees everything allocated after `mark`. Returns `false` (and changes
    /// nothing) if `mark` is ahead of the cursor.
    pub fn release_to(&mut self, mark: u32) -> bool {
        if mark > self.cursor {
            return false;
        }
        self.cursor = mark;
        true
    }

    /// Frees everything; the peak and failure counters are kept.
    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    pub const fn stats(&self) -> AllocStats {
        AllocStats {
            capacity: self.capacity,
            used: self.cursor,
            peak: self.peak,
            failed: self.failed,
        }
    }
}

const NONE: u16 = u16::MAX;
const TAKEN: u16 = u16::MAX - 1;

/// A pool of `N` equally sized slots addressed by index, with a free list and
/// double-free detection. `N` must be in `1..=65533`.
#[derive(Clone, Debug)]
pub struct FixedPool<const N: usize> {
    next: [u16; N],
    head: u16,
    in_use: u32,
    peak: u32,
    failed: u32,
}

impl<const N: usize> FixedPool<N> {
    pub const fn new() -> Self {
        assert!(N >= 1 && N <= (u16::MAX - 2) as usize);
        let mut next = [NONE; N];
        let mut i = 0;
        while i + 1 < N {
            next[i] = (i + 1) as u16;
            i += 1;
        }
        Self {
            next,
            head: 0,
            in_use: 0,
            peak: 0,
            failed: 0,
        }
    }

    /// Takes a free slot, or `None` when the pool is exhausted.
    pub fn alloc(&mut self) -> Option<u16> {
        if self.head == NONE {
            self.failed = self.failed.saturating_add(1);
            return None;
        }
        let slot = self.head;
        self.head = self.next[slot as usize];
        self.next[slot as usize] = TAKEN;
        self.in_use += 1;
        if self.in_use > self.peak {
            self.peak = self.in_use;
        }
        Some(slot)
    }

    /// Returns a slot to the pool. `false` means the slot was out of range or
    /// not currently allocated (a double free), and nothing changed.
    pub fn free(&mut self, slot: u16) -> bool {
        let index = slot as usize;
        if index >= N || self.next[index] != TAKEN {
            return false;
        }
        self.next[index] = self.head;
        self.head = slot;
        self.in_use -= 1;
        true
    }

    pub const fn stats(&self) -> AllocStats {
        AllocStats {
            capacity: N as u32,
            used: self.in_use,
            peak: self.peak,
            failed: self.failed,
        }
    }
}

impl<const N: usize> Default for FixedPool<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_aligns_and_tracks_peak() {
        let mut a = BumpArena::new(100);
        assert_eq!(a.alloc(10, 1), Ok(0));
        assert_eq!(a.alloc(8, 16), Ok(16));
        assert_eq!(a.stats().used, 24);
        let mark = a.mark();
        assert_eq!(a.alloc(50, 1), Ok(24));
        assert_eq!(a.stats().peak, 74);
        assert!(a.release_to(mark));
        assert_eq!(a.stats().used, 24);
        assert_eq!(a.stats().peak, 74);
        assert!(!a.release_to(99));
    }

    #[test]
    fn arena_refuses_bad_requests_and_counts_failures() {
        let mut a = BumpArena::new(32);
        assert_eq!(a.alloc(0, 4), Err(AllocError::ZeroSize));
        assert_eq!(a.alloc(4, 3), Err(AllocError::InvalidAlignment));
        assert_eq!(a.alloc(4, 0), Err(AllocError::InvalidAlignment));
        assert_eq!(a.alloc(33, 1), Err(AllocError::OutOfMemory));
        assert_eq!(a.alloc(u32::MAX, 1), Err(AllocError::OutOfMemory));
        assert_eq!(a.stats().failed, 2);
        assert_eq!(a.stats().used, 0);
        a.alloc(32, 1).unwrap();
        a.reset();
        assert_eq!(a.stats().used, 0);
        assert_eq!(a.stats().peak, 32);
    }

    #[test]
    fn pool_reuses_slots_and_detects_double_free() {
        let mut p: FixedPool<3> = FixedPool::new();
        let a = p.alloc().unwrap();
        let b = p.alloc().unwrap();
        let c = p.alloc().unwrap();
        assert_eq!((a, b, c), (0, 1, 2));
        assert_eq!(p.alloc(), None);
        assert_eq!(p.stats().failed, 1);
        assert!(p.free(b));
        assert!(!p.free(b));
        assert!(!p.free(7));
        assert_eq!(p.alloc(), Some(b));
        assert_eq!(p.stats().peak, 3);
        assert!(p.free(a) && p.free(b) && p.free(c));
        assert_eq!(p.stats().used, 0);
    }

    #[test]
    fn pool_of_one() {
        let mut p: FixedPool<1> = FixedPool::new();
        assert_eq!(p.alloc(), Some(0));
        assert_eq!(p.alloc(), None);
        assert!(p.free(0));
        assert_eq!(p.alloc(), Some(0));
    }
}
