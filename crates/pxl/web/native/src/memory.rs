// pxl-native/memory.rs

//! The kernel's memory on a device: one region taken from the system at the
//! first request and never given back, the base every address the kernel
//! names is an offset from (xpute_runtime::mem::base), and the allocator over
//! it — the slab over a buddy the module has (pxl-wasm), so a `Vec` the kernel
//! hands the shell lies in the memory the shell reads.
//!
//! Unlike the module's, this allocator is the whole library's, and a device's
//! GPU driver calls back on threads of its own; a lock makes the slab — built
//! for one thread — sound for them. The kernel's thread takes it uncontended.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::alloc::{GlobalAlloc, Layout, System};

use pxl_kernel::host::HeapStat;
use pxl_kernel::mem::section;
use pxl_kernel::mem::spec::{GLOBAL_BASE, PHONE_BYTES};
use xpute_core::alloc::slab::SlabMalloc;

/// The memory the kernel is given: a phone's reservation, mem.json's, as a
/// page on a phone reserves.
pub const GIVEN: u32 = PHONE_BYTES;

/// Where the bracket's own data ends: at mem.json's GLOBAL_BASE, past the
/// rings' front (pxl-kernel door/layout.rs). A device's library keeps its
/// statics where the linker put them, so nothing of its own lies in the
/// memory above the front.
pub const DATA_END: u32 = GLOBAL_BASE;

/// The region's alignment: a buddy block lies at an offset from the base that
/// is a multiple of its size, so a base on a large boundary keeps every block
/// aligned as far as anything asks.
const REGION_ALIGN: usize = 1 << 16;

/// The region's first byte, 0 before the first request.
static REGION: AtomicUsize = AtomicUsize::new(0);

/// The region, taken on the first call — zeroed, as a WebAssembly memory is,
/// and committed by the system page by page as it is touched rather than at
/// once — and made the memory's base.
pub fn region() -> usize {
    let at = REGION.load(Ordering::Acquire);
    if at != 0 {
        return at;
    }
    let layout = Layout::from_size_align(GIVEN as usize, REGION_ALIGN).unwrap_or_else(|_| std::process::abort());
    // SAFETY: a layout of non-zero size.
    let p = unsafe { System.alloc_zeroed(layout) };
    if p.is_null() {
        // No memory for the kernel is no kernel; nothing can be reported
        // from inside the allocator that would need it.
        std::process::abort();
    }
    let addr = p.expose_provenance();
    match REGION.compare_exchange(0, addr, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {
            xpute_runtime::mem::base::set_base(addr, GIVEN as usize);
            addr
        }
        Err(won) => {
            // SAFETY: the block this call took, which nothing has seen.
            unsafe { System.dealloc(p, layout) };
            won
        }
    }
}

/// The heap: from the page past the last fixed range to the end of the
/// region, as the module's is to the end of its memory.
struct ModuleHeap;

/// The largest block the heap's tree can describe, as the module's (pxl-wasm).
const HEAP_MAX_LOG2: usize = 31;

/// The smallest block, which is the page the rest of the memory is counted in.
const HEAP_MIN_LOG2: usize = section::PAGE_BYTES.trailing_zeros() as usize;

/// A bit for every node of the tree that can be a parent.
const HEAP_SPLIT_BYTES: usize = (1 << (HEAP_MAX_LOG2 - HEAP_MIN_LOG2)) / 8;

impl xpute_core::alloc::buddy_tree::Range for ModuleHeap {
    const MIN_LOG2: usize = HEAP_MIN_LOG2;
    const MAX_LOG2: usize = HEAP_MAX_LOG2;
    type Split = [u8; HEAP_SPLIT_BYTES];
    const SPLIT_INIT: Self::Split = [0; HEAP_SPLIT_BYTES];

    fn base() -> usize {
        region() + section::heap_at(DATA_END) as usize
    }
    fn end() -> usize {
        region() + GIVEN as usize
    }
}

/// The slab behind a spin lock.
pub struct Locked {
    lock: AtomicBool,
    slab: SlabMalloc<ModuleHeap>,
}

impl Locked {
    fn with<R>(&self, f: impl FnOnce(&SlabMalloc<ModuleHeap>) -> R) -> R {
        while self.lock.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
        let r = f(&self.slab);
        self.lock.store(false, Ordering::Release);
        r
    }

    /// The allocator's reading, for the ledger and the heap's row.
    pub fn stat(&self) -> HeapStat {
        self.with(|a| HeapStat {
            range: a.range_bytes(),
            used: a.used(),
            live: a.live(),
            run: a.run_live(),
            slot: a.slot_live(),
            largest_free: a.largest_free(),
            releases: a.releases(),
            refusals: a.refusals(),
            requests: a.pages().allocs() as u32,
        })
    }
}

// SAFETY: every call into the slab holds the lock.
unsafe impl GlobalAlloc for Locked {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's layout, passed on.
        self.with(|a| unsafe { a.alloc(layout) })
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's block, passed on.
        self.with(|a| unsafe { a.dealloc(ptr, layout) })
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: as `alloc`.
        self.with(|a| unsafe { a.alloc_zeroed(layout) })
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: as `dealloc`.
        self.with(|a| unsafe { a.realloc(ptr, layout, new_size) })
    }
}

/// The library's one allocator.
#[global_allocator]
pub static ALLOC: Locked = Locked {
    lock: AtomicBool::new(false),
    slab: SlabMalloc::new(),
};
