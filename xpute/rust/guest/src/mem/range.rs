// xpute-guest/mem/range.rs

//! A range of the guest's own ahead of its heap: placed once at boot, never
//! freed, sized at compile time. Whoever lays the memory out hands each owner
//! its range once (`OwnedRange`); a range is read only through `arr` and
//! `Section::at`, which check its end.

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::global::Global;
use crate::mem;
use crate::mem::section::Section;

/// One owner's range, handed in once at boot; asking before is a bug.
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

/// A cache line, which an array is laid on.
const LINE_BYTES: usize = 64;

/// `cnt` of `size` bytes, rounded up to a whole cache line, as an array is laid.
pub const fn arr_bytes(cnt: usize, size: usize) -> u32 {
    ((cnt * size).div_ceil(LINE_BYTES) * LINE_BYTES) as u32
}

/// `n` of `T` from `off` into `s`, in the guest's own memory. Sizes are fixed
/// at compile time, so out of bounds or misaligned is a bug, not a refusal.
///
/// # Safety
/// The caller holds those bytes alone while it keeps the array, and they are
/// a valid `T` (zeroed, for plain numbers).
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
