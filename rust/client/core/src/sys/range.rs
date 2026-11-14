// pxl-core/sys/range.rs

//! A fixed range is memory the core holds for good, in `.fixed`: past the
//! module's data and before the heap (sys/heap.rs), sized at compile time by
//! its owner's consts and never freed, so a large array takes no buddy block.
//! Only the core's own formats live in one; an external format (glTF, PNG)
//! has no bound to size it by, so it is read into the heap (sys/io/loader.rs).
//! link/layout.rs lays the ranges out and hands each owner its place at boot.
//! A range is read only through `arr`, which checks its end.

use pxl_global::Global;

use crate::sys::heap;

/// A cache line is this many bytes, and arrays are laid out in whole lines.
const LINE_BYTES: usize = 64;

/// The bytes an array of `cnt` items of `size` bytes takes, rounded up to
/// whole cache lines.
pub(crate) const fn arr_bytes(cnt: usize, size: usize) -> u32 {
    ((cnt * size).div_ceil(LINE_BYTES) * LINE_BYTES) as u32
}

/// A range: where it begins in `.fixed`, and its bytes.
#[derive(Clone, Copy)]
pub(crate) struct Range {
    off: u32,
    bytes: u32,
}

impl Range {
    pub(crate) const fn at(off: u32, bytes: u32) -> Range {
        Range { off, bytes }
    }

    /// An array of `n` items of `T` at `off` into the range. Sizes are fixed
    /// at compile time, so an array out of bounds or misaligned is a bug.
    ///
    /// # Safety
    /// The caller holds those bytes alone for as long as it keeps the array,
    /// and they are valid `T`s (zeroed, for plain numbers).
    pub(crate) unsafe fn arr<T>(self, off: u32, n: usize) -> &'static mut [T] {
        let end = (n * size_of::<T>()).checked_add(off as usize);
        xpute_kit::ensure!(
            end.is_some_and(|e| e <= self.bytes as usize),
            ENOTRECOVERABLE,
            off
        );
        let p = core::ptr::with_exposed_provenance_mut::<u8>(
            heap::fixed_at() + (self.off + off) as usize,
        );
        xpute_kit::ensure!(p.cast::<T>().is_aligned(), ENOTRECOVERABLE, off);
        // SAFETY: inside `.fixed`, which nothing but its owners writes,
        // aligned, and the caller's alone.
        unsafe { core::slice::from_raw_parts_mut(p.cast(), n) }
    }
}

/// One owner's range, handed in once at boot. Asking for it before then is a
/// bug.
pub(crate) struct OwnedRange(Global<Range>);

impl OwnedRange {
    pub(crate) const fn new() -> OwnedRange {
        OwnedRange(Global::new())
    }

    pub(crate) fn set(&self, r: Range) {
        self.0.set(r);
    }

    /// # Safety
    /// As `Range::arr`.
    pub(crate) unsafe fn arr<T>(&self, off: u32, n: usize) -> &'static mut [T] {
        let r = *self.0.get(|| xpute_kit::bug!(ENOTRECOVERABLE));
        // SAFETY: the caller's.
        unsafe { r.arr(off, n) }
    }
}
