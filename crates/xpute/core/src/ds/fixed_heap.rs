//! A heap over a fixed range of a linear memory, handed out in power-of-two
//! size classes: the allocator for a module whose memory is imported and
//! never grows. Not @xpute/core's arena (a bump pointer with mark and
//! rewind, for scratch with an order); this is what a `Vec` inside the
//! module allocates from. The default allocator asks for pages when it runs out and
//! would fail at its first request past the initial size; this one owns
//! the range its `Range` names and nothing else.
//!
//! Buddy blocks rather than a bump pointer, because a bump pointer leaks every
//! block a vector leaves behind on its way up unless it happens to be the last;
//! and buddies rather than a list per class, because a list per class only
//! ever serves its own size. With a kernel's worth of state in the range —
//! vectors of every size, tables reserved once, scratch that grows to a job —
//! the lists held what one size had given back while another size cut fresh
//! space from the top, until the top met the end with room free below it. A
//! request here takes the smallest free block that fits and halves it down to
//! its class; a freed block joins its buddy whenever that one is free too, back
//! up the classes, so space given back in one size serves any other and
//! neighbours become one block again. Exhausting the heap is then what is live
//! not fitting in one piece: the request returns null and the allocation traps.
//!
//! A global allocator can keep its state nowhere but in the memory it hands
//! out, so which blocks are free is a bitmap at the front of the range — a bit
//! for every place a block of every class could start, about a sixty-fourth of
//! the range — and a free block links its list in its own first two words.
//!
//! The state is reset whenever the module is instantiated: with the memory
//! imported the linker cannot assume it zeroed, so every static, the
//! heap's included, is written by the data segments at instantiation
//! (verified — a second instance over a memory the first left mid-job
//! starts clean).

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::marker::PhantomData;

/// The smallest class: two words of links, on a 64-bit host too.
const MIN_SHIFT: usize = 4;
const CLASSES: usize = 32 - MIN_SHIFT;
const WORD: usize = core::mem::size_of::<usize>();

/// The alignment the arena starts at. A block lies at a multiple of its own
/// size from there, so it is aligned to that size up to this; a request that
/// asks for more is refused.
const MAX_ALIGN: usize = 4096;

/// Where a heap's range lies: from `base()` up to `end()`, both read once, at
/// the first request — a linker symbol's address, or a buffer's.
pub trait Range {
    fn base() -> usize;
    fn end() -> usize;
}

/// Where untouched space starts, the requests served so far, the bytes of the
/// blocks handed out now, and where the arena starts past the bitmap; each
/// class's list of free blocks (a block's first word the next, its second the
/// previous); per class, the free blocks and the blocks handed out now; and
/// where each class's bits start in the bitmap, and the bitmap. The fields lead
/// in that order so a host reads them as a row (`stat`).
#[repr(C)]
struct Heap {
    top: usize,
    allocs: usize,
    live: usize,
    origin: usize,
    free: [usize; CLASSES],
    free_blocks: [u32; CLASSES],
    held: [u32; CLASSES],
    bit_at: [usize; CLASSES],
    bitmap: usize,
}

impl Heap {
    /// Lays the bitmap out at `base`, cleared, and the arena past it.
    ///
    /// # Safety
    /// `base..end` is memory this heap owns alone.
    unsafe fn open(&mut self, base: usize, end: usize) {
        let span = end - base;
        let mut bits = 0;
        for class in 0..CLASSES {
            self.bit_at[class] = bits;
            bits += (span >> (class + MIN_SHIFT)) + 1;
        }
        let bytes = bits.div_ceil(8);
        core::ptr::write_bytes(base as *mut u8, 0, bytes);
        self.bitmap = base;
        self.origin = (base + bytes + MAX_ALIGN - 1) & !(MAX_ALIGN - 1);
        self.top = self.origin;
    }

    /// The byte and the mask of the bit that says a block of `class` at `off`
    /// from the origin is free.
    fn bit(&self, class: usize, off: usize) -> (*mut u8, u8) {
        let i = self.bit_at[class] + (off >> (class + MIN_SHIFT));
        ((self.bitmap + i / 8) as *mut u8, 1 << (i % 8))
    }

    /// Puts the block at `off` on its class's list.
    ///
    /// # Safety
    /// The block is in the arena, cut and not handed out.
    unsafe fn push(&mut self, class: usize, off: usize) {
        let at = self.origin + off;
        let head = self.free[class];
        *(at as *mut usize) = head;
        *((at + WORD) as *mut usize) = 0;
        if head != 0 {
            *((head + WORD) as *mut usize) = at;
        }
        self.free[class] = at;
        self.free_blocks[class] += 1;
        let (byte, mask) = self.bit(class, off);
        *byte |= mask;
    }

    /// Takes the block at `off` off its class's list.
    ///
    /// # Safety
    /// The block is on that list.
    unsafe fn unlink(&mut self, class: usize, off: usize) {
        let at = self.origin + off;
        let (next, prev) = (*(at as *const usize), *((at + WORD) as *const usize));
        if prev == 0 {
            self.free[class] = next;
        } else {
            *(prev as *mut usize) = next;
        }
        if next != 0 {
            *((next + WORD) as *mut usize) = prev;
        }
        self.free_blocks[class] -= 1;
        let (byte, mask) = self.bit(class, off);
        *byte &= !mask;
    }

    /// Gives the block at `off` back, joined with its buddy for as long as the
    /// buddy is free: a buddy past the top has never been cut, so it is not.
    ///
    /// # Safety
    /// The block is in the arena, cut, and no longer handed out.
    unsafe fn release(&mut self, mut class: usize, mut off: usize) {
        while class + 1 < CLASSES {
            let size = 1 << (class + MIN_SHIFT);
            let buddy = off ^ size;
            if self.origin + buddy + size > self.top {
                break;
            }
            let (byte, mask) = self.bit(class, buddy);
            if *byte & mask == 0 {
                break;
            }
            self.unlink(class, buddy);
            off = off.min(buddy);
            class += 1;
        }
        self.push(class, off);
    }
}

pub struct FixedHeap<R: Range> {
    heap: UnsafeCell<Heap>,
    range: PhantomData<R>,
}

// SAFETY: the heap serves one thread — a module instantiated once per
// memory, which is what the imported-memory design guarantees.
unsafe impl<R: Range> Sync for FixedHeap<R> {}

impl<R: Range> FixedHeap<R> {
    pub const fn new() -> Self {
        FixedHeap {
            heap: UnsafeCell::new(Heap {
                top: 0,
                allocs: 0,
                live: 0,
                origin: 0,
                free: [0; CLASSES],
                free_blocks: [0; CLASSES],
                held: [0; CLASSES],
                bit_at: [0; CLASSES],
                bitmap: 0,
            }),
            range: PhantomData,
        }
    }

    /// Requests served since instantiation, reallocations that moved included:
    /// a span of work that allocates nothing leaves it where it was.
    pub fn allocs(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).allocs }
    }

    /// Where the heap's row lies — the top it has cut to (0 before the first
    /// request), `allocs`, `live` and the arena's origin, a word each, and
    /// past them the lists and the counts per class (`Heap`) — for a host to
    /// read in place.
    pub fn stat(&self) -> *const usize {
        self.heap.get() as *const usize
    }

    /// The bytes of the blocks handed out now, each at its class's size.
    pub fn live(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        unsafe { (*self.heap.get()).live }
    }

    /// How much of the arena has been cut, for a ledger.
    pub fn used(&self) -> usize {
        // SAFETY: single-threaded; see the Sync impl.
        let h = unsafe { &*self.heap.get() };
        h.top - h.origin
    }
}

impl<R: Range> Default for FixedHeap<R> {
    fn default() -> Self {
        Self::new()
    }
}

fn class_of(layout: Layout) -> usize {
    let bytes = layout.size().max(layout.align()).max(1 << MIN_SHIFT);
    (usize::BITS - (bytes - 1).leading_zeros()) as usize - MIN_SHIFT
}

unsafe impl<R: Range> GlobalAlloc for FixedHeap<R> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let h = &mut *self.heap.get();
        if h.top == 0 {
            h.open(R::base(), R::end());
        }
        h.allocs += 1;
        let class = class_of(layout);
        if class >= CLASSES || layout.align() > MAX_ALIGN {
            return core::ptr::null_mut();
        }
        let size = 1 << (class + MIN_SHIFT);
        let off = match (class..CLASSES).find(|&k| h.free[k] != 0) {
            // The smallest free block that fits, halved down to the class:
            // each upper half goes on the list a class below.
            Some(mut k) => {
                let off = h.free[k] - h.origin;
                h.unlink(k, off);
                while k > class {
                    k -= 1;
                    h.push(k, off + (1 << (k + MIN_SHIFT)));
                }
                off
            }
            // Fresh space: the top brought to a multiple of the block, each
            // piece stepped over given back as a block of its own.
            None => {
                let mut off = h.top - h.origin;
                while off & (size - 1) != 0 {
                    let piece = off & off.wrapping_neg();
                    if h.origin + off + piece > R::end() {
                        return core::ptr::null_mut();
                    }
                    h.top += piece;
                    h.release(piece.trailing_zeros() as usize - MIN_SHIFT, off);
                    off += piece;
                }
                if h.origin + off + size > R::end() {
                    return core::ptr::null_mut();
                }
                h.top = h.origin + off + size;
                off
            }
        };
        h.held[class] += 1;
        h.live += size;
        (h.origin + off) as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let h = &mut *self.heap.get();
        let class = class_of(layout);
        h.held[class] -= 1;
        h.live -= 1 << (class + MIN_SHIFT);
        h.release(class, ptr as usize - h.origin);
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
        fn end() -> usize {
            Self::base() + (1 << 16)
        }
    }

    #[test]
    fn a_freed_block_serves_the_next_request_of_its_class() {
        let heap: FixedHeap<Whole> = FixedHeap::new();
        let layout = Layout::from_size_align(100, 8).unwrap();
        unsafe {
            let a = heap.alloc(layout);
            assert_eq!(a as usize % MAX_ALIGN, 0, "the first block starts the arena, past the bitmap");
            let b = heap.alloc(layout);
            assert_eq!(b as usize, a as usize + 128, "a class rounds up to its power of two");
            heap.dealloc(a, layout);
            assert_eq!(heap.alloc(layout), a);
            // A realloc within the class keeps the block; past it, moves.
            assert_eq!(heap.realloc(b, layout, 120), b);
            let c = heap.realloc(b, layout, 200);
            assert_ne!(c, b);
        }
        assert_eq!(heap.used(), 128 * 2 + 256);
        assert_eq!(heap.allocs(), 4, "three requests and the one move");
        // SAFETY: the row the heap names, two words.
        assert_eq!(unsafe { *heap.stat().add(1) }, heap.allocs());
    }
}
