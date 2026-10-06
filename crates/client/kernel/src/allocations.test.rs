// pxl-kernel/allocations.test.rs
//
// Counts heap allocations per thread, so tests running side by side do not
// count each other's.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

// SAFETY: the system allocator, each call passed through unchanged; the
// count is a const-initialized thread-local with no destructor, so touching
// it never allocates or is torn down under a call.
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

pub fn during<T>(f: impl FnOnce() -> T) -> (u64, T) {
    let before = COUNT.with(Cell::get);
    let out = f();
    (COUNT.with(Cell::get) - before, out)
}
