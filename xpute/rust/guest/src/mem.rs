// xpute-guest/mem.rs

//! Every address a module names is an offset from a memory's base, never a
//! pointer, so it means the same on every host and fits a `u32`. A part laid
//! over a region (the rings, the streams, the directory) takes the `Memory`
//! it lies in; a program reading its own data goes through `core`, the
//! guest's own, whose base is 0 on wasm32 and in a native test, and a native
//! build's region's start.

pub mod heap;
pub mod section;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

#[cfg(not(target_arch = "wasm32"))]
use crate::global::Global;

/// The memory both ends see: every address is an offset from its base.
#[derive(Clone, Copy, Debug)]
pub struct Memory {
    base: usize,
    bytes: usize,
}

impl Memory {
    /// # Safety
    /// `base..base + bytes` is memory both ends may read and write, and stays
    /// where it is for as long as the `Memory` or a part laid over it lives.
    pub const unsafe fn new(base: usize, bytes: usize) -> Memory {
        Memory { base, bytes }
    }

    pub fn base(self) -> usize {
        self.base
    }

    /// Made from the address, not by `ptr::add` on the wasm base, which is UB:
    /// a release build once dropped a ring's capacity word for it. `off_of`
    /// exposed the provenance this recovers.
    pub fn at<T>(self, off: usize) -> *mut T {
        core::ptr::with_exposed_provenance_mut(self.base + off)
    }

    /// By copy: an offset from the other end carries no alignment a compiler
    /// may assume. A word past the memory's end is a bug.
    #[track_caller]
    pub fn word(self, off: usize) -> u32 {
        self.within(off, 4);
        // SAFETY: inside the memory, which `new`'s caller keeps valid.
        unsafe { self.at::<u32>(off).read_unaligned() }
    }

    /// As `word`.
    #[track_caller]
    pub fn set_word(self, off: usize, w: u32) {
        self.within(off, 4);
        // SAFETY: as `word`.
        unsafe { self.at::<u32>(off).write_unaligned(w) }
    }

    #[track_caller]
    fn within(self, off: usize, len: usize) {
        off.checked_add(len).filter(|&end| end <= self.bytes).or_bug(Errno::EFAULT);
    }

    /// A pointer outside the memory is a bug; on a native build that includes
    /// statics and read-only data.
    #[track_caller]
    pub fn off_of<T>(self, p: *const T) -> usize {
        p.expose_provenance().checked_sub(self.base).filter(|&off| off < self.bytes).or_bug(Errno::EFAULT)
    }

    /// An empty slice's pointer is only its alignment, possibly outside the
    /// region, so it names 0. Pass a slice the host writes into as `&mut`.
    #[track_caller]
    pub fn off_of_slice<T>(self, s: *const [T]) -> usize {
        if s.is_empty() {
            0
        } else {
            self.off_of(s.cast::<T>())
        }
    }
}

/// The guest's own memory.
#[cfg(not(target_arch = "wasm32"))]
static CORE: Global<Memory> = Global::new();

/// The guest's own memory, set once, before anything is allocated, on the
/// guest's thread: it is a `Global`, and a thread marked as another's may not
/// write it.
///
/// # Safety
/// As `Memory::new`'s, for the guest's whole life.
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn set(addr: usize, bytes: usize) {
    assert!(!crate::global::off_marked_thread(), "the core memory is set on the guest's thread");
    // SAFETY: the caller's.
    CORE.set(unsafe { Memory::new(addr, bytes) });
}

/// The guest's own memory: what `set` stored.
#[cfg(not(target_arch = "wasm32"))]
pub fn core() -> Memory {
    // SAFETY: before `set`, a native test's guest addresses the process from 0.
    *CORE.get(|| unsafe { Memory::new(0, usize::MAX) })
}

/// The guest's own memory: the module's, from address 0.
#[cfg(target_arch = "wasm32")]
pub const fn core() -> Memory {
    // SAFETY: a module's memory begins at 0 and is all its own.
    unsafe { Memory::new(0, usize::MAX) }
}

/// `Memory::at` over `core`, for a program reading its own data.
pub fn at<T>(off: usize) -> *mut T {
    core().at(off)
}

/// `Memory::off_of` over `core`.
#[track_caller]
pub fn off_of<T>(p: *const T) -> usize {
    core().off_of(p)
}

/// `Memory::off_of_slice` over `core`.
#[track_caller]
pub fn off_of_slice<T>(s: *const [T]) -> usize {
    core().off_of_slice(s)
}
