// xpute-test-mem/lib.rs

//! A test process's guest memory, as a native guest's is the one region it
//! reserves (xpute-link `mem::set`): the process's whole heap, a global
//! allocator over one region set as the guest's memory at the first
//! allocation, so that every allocation, the guest's and the host's alike,
//! has an offset. It never frees, a test being short; its region is the most
//! a u32 offset reaches, reserved zeroed, which a system maps only as it is
//! touched. A test binary takes it by
//! `#[global_allocator] static HEAP: Heap = Heap;`, as cpp/dev/test/mem is
//! the C++ tests'.

#![cfg(not(target_arch = "wasm32"))]

use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Once;

use xpute_link::mem::{self, Region};

pub struct Heap;

const SZ: usize = u32::MAX as usize & !15;

static END: AtomicUsize = AtomicUsize::new(0);

// SAFETY: each block lies in the region, apart from every other, and none is
// handed out twice.
unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        static SET: Once = Once::new();
        SET.call_once(|| {
            // At the system's own alignment, the system's zeroed memory is
            // mapped as it is touched; past it, it is written. A region the
            // system refuses stops the process, as nothing inside the
            // allocator may unwind.
            // SAFETY: a layout of nonzero size.
            let base = unsafe { System.alloc_zeroed(Layout::from_size_align(SZ, 16).unwrap()) };
            if base.is_null() {
                std::process::abort();
            }
            mem::set(Region { base, len: SZ as u32 });
        });
        let base = mem::region().base.expose_provenance();
        let mut end = END.load(Relaxed);
        loop {
            let at = (base + end).next_multiple_of(l.align()) - base;
            if at + l.size() > SZ {
                return core::ptr::null_mut();
            }
            match END.compare_exchange_weak(end, at + l.size(), Relaxed, Relaxed) {
                Ok(_) => return core::ptr::with_exposed_provenance_mut(base + at),
                Err(now) => end = now,
            }
        }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

/// Takes `len` zeroed bytes of the guest's memory, given to no one else, and
/// answers where they lie.
pub fn alloc(len: u32) -> u32 {
    mem::off(Vec::leak(vec![0u64; len.div_ceil(8).max(1) as usize]).as_ptr().cast())
}
