// xpute-kit/alloc/slab.rs

//! This is a slab allocator of size classes for requests smaller than a page,
//! over the buddy allocator of `buddy_tree.rs`, whose smallest block is a page.
//! Each class borrows a run of pages from the buddy and carves it into slots
//! as SLUB does: the free slots form a list threaded through themselves, and
//! the run's header sits in its leading slots. A run goes back to the buddy
//! when its last slot is freed. A request of a page or more goes straight to
//! the buddy.
//!
//! A slot finds its run by masking its offset from the range's base, which
//! works because the buddy aligns every block to its own size from the base.
//! Masking the address would not: the base is aligned only to a page, and a
//! run can be larger than a page.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr;

use super::buddy_tree::{BuddyMalloc, Range};

/// The smallest class is sixteen bytes: below that, a class costs more in
/// run headers than it saves.
const MIN_CLASS_LOG2: usize = 4;

/// Enough partial list heads for a range whose smallest block is any size
/// the address space allows.
const MAX_CLASSES: usize = super::ADDR_BITS;

/// A run holds at least sixteen slots, so its header takes a small share of
/// the run.
const MIN_SLOTS_LOG2: usize = 4;

/// A run's header, kept in the run's leading slots. `prev` and `next` link
/// the run into its class's partial list, the runs that have a free slot. A
/// full run, or one given back to the buddy, is in no list.
#[repr(C)]
struct Run {
    prev: *mut Run,
    next: *mut Run,
    free: *mut u8,
    inuse: u32,
}

const _: () = assert!(
    1 << MIN_CLASS_LOG2 >= size_of::<*mut u8>(),
    "a free slot holds the address of the next"
);

const fn class_log2(c: usize) -> usize {
    MIN_CLASS_LOG2 + c
}

pub struct SlabMalloc<R: Range> {
    buddy: BuddyMalloc<R>,
    partial: UnsafeCell<[*mut Run; MAX_CLASSES]>,
    allocs: UnsafeCell<u32>,
    refusals: UnsafeCell<u32>,
    /// The bytes of the runs that are out, and of the slots handed out from
    /// them. `run_live - slot_live` is what carving runs into slots costs.
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
    const CLASS_CNT: usize = R::MIN_LOG2 - MIN_CLASS_LOG2;

    const fn run_log2(c: usize) -> usize {
        let by_slots = class_log2(c) + MIN_SLOTS_LOG2;
        if by_slots > R::MIN_LOG2 {
            by_slots
        } else {
            R::MIN_LOG2
        }
    }

    const fn head_slots(c: usize) -> usize {
        size_of::<Run>().div_ceil(1usize << class_log2(c))
    }

    const fn slots(c: usize) -> usize {
        1 << (Self::run_log2(c) - class_log2(c))
    }

    const CHECK: () = {
        assert!(
            Self::CLASS_CNT <= MAX_CLASSES,
            "a range's smallest block asks for more classes than there are heads"
        );
        let mut c = 0;
        while c < Self::CLASS_CNT {
            assert!(
                Self::head_slots(c) < Self::slots(c),
                "a run has a slot to hand out past the ones its header takes"
            );
            c += 1;
        }
    };

    /// `class_for` gives the class for a request, or None when the buddy
    /// serves it. A slot is aligned to its own size, so a request's alignment
    /// is met by raising its size to the alignment.
    fn class_for(layout: Layout) -> Option<usize> {
        let need = layout.size().max(layout.align()).max(1);
        if need >= 1 << R::MIN_LOG2 {
            return None;
        }
        let log2 = (need.next_power_of_two().trailing_zeros() as usize).max(MIN_CLASS_LOG2);
        Some(log2 - MIN_CLASS_LOG2)
    }

    pub const fn new() -> SlabMalloc<R> {
        // Naming `CHECK` makes the compiler evaluate its assertions: an
        // associated const is evaluated only where it is named.
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

    pub fn peak(&self) -> usize {
        self.buddy.peak()
    }

    /// Bytes live as the buddy counts them: a run counts whole, however few
    /// of its slots are in use.
    pub fn live(&self) -> usize {
        self.buddy.live()
    }

    pub fn range_sz(&self) -> usize {
        self.buddy.range_sz()
    }

    /// Counts only the blocks given back to the buddy, not a slot freed into
    /// a run that stays in use. Such a slot could serve a refused request of
    /// its class, but a caller that waits for this count to change before it
    /// retries waits for a block instead.
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
    /// The range `R` names must be this module's own memory.
    pub unsafe fn malloc(&self, bytes: usize) -> *mut u8 {
        // SAFETY: the caller's.
        unsafe { self.alloc(Self::layout(bytes)) }
    }

    /// # Safety
    /// `at` was returned by `malloc` with the same `bytes`, and has not been
    /// freed since.
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
        let run_sz = 1usize << Self::run_log2(c);
        // SAFETY: the buddy's own range.
        let at = unsafe { self.buddy.malloc(run_sz) };
        if at.is_null() {
            return ptr::null_mut();
        }
        let class_sz = 1usize << class_log2(c);
        let mut next: *mut u8 = ptr::null_mut();
        // The free list is built from the last slot back, so the first slots
        // handed out have the lowest addresses.
        let mut i = Self::slots(c);
        while i > Self::head_slots(c) {
            i -= 1;
            // SAFETY: `i` is below `slots(c)`, and `slots(c)` slots of
            // `class_sz` bytes fill the run of `run_sz` bytes just taken.
            let slot = unsafe { at.add(i * class_sz) };
            // SAFETY: the slot is free, and a class is at least one pointer.
            unsafe { *slot.cast::<*mut u8>() = next };
            next = slot;
        }
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { *self.run_live.get() += run_sz };
        let run = at.cast::<Run>();
        // SAFETY: the run's leading slots, which head_slots keeps for this.
        unsafe {
            (*run).free = next;
            (*run).inuse = 0;
        }
        // SAFETY: the run is of class `c` and was just taken from the buddy,
        // so it is in no list.
        unsafe { self.push_partial(c, run) };
        run
    }

    /// # Safety
    /// `run` is a run of class `c` that is in no list.
    unsafe fn push_partial(&self, c: usize, run: *mut Run) {
        // SAFETY: single-threaded; see the Sync impl.
        let partial = unsafe { &mut *self.partial.get() };
        let head = partial[c];
        // SAFETY: `run` is a run in no list, the caller's, and `head`, where
        // not null, is the first run of class `c`'s list, which is live.
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
        // SAFETY: `run` is in class `c`'s partial list, the caller's, so its
        // neighbours, where not null, are live runs of that list.
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
            // The range's base is aligned only to a page, so no block is sure
            // of a larger alignment.
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
            (*run).free = *slot.cast::<*mut u8>();
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

        let run_sz = 1usize << Self::run_log2(c);
        let base = R::base();
        let run = (base + ((ptr as usize - base) & !(run_sz - 1))) as *mut Run;

        // SAFETY: the caller's — a slot of this run, of this class.
        unsafe {
            let was_full = (*run).free.is_null();
            *ptr.cast::<*mut u8>() = (*run).free;
            (*run).free = ptr;
            (*run).inuse -= 1;
            *self.slot_live.get() -= 1usize << class_log2(c);

            if (*run).inuse == 0 {
                if !was_full {
                    self.drop_partial(c, run);
                }
                *self.run_live.get() -= run_sz;
                self.buddy.free(run.cast::<u8>(), run_sz);
            } else if was_full {
                self.push_partial(c, run);
            }
        }
    }
}

#[cfg(test)]
#[path = "slab.test.rs"]
mod test;
