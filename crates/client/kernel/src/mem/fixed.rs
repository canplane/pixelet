// pxl-kernel/mem/fixed.rs

//! What `.fixed` is to whoever holds a range of it: placed once at boot,
//! never freed, sized at compile time. Only the kernel's own formats belong
//! here; external ones (glTF, PNG) have no predictable bound and stay in the
//! heap. Which ranges there are and how large is the roof's
//! (door/layout.rs), which hands each owner its own at boot; a range is read
//! only through `lane` and `Section::at`, which check its end.

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_guest::global::Global;
use xpute_guest::mem::base::ptr_at;
use xpute_guest::mem::section::Section;

/// One owner's range, handed in once at boot; asking before is a bug.
pub struct Held(Global<Option<Section>>);

impl Default for Held {
    fn default() -> Held {
        Held::new()
    }
}

impl Held {
    pub const fn new() -> Held {
        Held(Global::new())
    }

    pub fn set(&self, s: Section) {
        self.0.set(Some(s));
    }

    pub fn get(&self) -> Section {
        self.0.get(|| None).or_bug(Errno::ENOTRECOVERABLE)
    }
}

/// `n` of `T` from `off` into `s`. Sizes are fixed at compile time, so out
/// of bounds or misaligned is a bug, not a refusal.
///
/// # Safety
/// The caller holds those bytes alone while it keeps the lane, and they are
/// a valid `T` (zeroed, for the plain numbers every lane here holds).
pub unsafe fn lane<T>(s: Section, off: u32, n: usize) -> &'static mut [T] {
    let at = u32::try_from(n * size_of::<T>()).ok().and_then(|bytes| s.at(off, bytes)).or_bug(Errno::ENOTRECOVERABLE);
    xpute_core::ensure!((at as usize).is_multiple_of(align_of::<T>()), ENOTRECOVERABLE, at);
    // SAFETY: inside `s`, aligned, and the caller's alone.
    unsafe { core::slice::from_raw_parts_mut(ptr_at::<T>(at as usize), n) }
}
