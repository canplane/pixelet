//! The module's heap: a fixed range of the core memory, handed out in
//! power-of-two size classes.
//!
//! The memory is imported and never grows — it is the client's fixed
//! reservation, cut into sections — so the default allocator, which asks
//! for pages when it runs out, would fail at its first request past the
//! initial size. This one owns `[__heap_base, MODULE_BYTES)`: what is left of
//! the module's section after the linker placed the stack and the data
//! before it (`--stack-first`, the default; 1 MB of stack). The section's
//! size is the one number this crate and core_pool.ts agree on.
//!
//! Size classes rather than a bump pointer, because what lives here is the
//! contexts' scratch — vectors that grow to a chunk's size, several of them
//! interleaved as scans and bakes alternate — and a bump pointer leaks every
//! block a vector leaves behind on its way up unless it happens to be the
//! last. A freed block goes on its class's list and the next request of
//! that class takes it, so the heap's use is bounded by the high-water mark
//! of each vector, at most doubled by the rounding. Exhausting the arena is
//! a bug in the sizing, not a runtime condition: it traps, and the client
//! falls back to the TypeScript path.

use core::alloc::{GlobalAlloc, Layout};

/// The section at the front of the core memory that is the module's own.
/// Equal to core_pool.ts's MODULE_BYTES.
pub const MODULE_BYTES: usize = 16 << 20;

/// The smallest class, and the alignment every block has.
const MIN_SHIFT: usize = 4;
const CLASSES: usize = 32 - MIN_SHIFT;

extern "C" {
    static __heap_base: u8;
}

pub struct Arena;

/// Where untouched space starts, and each class's list of freed blocks
/// (each block's first word is the next). Reset whenever the module is
/// instantiated: with the memory imported the linker cannot assume it
/// zeroed, so every static, this one included, is written by the data
/// segments at instantiation (verified — a second instance over a memory
/// the first left mid-bake starts clean).
struct Heap {
    top: usize,
    free: [usize; CLASSES],
}

static mut HEAP: Heap = Heap { top: 0, free: [0; CLASSES] };

fn heap_base() -> usize {
    core::ptr::addr_of!(__heap_base) as usize
}

fn class_of(layout: Layout) -> usize {
    let bytes = layout.size().max(layout.align()).max(1 << MIN_SHIFT);
    (usize::BITS - (bytes - 1).leading_zeros()) as usize - MIN_SHIFT
}

unsafe impl GlobalAlloc for Arena {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let h = &mut *core::ptr::addr_of_mut!(HEAP);
        if h.top == 0 {
            h.top = heap_base();
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
        if end > MODULE_BYTES {
            return core::ptr::null_mut();
        }
        h.top = end;
        start as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let h = &mut *core::ptr::addr_of_mut!(HEAP);
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

/// How much of the arena has been cut, for the ledger.
pub fn used() -> usize {
    // SAFETY: single-threaded module; see lib.rs.
    let h = unsafe { &*core::ptr::addr_of!(HEAP) };
    if h.top == 0 {
        0
    } else {
        h.top - heap_base()
    }
}
