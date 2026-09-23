// xpute-core/alloc/slab.rs

//! Size classes over `buddy_tree.rs`: the level that lets a buddy whose
//! smallest block is a 4 KiB page answer a twenty-four-byte `String`.
//!
//! One range, two levels. The buddy hands out pages and runs of pages and
//! nothing smaller, which is what keeps its split bitmap at 32 KiB instead of
//! the 8 MiB a sixteen-byte leaf over the same range costs. A request under a
//! page takes a free slot of its class here; a class with no free slot borrows
//! a run below, carves it into slots, and **gives the run back when the last
//! slot in it is freed** — that return path is the whole reason the pair is
//! one allocator rather than two side by side, since what one class gives back
//! becomes available to every other class through the level below.
//!
//! A request of a page or more is not carved at all. It goes straight down,
//! and it is the common case rather than the edge: a band's lanes and a fetch
//! body are all far past a page.
//!
//! ## Where the bookkeeping lives
//!
//! In the run, the way SLUB does it rather than the way SLAB did. A free slot
//! is not in use by definition, so its own first word holds the address of the
//! next free slot; there is no bitmap and no per-object header, and a run is
//! its slots and nothing else. What a run needs besides — the head of that
//! free list, how many slots are out, and the links of the partial list its
//! class keeps — sits in the run's leading slots, which is the one thing SLUB
//! does not need because Linux already has a `struct page` per page to put it
//! in. Nothing here has a page descriptor array, and one would be proportional
//! to the range rather than to what is live, which is the cost this whole
//! arrangement exists to avoid.
//!
//! ## How a slot finds its run
//!
//! By masking, with no table. The buddy places a block of size `S` at an
//! offset from the range's base that is a multiple of `S`, so a slot's run
//! begins at `base + ((slot - base) & !(run_bytes - 1))`. The masking is of
//! the offset and not of the address because the range's base is only known to
//! be a page boundary: it is where the ranges in front of it happened to end,
//! rounded up, and nothing makes it a multiple of the largest run.
//!
//! `dealloc` is handed the `Layout` the allocation was made with, so the class
//! — and with it `run_bytes` — is known before the mask is needed.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr;

use super::buddy_tree::{BuddyMalloc, Range};

/// The smallest class. Below sixteen bytes a class costs more in run headers
/// and rounding than it saves, and a free slot has to hold one pointer.
const MIN_CLASS_LOG2: usize = 4;

/// Classes, sized for any smallest block a 32-bit address space allows rather
/// than for the range's own: a head is one word, and the classes past
/// `class_count` are never named. The largest class is the one below the
/// buddy's smallest block — that block or more is the buddy's own business and
/// is passed down whole.
const MAX_CLASSES: usize = 32;

/// The fewest slots a run is cut into. A run's leading slots are spent on its
/// own header, so a run of two slots would spend half of itself on it; sixteen
/// caps that at a sixteenth, which is what sizes the runs of the large classes
/// above one page.
const MIN_SLOTS_LOG2: usize = 4;

/// What a run keeps about itself, in its own leading slots.
///
/// `prev`/`next` thread the partial list of the run's class — the runs with at
/// least one free slot. A full run is in no list, and is put back on one by
/// the free that gives it a slot again; an empty run is in no list either,
/// having gone back to the buddy.
#[repr(C)]
struct Run {
    prev: *mut Run,
    next: *mut Run,
    free: *mut u8,
    inuse: u32,
}

const _: () = assert!(1 << MIN_CLASS_LOG2 >= core::mem::size_of::<*mut u8>(), "a free slot holds the address of the next");

/// `1 << class_log2(c)` is the bytes a slot of class `c` holds.
const fn class_log2(c: usize) -> usize {
    MIN_CLASS_LOG2 + c
}

/// A slab of size classes over a buddy of pages, as the module's
/// `#[global_allocator]`.
pub struct SlabMalloc<R: Range> {
    buddy: BuddyMalloc<R>,
    /// The head of each class's partial list, null where the class has no run
    /// with a free slot.
    partial: UnsafeCell<[*mut Run; MAX_CLASSES]>,
    /// Requests served, both paths, which is what a per-step probe counts. The
    /// buddy's own counter moves only when a run is borrowed.
    allocs: UnsafeCell<u32>,
    /// Requests answered with null, both paths: the one way the heap is full.
    refusals: UnsafeCell<u32>,
    /// Bytes borrowed from the buddy as runs, and of those the bytes out in
    /// slots, each at its class's size. The buddy counts a run whole the
    /// moment it is borrowed, so its `live` cannot say how much of the heap
    /// anything actually holds; the difference between these two is what the
    /// carving costs — a run's header slots and every slot standing free in a
    /// run that cannot go back until its last one does.
    run_live: UnsafeCell<usize>,
    slot_live: UnsafeCell<usize>,
}

// SAFETY: one thread — a module instantiated once per memory, which is what
// the imported-memory design guarantees.
unsafe impl<R: Range> Sync for SlabMalloc<R> {}

impl<R: Range> Default for SlabMalloc<R> {
    fn default() -> SlabMalloc<R> {
        SlabMalloc::new()
    }
}

impl<R: Range> SlabMalloc<R> {
    /// The classes this range has: everything from MIN_CLASS_LOG2 up to the
    /// one below the buddy's smallest block.
    const CLASS_COUNT: usize = R::MIN_LOG2 - MIN_CLASS_LOG2;

    /// `1 << run_log2(c)` is the bytes a run of class `c` borrows from the
    /// buddy: one of the buddy's smallest blocks, or as much more as
    /// MIN_SLOTS_LOG2 asks for.
    const fn run_log2(c: usize) -> usize {
        let by_slots = class_log2(c) + MIN_SLOTS_LOG2;
        if by_slots > R::MIN_LOG2 {
            by_slots
        } else {
            R::MIN_LOG2
        }
    }

    /// The leading slots a run of class `c` spends on its own header.
    const fn head_slots(c: usize) -> usize {
        core::mem::size_of::<Run>().div_ceil(1usize << class_log2(c))
    }

    /// The slots a run of class `c` is cut into, its header's included.
    const fn slots(c: usize) -> usize {
        1 << (Self::run_log2(c) - class_log2(c))
    }

    /// Checked once, at `new`: a run has a slot to hand out past the ones its
    /// header takes, and the classes fit the heads there are.
    const CHECK: () = {
        assert!(Self::CLASS_COUNT <= MAX_CLASSES, "a range's smallest block asks for more classes than there are heads");
        let mut c = 0;
        while c < Self::CLASS_COUNT {
            assert!(Self::head_slots(c) < Self::slots(c), "a run has a slot to hand out past the ones its header takes");
            c += 1;
        }
    };

    /// The class a layout falls in, or none where it belongs to the buddy.
    ///
    /// A slot of class `c` sits at a multiple of `c` from a run base that is
    /// itself a multiple of its own size, so a class at least as large as the
    /// alignment satisfies it — which is why the alignment is folded into the
    /// size rather than handled apart from it.
    fn class_for(layout: Layout) -> Option<usize> {
        let need = layout.size().max(layout.align()).max(1);
        if need >= 1 << R::MIN_LOG2 {
            return None;
        }
        let log2 = (need.next_power_of_two().trailing_zeros() as usize).max(MIN_CLASS_LOG2);
        Some(log2 - MIN_CLASS_LOG2)
    }

    pub const fn new() -> SlabMalloc<R> {
        // Forces the checks above to be evaluated: an associated const is only
        // checked where it is named.
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

    /// The page allocator underneath, for a caller that wants whole pages and
    /// says so.
    pub fn pages(&self) -> &BuddyMalloc<R> {
        &self.buddy
    }

    /// Bytes of the range that have ever been used.
    pub fn used(&self) -> usize {
        self.buddy.used()
    }

    /// Bytes borrowed from the range and not yet given back, runs at their
    /// whole size — not the slots that are live inside them.
    pub fn live(&self) -> usize {
        self.buddy.live()
    }

    /// The bytes of the range the allocator was given.
    pub fn range_bytes(&self) -> usize {
        self.buddy.range_bytes()
    }

    /// Blocks given back to the range. A request refused before might be
    /// served once this has moved, and not before — which is true of the page
    /// level and only of it, a slot returning to its run freeing nothing the
    /// range can hand to someone else.
    pub fn releases(&self) -> u32 {
        self.buddy.releases()
    }

    /// Requests served since the first call, slots and pages alike.
    pub fn allocs(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { *self.allocs.get() as usize }
    }

    /// Requests refused since the first call. A slot request refused because
    /// no run could be borrowed counts once, here, and not again below.
    pub fn refusals(&self) -> u32 {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { *self.refusals.get() }
    }

    /// The largest block on the page level's free lists.
    pub fn largest_free(&self) -> usize {
        self.buddy.largest_free()
    }

    /// Bytes borrowed from the range as runs to be carved into slots. What
    /// `live` holds besides this went straight down as whole pages.
    pub fn run_live(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { *self.run_live.get() }
    }

    /// Bytes out in slots, each at its class's size: what callers of a
    /// sub-page request actually hold. `run_live` less this is what the
    /// carving costs.
    pub fn slot_live(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { *self.slot_live.get() }
    }

    /// Where the page level's row lies, for a host to read in place.
    pub fn stat(&self) -> *const usize {
        self.buddy.stat()
    }

    /// The address a request of `bytes` was given, or null. The pair of
    /// `free`, for a caller that has an address and a size rather than a
    /// `Layout`.
    ///
    /// # Safety
    /// The range `R` names must be this module's.
    pub unsafe fn malloc(&self, bytes: usize) -> *mut u8 {
        // SAFETY: the caller's.
        unsafe { self.alloc(Self::layout(bytes)) }
    }

    /// Gives back what `malloc` returned, told the size it asked for.
    ///
    /// # Safety
    /// `at` came from `malloc(bytes)` and has not been given back.
    pub unsafe fn free(&self, at: *mut u8, bytes: usize) {
        // SAFETY: the caller's.
        unsafe { self.dealloc(at, Self::layout(bytes)) }
    }

    /// What `malloc`/`free` stand in for: the word alignment every block of
    /// this kernel is read at.
    fn layout(bytes: usize) -> Layout {
        // SAFETY: 8 is a power of two, and the size cannot overflow a range
        // that fits in the module's memory.
        unsafe { Layout::from_size_align_unchecked(bytes, 8) }
    }

    /// Borrows a run for class `c`, threads its free list through its slots,
    /// and puts it at the head of the class's partial list. Null where the
    /// buddy has no room.
    unsafe fn open_run(&self, c: usize) -> *mut Run {
        let run_bytes = 1usize << Self::run_log2(c);
        // SAFETY: the buddy's own range.
        let at = unsafe { self.buddy.malloc(run_bytes) };
        if at.is_null() {
            return ptr::null_mut();
        }
        let class = 1usize << class_log2(c);
        let mut next: *mut u8 = ptr::null_mut();
        // From the back, so the list runs forwards through the run and a run's
        // first allocations are its lowest addresses.
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

// SAFETY: the allocator hands out slots of runs it borrowed, or whole blocks
// of the range, each aligned to its own size and never overlapping while it is
// out.
unsafe impl<R: Range> GlobalAlloc for SlabMalloc<R> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: single-threaded; see the Sync impl.
        let allocs = unsafe { &mut *self.allocs.get() };
        // SAFETY: single-threaded; see the Sync impl.
        let refused = || unsafe { *self.refusals.get() = (*self.refusals.get()).wrapping_add(1) };

        let Some(c) = Self::class_for(layout) else {
            // A page or more. The buddy places a block at a multiple of its
            // own size from a base that is a page boundary, so it satisfies
            // any alignment up to a page and none past one.
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
                // The last slot: the run goes back, and every class gets the
                // pages rather than this one keeping them.
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
