// xpute-host/mem.rs

//! The guest's core memory as a host reads and writes it: by the offsets the
//! guest names, each read or write reaching only the range it names, and
//! never one reference over the whole, as C++'s `xpute::Memory`
//! (cpp/xpute/guest/mem/memory.hpp) reads a word by offset. The host's own
//! blocks may lie in the same memory — a guest linked into the host's
//! process allocates for both from one heap (`native::Alloc`) — and a
//! `&mut [u8]` over all of it would claim them too.
//!
//! On every platform, not `native`'s alone: a host that holds a module's
//! memory as bytes, a test among them, reads it the same way, and so does
//! any host's GPU calls (xpute-gpu host).

use core::marker::PhantomData;
use core::ptr::NonNull;

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

    /// The memory at `base`, `bytes` long.
    ///
    /// # Safety
    /// The `bytes` bytes at `base` are readable and writable for as long as
    /// the value is used. While it is, the caller holds no reference into a
    /// range the guest names, and no other thread reads or writes one: the
    /// guest is between turns, and what else lies in the memory (the host's
    /// own blocks among it) is reached only through its owners.
    pub unsafe fn from_raw(base: NonNull<u8>, bytes: usize) -> Memory<'a> {
        Memory { base, bytes, of: PhantomData }
    }

    /// Its bytes.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// The `n` bytes at `at`; past the memory's end is the guest's bug.
    fn range(&self, at: usize, n: usize) -> *mut u8 {
        let end = at.checked_add(n).filter(|&end| end <= self.bytes);
        assert!(end.is_some(), "[mem] {n} bytes at {at} run past the memory's {}", self.bytes);
        // SAFETY: in bounds, as just checked.
        unsafe { self.base.as_ptr().add(at) }
    }

    /// The `n` bytes at `at`, borrowed while nothing is written.
    pub fn read(&self, at: usize, n: usize) -> &[u8] {
        // SAFETY: in the memory, which `from_raw`'s caller lets this read;
        // `&self` keeps every write out while the slice lives.
        unsafe { core::slice::from_raw_parts(self.range(at, n), n) }
    }

    /// `from` written at `at`.
    pub fn write(&mut self, at: usize, from: &[u8]) {
        let to = self.range(at, from.len());
        // SAFETY: in the memory; `from` may lie in it too (the host's own
        // blocks can), so the copy allows an overlap.
        unsafe { core::ptr::copy(from.as_ptr(), to, from.len()) }
    }

    /// The little-endian word at `at`, which need not be aligned: an offset
    /// from the other side carries no alignment.
    pub fn word(&self, at: u32) -> u32 {
        let w = self.read(at as usize, 4);
        u32::from_le_bytes([w[0], w[1], w[2], w[3]])
    }

    /// `v` written at `at` as a little-endian word.
    pub fn set_word(&mut self, at: u32, v: u32) {
        self.write(at as usize, &v.to_le_bytes());
    }
}

#[cfg(test)]
#[path = "mem.test.rs"]
mod test;
