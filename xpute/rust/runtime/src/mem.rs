// xpute-runtime/mem.rs

//! The core's memory as any runtime reads and writes it: by the offsets the
//! core names, each access reaching only its range, never one reference over
//! the whole, since the runtime's own blocks may lie in it (C++'s `xpute::Memory`).

use core::marker::PhantomData;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU32, Ordering};

pub struct Memory<'a> {
    base: NonNull<u8>,
    bytes: usize,
    of: PhantomData<&'a mut [u8]>,
}

impl<'a> Memory<'a> {
    /// A memory the caller holds whole, as a test does.
    pub fn of(bytes: &'a mut [u8]) -> Memory<'a> {
        Memory {
            bytes: bytes.len(),
            base: NonNull::from(bytes).cast(),
            of: PhantomData,
        }
    }

    /// # Safety
    /// The bytes are live while the value is used, and meanwhile nothing else
    /// holds a reference into a range the core names: the core is between turns.
    pub unsafe fn from_raw(base: NonNull<u8>, bytes: usize) -> Memory<'a> {
        Memory {
            base,
            bytes,
            of: PhantomData,
        }
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// The `n` bytes at `at`; past the memory's end is the core's bug.
    fn range(&self, at: usize, n: usize) -> *mut u8 {
        let end = at.checked_add(n).filter(|&end| end <= self.bytes);
        assert!(
            end.is_some(),
            "[mem] {n} bytes at {at} run past the memory's {}",
            self.bytes
        );
        // SAFETY: in bounds, as just checked.
        unsafe { self.base.as_ptr().add(at) }
    }

    /// The `n` bytes at `at`, borrowed while nothing is written.
    pub fn read(&self, at: usize, n: usize) -> &[u8] {
        // SAFETY: in the memory, which `from_raw`'s caller lets this read;
        // `&self` keeps every write out while the slice lives.
        unsafe { core::slice::from_raw_parts(self.range(at, n), n) }
    }

    pub fn write(&mut self, at: usize, from: &[u8]) {
        let to = self.range(at, from.len());
        // SAFETY: in the memory; `from` may lie in it too, so the copy may overlap.
        unsafe { core::ptr::copy(from.as_ptr(), to, from.len()) }
    }

    /// The little-endian word at `at`, aligned or not.
    pub fn word(&self, at: u32) -> u32 {
        let w = self.read(at as usize, 4);
        u32::from_le_bytes([w[0], w[1], w[2], w[3]])
    }

    pub fn set_word(&mut self, at: u32, v: u32) {
        self.write(at as usize, &v.to_le_bytes());
    }

    /// The word at `at` as an atomic, for a word that hands memory between
    /// runtime and core, which both sides touch only atomically: a ring's
    /// position, a stream's END. It must lie on a word.
    fn atomic(&self, at: u32) -> &AtomicU32 {
        let p = self.range(at as usize, 4).cast::<u32>();
        assert!(p.is_aligned(), "[mem] the word at {at} is not on a word");
        // SAFETY: in the memory and aligned, as just checked; a word both
        // sides touch only atomically.
        unsafe { AtomicU32::from_ptr(p) }
    }

    /// The handing word at `at`, loaded with acquire: what the other side
    /// wrote before it stored the word is seen.
    pub fn load_acquire(&self, at: u32) -> u32 {
        self.atomic(at).load(Ordering::Acquire)
    }

    /// `v` stored into the handing word at `at` with release: what this side
    /// wrote or read before is done before the other side sees `v`.
    pub fn store_release(&mut self, at: u32, v: u32) {
        self.atomic(at).store(v, Ordering::Release);
    }
}

#[cfg(test)]
#[path = "mem.test.rs"]
mod test;
