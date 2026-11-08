// xpute-core/mem/range.rs

//! The core's fixed ranges are ranges of its own memory below its heap, sized
//! at compile time, placed once at boot and never freed. Whoever lays out the
//! memory hands each owner its range once (`OwnedRange`). A range is read only
//! through `arr` and `Section::at`, which check its end.

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::global::Global;
use crate::mem;
use crate::mem::section::Section;

/// This is one owner's range, handed in once at boot. Asking for it before then
/// is a bug.
pub struct OwnedRange(Global<Option<Section>>);

impl Default for OwnedRange {
    fn default() -> OwnedRange {
        OwnedRange::new()
    }
}

impl OwnedRange {
    pub const fn new() -> OwnedRange {
        OwnedRange(Global::new())
    }

    pub fn set(&self, s: Section) {
        self.0.set(Some(s));
    }

    pub fn get(&self) -> Section {
        self.0.get(|| None).or_bug(Errno::ENOTRECOVERABLE)
    }
}

/// A cache line is this many bytes, and arrays are laid out in whole lines.
const LINE_BYTES: usize = 64;

/// This is the bytes an array of `cnt` items of `size` bytes takes, rounded up
/// to whole cache lines.
pub const fn arr_bytes(cnt: usize, size: usize) -> u32 {
    ((cnt * size).div_ceil(LINE_BYTES) * LINE_BYTES) as u32
}

/// `arr` answers an array of `n` items of `T`, at `off` into `s` in the core's
/// own memory. Sizes are fixed at compile time, so an array out of bounds or
/// misaligned is a bug, not a refusal.
///
/// # Safety
/// The caller must hold those bytes alone for as long as it keeps the array,
/// and they must be valid `T`s (zeroed, for plain numbers).
pub unsafe fn arr<T>(s: Section, off: u32, n: usize) -> &'static mut [T] {
    let at = u32::try_from(n * size_of::<T>())
        .ok()
        .and_then(|bytes| s.at(off, bytes))
        .or_bug(Errno::ENOTRECOVERABLE);
    xpute_kit::ensure!(
        (at as usize).is_multiple_of(align_of::<T>()),
        ENOTRECOVERABLE,
        at
    );
    // SAFETY: inside `s`, aligned, and the caller's alone.
    unsafe { core::slice::from_raw_parts_mut(mem::at::<T>(at as usize), n) }
}

#[cfg(test)]
#[path = "range.test.rs"]
mod test;
