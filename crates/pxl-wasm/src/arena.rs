//! The module's heap: a fixed range of the core memory, handed out by a
//! bump pointer.
//!
//! The memory is imported and never grows — it is the client's fixed
//! reservation, cut into sections — so the default allocator, which asks
//! for pages when it runs out, would fail at its first request past the
//! initial size. This one owns `[__heap_base, MODULE_BYTES)`: what is left of
//! the module's section after the linker placed the stack and the data
//! before it (`--stack-first`, the default; 1 MB of stack). The section's
//! size is the one number this crate and core_pool.ts agree on.
//!
//! A bump allocator, because what lives on this heap is the bake context's
//! scratch — vectors that grow to a chunk's size and are then reused — and
//! not a churn of small objects. Freeing the topmost block gives its bytes
//! back and growing the topmost block extends it in place, which is a
//! vector's usual case; any other free is leaked, bounded by the sum of the
//! capacities a vector passed through on its way up, about twice what it
//! ends at. Exhausting the arena is a bug in the sizing, not a runtime
//! condition: it traps, and the client falls back to the TypeScript mesher.

use core::alloc::{GlobalAlloc, Layout};

/// The section at the front of the core memory that is the module's own.
/// Equal to core_pool.ts's MODULE_BYTES.
pub const MODULE_BYTES: usize = 16 << 20;

extern "C" {
    static __heap_base: u8;
}

pub struct Arena;

/// Where the next allocation starts; 0 until the first one, which begins at
/// the heap base. Reset whenever the module is instantiated: with the memory
/// imported the linker cannot assume it zeroed, so every static, this one
/// included, is written by the data segments at instantiation (verified —
/// a second instance over a memory the first left mid-bake starts clean).
struct Cursor {
    top: usize,
    last: usize,
}

static mut CURSOR: Cursor = Cursor { top: 0, last: 0 };

fn heap_base() -> usize {
    core::ptr::addr_of!(__heap_base) as usize
}

unsafe impl GlobalAlloc for Arena {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let c = &mut *core::ptr::addr_of_mut!(CURSOR);
        if c.top == 0 {
            c.top = heap_base();
        }
        let start = (c.top + layout.align() - 1) & !(layout.align() - 1);
        let end = start + layout.size();
        if end > MODULE_BYTES {
            return core::ptr::null_mut();
        }
        c.last = start;
        c.top = end;
        start as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        let c = &mut *core::ptr::addr_of_mut!(CURSOR);
        if ptr as usize == c.last {
            c.top = c.last;
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let c = &mut *core::ptr::addr_of_mut!(CURSOR);
        if ptr as usize == c.last {
            let end = c.last + new_size;
            if end > MODULE_BYTES {
                return core::ptr::null_mut();
            }
            c.top = end;
            return ptr;
        }
        let new = self.alloc(Layout::from_size_align_unchecked(new_size, layout.align()));
        if !new.is_null() {
            core::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
        }
        new
    }
}

/// How much of the arena is in use, for the ledger.
pub fn used() -> usize {
    // SAFETY: single-threaded module; see lib.rs.
    let c = unsafe { &*core::ptr::addr_of!(CURSOR) };
    if c.top == 0 {
        0
    } else {
        c.top - heap_base()
    }
}
