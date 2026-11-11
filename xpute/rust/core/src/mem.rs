// xpute-core/mem.rs

//! Every address a core names is an offset from a memory's base, never a
//! pointer, so that it means the same on every runtime and fits in a `u32`.
//! A part laid over a region of memory is given the `Memory` it lies in. A
//! program reaches its own data through `core`, whose base is 0 on wasm32
//! and in a native test.

pub mod heap;
pub mod range;
pub mod section;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

#[cfg(not(target_arch = "wasm32"))]
use crate::global::Global;

#[derive(Clone, Copy, Debug)]
pub struct Memory {
    base: usize,
    bytes: usize,
}

impl Memory {
    /// # Safety
    /// `base..base + bytes` must be memory both ends may read and write, and
    /// must stay where it is for as long as the `Memory`, or a part laid over
    /// it, lives.
    pub const unsafe fn new(base: usize, bytes: usize) -> Memory {
        Memory { base, bytes }
    }

    pub fn base(self) -> usize {
        self.base
    }

    /// The pointer is made from the address, with the provenance `off_of`
    /// exposed, and never by `ptr::add` on the wasm base: adding to the null
    /// pointer at address 0 is undefined behaviour.
    pub fn at<T>(self, off: usize) -> *mut T {
        core::ptr::with_exposed_provenance_mut(self.base + off)
    }

    /// Reads the word by copy, since an offset from the other end carries no
    /// alignment a compiler may assume. A word past the memory's end is a bug.
    #[track_caller]
    pub fn word(self, off: usize) -> u32 {
        self.within(off, 4);
        // SAFETY: inside the memory, which `new`'s caller keeps valid.
        unsafe { self.at::<u32>(off).read_unaligned() }
    }

    #[track_caller]
    pub fn set_word(self, off: usize, w: u32) {
        self.within(off, 4);
        // SAFETY: as `word`.
        unsafe { self.at::<u32>(off).write_unaligned(w) }
    }

    /// Whether the `len` bytes at `off` lie inside the memory.
    pub fn holds(self, off: usize, len: usize) -> bool {
        off.checked_add(len).is_some_and(|end| end <= self.bytes)
    }

    #[track_caller]
    fn within(self, off: usize, len: usize) {
        self.holds(off, len).then_some(()).or_bug(Errno::EFAULT);
    }

    /// A pointer outside the memory is a bug. On a native build, statics and
    /// read-only data lie outside the memory.
    #[track_caller]
    pub fn off_of<T>(self, p: *const T) -> usize {
        p.expose_provenance()
            .checked_sub(self.base)
            .filter(|&off| off < self.bytes)
            .or_bug(Errno::EFAULT)
    }

    /// An empty slice's pointer is only its alignment and may lie outside the
    /// region, so an empty slice names 0. A slice the runtime writes into
    /// must be passed as `&mut`.
    #[track_caller]
    pub fn off_of_slice<T>(self, s: *const [T]) -> usize {
        if s.is_empty() {
            0
        } else {
            self.off_of(s.cast::<T>())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
static CORE: Global<Memory> = Global::new();

/// Sets the core memory. Call it once, on the core's thread (it is a
/// `Global`), before anything is allocated.
///
/// # Safety
/// As for `Memory::new`, for the core's whole life.
#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn set(addr: usize, bytes: usize) {
    assert!(
        !crate::global::off_marked_thread(),
        "the core memory is set on the core's thread"
    );
    // SAFETY: the caller's.
    CORE.set(unsafe { Memory::new(addr, bytes) });
}

#[cfg(not(target_arch = "wasm32"))]
pub fn core() -> Memory {
    // SAFETY: before `set`, a native test's core addresses the process from 0.
    *CORE.get(|| unsafe { Memory::new(0, usize::MAX) })
}

#[cfg(target_arch = "wasm32")]
pub const fn core() -> Memory {
    // SAFETY: a module's memory begins at 0 and is all its own.
    unsafe { Memory::new(0, usize::MAX) }
}

pub fn at<T>(off: usize) -> *mut T {
    core().at(off)
}

#[track_caller]
pub fn off_of<T>(p: *const T) -> usize {
    core().off_of(p)
}

#[track_caller]
pub fn off_of_slice<T>(s: *const [T]) -> usize {
    core().off_of_slice(s)
}
