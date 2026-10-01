// pxl-kernel/allocations.test.rs
//
// Counts the heap allocations a thread makes, for the laws that say a path
// allocates nothing once it has held the most it needs: the test binary's
// allocator is the system's, counted per thread so tests running beside one
// another do not count each other.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

// SAFETY: the system's allocator, each call passed through unchanged; the
// count is a thread-local cell with a const initializer and no destructor,
// so touching it allocates nothing and is never torn down under a call.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        COUNT.with(|c| c.set(c.get() + 1));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The allocations this thread makes while `f` runs, and what it returns.
pub fn during<T>(f: impl FnOnce() -> T) -> (u64, T) {
    let before = COUNT.with(Cell::get);
    let out = f();
    (COUNT.with(Cell::get) - before, out)
}
