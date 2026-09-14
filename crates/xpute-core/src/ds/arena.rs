//! A heap over a fixed range of a linear memory, handed out in power-of-two
//! size classes: the allocator for a module whose memory is imported and
//! never grows. The default allocator asks for pages when it runs out and
//! would fail at its first request past the initial size; this one owns
//! the range its `Range` names and nothing else.
//!
//! Size classes rather than a bump pointer, because what lives in such a
//! heap is scratch — vectors that grow to a job's size, several of them
//! interleaved — and a bump pointer leaks every block a vector leaves
//! behind on its way up unless it happens to be the last. A freed block
//! goes on its class's list and the next request of that class takes it,
//! so the heap's use is bounded by the high-water mark of each vector, at
//! most doubled by the rounding. Exhausting the arena is a bug in the
//! sizing, not a runtime condition: the request returns null and the
//! allocation traps.
//!
//! The state is reset whenever the module is instantiated: with the memory
//! imported the linker cannot assume it zeroed, so every static, the
//! arena's included, is written by the data segments at instantiation
//! (verified — a second instance over a memory the first left mid-job
//! starts clean).

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::marker::PhantomData;

/// The smallest class, and the alignment every block has.
const MIN_SHIFT: usize = 4;
const CLASSES: usize = 32 - MIN_SHIFT;

/// Where an arena's range lies: from `base()` (read once, at the first
/// request — a linker symbol's address, or a buffer's) up to `END`.
pub trait Range {
    fn base() -> usize;
    const END: usize;
}

/// Where untouched space starts, and each class's list of freed blocks
/// (each block's first word is the next).
struct Heap {
    top: usize,
    free: [usize; CLASSES],
}

pub struct Arena<R: Range> {
    heap: UnsafeCell<Heap>,
    range: PhantomData<R>,
}

// SAFETY: the arena serves one thread — a module instantiated once per
// memory, which is what the imported-memory design guarantees.
unsafe impl<R: Range> Sync for Arena<R> {}

impl<R: Range> Arena<R> {
    pub const fn new() -> Self {
        Arena {
            heap: UnsafeCell::new(Heap { top: 0, free: [0; CLASSES] }),
            range: PhantomData,
        }
    }

    /// How much of the range has been cut, for a ledger.
    pub fn used(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        let h = unsafe { &*self.heap.get() };
        if h.top == 0 {
            0
        } else {
            h.top - R::base()
        }
    }
}

impl<R: Range> Default for Arena<R> {
    fn default() -> Self {
        Self::new()
    }
}

fn class_of(layout: Layout) -> usize {
    let bytes = layout.size().max(layout.align()).max(1 << MIN_SHIFT);
    (usize::BITS - (bytes - 1).leading_zeros()) as usize - MIN_SHIFT
}

unsafe impl<R: Range> GlobalAlloc for Arena<R> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let h = &mut *self.heap.get();
        if h.top == 0 {
            h.top = R::base();
        }
        let class = class_of(layout);
        let head = h.free[class];
        if head != 0 {
            h.free[class] = *(head as *const usize);
            return head as *mut u8;
        }
        let bytes = 1 << (class + MIN_SHIFT);
        let start = (h.top + layout.align() - 1) & !(layout.align() - 1);
        let end = start + bytes;
        if end > R::END {
            return core::ptr::null_mut();
        }
        h.top = end;
        start as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let h = &mut *self.heap.get();
        let class = class_of(layout);
        *(ptr as *mut usize) = h.free[class];
        h.free[class] = ptr as usize;
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        if class_of(new_layout) == class_of(layout) {
            return ptr;
        }
        let new = self.alloc(new_layout);
        if !new.is_null() {
            core::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
            self.dealloc(ptr, layout);
        }
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(align(4096))]
    #[allow(dead_code)]
    struct Buffer([u8; 1 << 16]);
    // Writable, as a module's memory is; an immutable static would sit in
    // read-only pages.
    static mut BUFFER: Buffer = Buffer([0; 1 << 16]);
    struct Whole;
    impl Range for Whole {
        fn base() -> usize {
            core::ptr::addr_of_mut!(BUFFER) as usize
        }
        const END: usize = usize::MAX;
    }

    #[test]
    fn a_freed_block_serves_the_next_request_of_its_class() {
        let arena: Arena<Whole> = Arena::new();
        let layout = Layout::from_size_align(100, 8).unwrap();
        unsafe {
            let a = arena.alloc(layout);
            assert_eq!(a as usize, Whole::base());
            let b = arena.alloc(layout);
            assert_eq!(b as usize, a as usize + 128, "a class rounds up to its power of two");
            arena.dealloc(a, layout);
            assert_eq!(arena.alloc(layout), a);
            // A realloc within the class keeps the block; past it, moves.
            assert_eq!(arena.realloc(b, layout, 120), b);
            let c = arena.realloc(b, layout, 200);
            assert_ne!(c, b);
        }
        assert_eq!(arena.used(), 128 * 2 + 256);
    }
}
