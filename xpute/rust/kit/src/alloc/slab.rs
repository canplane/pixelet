// xpute-kit/alloc/slab.rs

//! Size classes over `buddy_tree.rs`, whose smallest block is a page (a
//! 16-byte leaf would cost an 8 MiB split bitmap instead of 32 KiB). A class
//! borrows a run of pages, carves it into slots SLUB-style (free list through
//! the free slots, header in the run's leading slots), and gives the run back
//! when its last slot is freed. A page or more goes straight to the buddy.
//!
//! A slot finds its run by masking its offset from the range's base, not its
//! address: the base is only page-aligned.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr;

use super::buddy_tree::{BuddyMalloc, Range};

/// Below sixteen bytes a class costs more in headers than it saves.
const MIN_CLASS_LOG2: usize = 4;

/// Enough for any smallest block in a 32-bit address space.
const MAX_CLASSES: usize = 32;

/// At least sixteen slots a run, so its header takes at most a sixteenth.
const MIN_SLOTS_LOG2: usize = 4;

/// A run's header, in its leading slots. `prev`/`next` link the class's
/// partial list; a full or returned run is in no list.
#[repr(C)]
struct Run {
    prev: *mut Run,
    next: *mut Run,
    free: *mut u8,
    inuse: u32,
}

const _: () = assert!(1 << MIN_CLASS_LOG2 >= core::mem::size_of::<*mut u8>(), "a free slot holds the address of the next");

const fn class_log2(c: usize) -> usize {
    MIN_CLASS_LOG2 + c
}

pub struct SlabMalloc<R: Range> {
    buddy: BuddyMalloc<R>,
    partial: UnsafeCell<[*mut Run; MAX_CLASSES]>,
    allocs: UnsafeCell<u32>,
    refusals: UnsafeCell<u32>,
    /// `run_live - slot_live` is what carving costs; the buddy's `live`
    /// counts a run whole.
    run_live: UnsafeCell<usize>,
    slot_live: UnsafeCell<usize>,
}

// SAFETY: one thread; a module is instantiated once per memory.
unsafe impl<R: Range> Sync for SlabMalloc<R> {}

impl<R: Range> Default for SlabMalloc<R> {
    fn default() -> SlabMalloc<R> {
        SlabMalloc::new()
    }
}

impl<R: Range> SlabMalloc<R> {
    const CLASS_COUNT: usize = R::MIN_LOG2 - MIN_CLASS_LOG2;

    const fn run_log2(c: usize) -> usize {
        let by_slots = class_log2(c) + MIN_SLOTS_LOG2;
        if by_slots > R::MIN_LOG2 {
            by_slots
        } else {
            R::MIN_LOG2
        }
    }

    const fn head_slots(c: usize) -> usize {
        core::mem::size_of::<Run>().div_ceil(1usize << class_log2(c))
    }

    const fn slots(c: usize) -> usize {
        1 << (Self::run_log2(c) - class_log2(c))
    }

    const CHECK: () = {
        assert!(Self::CLASS_COUNT <= MAX_CLASSES, "a range's smallest block asks for more classes than there are heads");
        let mut c = 0;
        while c < Self::CLASS_COUNT {
            assert!(Self::head_slots(c) < Self::slots(c), "a run has a slot to hand out past the ones its header takes");
            c += 1;
        }
    };

    /// None where it belongs to the buddy. A slot is aligned to its class, so
    /// alignment folds into size.
    fn class_for(layout: Layout) -> Option<usize> {
        let need = layout.size().max(layout.align()).max(1);
        if need >= 1 << R::MIN_LOG2 {
            return None;
        }
        let log2 = (need.next_power_of_two().trailing_zeros() as usize).max(MIN_CLASS_LOG2);
        Some(log2 - MIN_CLASS_LOG2)
    }

    pub const fn new() -> SlabMalloc<R> {
        // An associated const is only evaluated where it is named.
        let () = Self::CHECK;
        SlabMalloc {
            buddy: BuddyMalloc::new(),
            partial: UnsafeCell::new([ptr::null_mut(); MAX_CLASSES]),
            allocs: UnsafeCell::new(0),
            refusals: UnsafeCell::new(0),
            run_live: UnsafeCell::new(0),
            slot_live: UnsafeCell::new(0),
        }
    }

    pub fn pages(&self) -> &BuddyMalloc<R> {
        &self.buddy
    }

    pub fn used(&self) -> usize {
        self.buddy.used()
    }

    /// Runs count whole, not their live slots.
    pub fn live(&self) -> usize {
        self.buddy.live()
    }

    pub fn range_bytes(&self) -> usize {
        self.buddy.range_bytes()
    }

    /// Page-level releases only: a slot returning to its run frees nothing
    /// another request could use.
    pub fn releases(&self) -> u32 {
        self.buddy.releases()
    }

    pub fn allocs(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { *self.allocs.get() as usize }
    }

    pub fn refusals(&self) -> u32 {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { *self.refusals.get() }
    }

    pub fn largest_free(&self) -> usize {
        self.buddy.largest_free()
    }

    pub fn run_live(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { *self.run_live.get() }
    }

    pub fn slot_live(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { *self.slot_live.get() }
    }

    pub fn stat(&self) -> *const usize {
        self.buddy.stat()
    }

    /// # Safety
    /// The range `R` names must be this module's.
    pub unsafe fn malloc(&self, bytes: usize) -> *mut u8 {
        // SAFETY: the caller's.
        unsafe { self.alloc(Self::layout(bytes)) }
    }

    /// # Safety
    /// `at` came from `malloc(bytes)` and has not been given back.
    pub unsafe fn free(&self, at: *mut u8, bytes: usize) {
        // SAFETY: the caller's.
        unsafe { self.dealloc(at, Self::layout(bytes)) }
    }

    fn layout(bytes: usize) -> Layout {
        // SAFETY: 8 is a power of two, and the size cannot overflow a range
        // that fits in the module's memory.
        unsafe { Layout::from_size_align_unchecked(bytes, 8) }
    }

    unsafe fn open_run(&self, c: usize) -> *mut Run {
        let run_bytes = 1usize << Self::run_log2(c);
        // SAFETY: the buddy's own range.
        let at = unsafe { self.buddy.malloc(run_bytes) };
        if at.is_null() {
            return ptr::null_mut();
        }
        let class = 1usize << class_log2(c);
        let mut next: *mut u8 = ptr::null_mut();
        // From the back, so the first allocations are the lowest addresses.
        let mut i = Self::slots(c);
        while i > Self::head_slots(c) {
            i -= 1;
            let slot = unsafe { at.add(i * class) };
            // SAFETY: the slot is free, and a class is at least one pointer.
            unsafe { *(slot as *mut *mut u8) = next };
            next = slot;
        }
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { *self.run_live.get() += run_bytes };
        let run = at as *mut Run;
        // SAFETY: the run's leading slots, which head_slots keeps for this.
        unsafe {
            (*run).free = next;
            (*run).inuse = 0;
        }
        unsafe { self.push_partial(c, run) };
        run
    }

    /// # Safety
    /// `run` is a run of class `c` that is in no list.
    unsafe fn push_partial(&self, c: usize, run: *mut Run) {
        // SAFETY: single-threaded; see the Sync impl.
        let partial = unsafe { &mut *self.partial.get() };
        let head = partial[c];
        unsafe {
            (*run).prev = ptr::null_mut();
            (*run).next = head;
            if !head.is_null() {
                (*head).prev = run;
            }
        }
        partial[c] = run;
    }

    /// # Safety
    /// `run` is a run of class `c` that is in that class's partial list.
    unsafe fn drop_partial(&self, c: usize, run: *mut Run) {
        // SAFETY: single-threaded; see the Sync impl.
        let partial = unsafe { &mut *self.partial.get() };
        unsafe {
            let (prev, next) = ((*run).prev, (*run).next);
            if prev.is_null() {
                partial[c] = next;
            } else {
                (*prev).next = next;
            }
            if !next.is_null() {
                (*next).prev = prev;
            }
        }
    }
}

// SAFETY: slots and blocks are aligned to their size and never overlap while out.
unsafe impl<R: Range> GlobalAlloc for SlabMalloc<R> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: single-threaded; see the Sync impl.
        let allocs = unsafe { &mut *self.allocs.get() };
        // SAFETY: single-threaded; see the Sync impl.
        let refused = || unsafe { *self.refusals.get() = (*self.refusals.get()).wrapping_add(1) };

        let Some(c) = Self::class_for(layout) else {
            // The range's base is only page-aligned.
            if layout.align() > 1 << R::MIN_LOG2 {
                refused();
                return ptr::null_mut();
            }
            // SAFETY: the caller's.
            let at = unsafe { self.buddy.malloc(layout.size()) };
            if at.is_null() {
                refused();
            } else {
                *allocs = allocs.wrapping_add(1);
            }
            return at;
        };

        // SAFETY: single-threaded; see the Sync impl.
        let mut run = unsafe { (*self.partial.get())[c] };
        if run.is_null() {
            // SAFETY: the class has no run with a free slot.
            run = unsafe { self.open_run(c) };
            if run.is_null() {
                refused();
                return ptr::null_mut();
            }
        }

        // SAFETY: a run on the partial list has a free slot by definition.
        unsafe {
            let slot = (*run).free;
            (*run).free = *(slot as *mut *mut u8);
            (*run).inuse += 1;
            if (*run).free.is_null() {
                self.drop_partial(c, run);
            }
            *self.slot_live.get() += 1usize << class_log2(c);
            *allocs = allocs.wrapping_add(1);
            slot
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(c) = Self::class_for(layout) else {
            // SAFETY: the caller's — the same size the block was asked for.
            return unsafe { self.buddy.free(ptr, layout.size()) };
        };

        let run_bytes = 1usize << Self::run_log2(c);
        let base = R::base();
        let run = (base + ((ptr as usize - base) & !(run_bytes - 1))) as *mut Run;

        // SAFETY: the caller's — a slot of this run, of this class.
        unsafe {
            let was_full = (*run).free.is_null();
            *(ptr as *mut *mut u8) = (*run).free;
            (*run).free = ptr;
            (*run).inuse -= 1;
            *self.slot_live.get() -= 1usize << class_log2(c);

            if (*run).inuse == 0 {
                if !was_full {
                    self.drop_partial(c, run);
                }
                *self.run_live.get() -= run_bytes;
                self.buddy.free(run as *mut u8, run_bytes);
            } else if was_full {
                self.push_partial(c, run);
            }
        }
    }
}

#[cfg(test)]
#[path = "slab.test.rs"]
mod test;
