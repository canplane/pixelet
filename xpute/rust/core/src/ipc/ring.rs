// xpute-core/ipc/ring.rs

//! A ring of frames, laid out by spec/ipc/ring.json. A push fails only when
//! the ring is full (EAGAIN) or when the packet exceeds a slot (EMSGSIZE);
//! whether it fails never depends on what was sent before.
//!
//! A ring's two sides may take turns on one thread, as a page's runtime and its
//! module do, or run at once on two, as a native runtime and a core on its own
//! thread do. The same rules, io_uring's, hold either way:
//!
//! - R1. Each position word has one writer: TAIL is the producer's and HEAD
//!   the consumer's.
//! - R2. A position word is stored with release and loaded with acquire. The
//!   producer writes a frame and its slot, then stores TAIL past them; the
//!   consumer loads TAIL before it reads them, and stores HEAD past a frame
//!   once it is done with the frame and its slot; the producer loads HEAD
//!   before it writes into that frame or slot again.
//! - R3. Memory a frame names by its address is the receiver's from the
//!   moment the frame is published until the receiver stores HEAD past it.
//!   The sender writes it again only once it has loaded a HEAD past that
//!   frame (`is_consumed`).
//!
//! The frames' and slots' own words are plain memory, ordered by the
//! position words alone. Where a target has no threads, as WebAssembly
//! without its atomics feature, the atomic accesses are its plain loads and
//! stores.

#[path = "ring.spec.rs"]
mod spec;

use core::sync::atomic::{AtomicU32, Ordering};

use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::spec::{HDR_SZ, WORD_SZ};

use super::frame::{self, cmd_of, cmd_word, flags_of, Frame};
use crate::mem::Memory;
use spec::{CAP, HEAD, HEADER_WORDS, SLOT_BASE, SLOT_BYTES, TAIL};

pub const fn ring_bytes(cap: u32) -> u32 {
    (HEADER_WORDS + cap * frame::WORDS) * 4
}

pub const fn slots_bytes(cap: u32, slot_bytes: u32) -> u32 {
    cap * slot_bytes
}

/// Payloads are padded to XTP's word (`WORD_SZ`).
fn padded(bytes: usize) -> usize {
    bytes.next_multiple_of(WORD_SZ as usize)
}

#[derive(Clone)]
pub struct Ring {
    mem: Memory,
    at: usize,
    pub cap: u32,
    pub slot_bytes: u32,
    mask: u32,
    slot_base: u32,
}

impl Ring {
    /// # Safety
    /// The ring's words at `at` and its slots in `mem` must stay valid, and
    /// never move, for as long as the ring is used.
    pub unsafe fn new(mem: Memory, at: usize) -> Ring {
        if !mem.at::<u32>(at).is_aligned() {
            xpute_kit::bug!(EINVAL, at);
        }
        let header = |word: u32| mem.word(at + 4 * word as usize);
        let cap = header(CAP);
        if !cap.is_power_of_two() {
            xpute_kit::bug!(EINVAL, cap, at);
        }
        Ring {
            mem,
            at,
            cap,
            slot_bytes: header(SLOT_BYTES),
            mask: cap - 1,
            slot_base: header(SLOT_BASE),
        }
    }

    /// `at` is where the ring's words begin in its memory.
    pub fn at(&self) -> usize {
        self.at
    }

    /// A position word (HEAD, TAIL), only ever touched as an atomic (R2).
    fn pos(&self, word: u32) -> &AtomicU32 {
        // SAFETY: a word of the ring `new` was given, aligned as `new` checked,
        // which neither side reads or writes but atomically.
        unsafe { AtomicU32::from_ptr(self.mem.at::<u32>(self.at + 4 * word as usize)) }
    }

    /// `frame_word` reads the ring's word at index `word` from the ring's
    /// start. A frame's words are the producer's until it publishes the frame,
    /// and the consumer's after.
    fn frame_word(&self, word: usize) -> u32 {
        // SAFETY: inside the ring `new` was given, aligned as its start is.
        unsafe { self.mem.at::<u32>(self.at + 4 * word).read() }
    }

    fn set_frame_word(&self, word: usize, w: u32) {
        // SAFETY: as `frame_word`.
        unsafe { self.mem.at::<u32>(self.at + 4 * word).write(w) }
    }

    /// The one slot at `start` from the slots' base, and no more, since the
    /// other side may be reading or writing another slot meanwhile.
    #[allow(clippy::mut_from_ref)]
    fn slot(&self, start: usize) -> &mut [u8] {
        // SAFETY: a slot of the memory `new` was given, which R2 hands to one
        // side at a time.
        unsafe {
            core::slice::from_raw_parts_mut(
                self.mem.at::<u8>(self.slot_base as usize + start),
                self.slot_bytes as usize,
            )
        }
    }

    /// # Safety
    /// As for `new`.
    pub unsafe fn init(mem: Memory, at: usize, cap: u32, slot_base: u32, slot_bytes: u32) -> Ring {
        if !cap.is_power_of_two() {
            xpute_kit::bug!(EINVAL, cap);
        }
        if !slot_bytes.is_power_of_two() || !slot_bytes.is_multiple_of(WORD_SZ) {
            xpute_kit::bug!(EINVAL, slot_bytes);
        }
        // SAFETY: the caller's memory, as `new` requires.
        unsafe {
            core::slice::from_raw_parts_mut(mem.at::<u8>(at), ring_bytes(cap) as usize).fill(0);
            let set_header = |word: u32, w: u32| mem.set_word(at + 4 * word as usize, w);
            set_header(CAP, cap);
            set_header(SLOT_BYTES, slot_bytes);
            set_header(SLOT_BASE, slot_base);
            Ring::new(mem, at)
        }
    }

    /// Finds the first fixed word of the ring header at `at` that differs from
    /// what `cap`, `slot_base` and `slot_bytes` say, and answers where it is in
    /// the memory and what it holds, or none when all agree. The position words
    /// are not checked, since a runtime may have pushed before the core looks.
    pub fn header_differs(
        mem: Memory,
        at: usize,
        cap: u32,
        slot_base: u32,
        slot_bytes: u32,
    ) -> Option<(u32, u32)> {
        [(CAP, cap), (SLOT_BYTES, slot_bytes), (SLOT_BASE, slot_base)]
            .into_iter()
            .find_map(|(word, w)| {
                let off = at + 4 * word as usize;
                let found = mem.word(off);
                (found != w).then_some((off as u32, found))
            })
    }

    pub fn head(&self) -> u32 {
        self.pos(HEAD).load(Ordering::Acquire)
    }

    pub fn tail(&self) -> u32 {
        self.pos(TAIL).load(Ordering::Acquire)
    }

    /// Whether the consumer has stored its head past the frame published at
    /// `pos`, so that what the frame named is the producer's again (R3). Only
    /// the producer asks this.
    pub fn is_consumed(&self, pos: u32) -> bool {
        let tail = self.tail();
        tail.wrapping_sub(self.head()) < tail.wrapping_sub(pos)
    }

    pub fn len(&self) -> u32 {
        self.tail().wrapping_sub(self.head())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The word, counted from the ring's start, where the frame at `pos`
    /// begins.
    pub fn entry_at(&self, pos: u32) -> u32 {
        HEADER_WORDS + (pos & self.mask) * frame::WORDS
    }

    fn slot_at(&self, pos: u32) -> usize {
        ((pos & self.mask) * self.slot_bytes) as usize
    }

    pub fn push(&self, tag: u32, cmd: u32, flags: u32, result: i32, pkt: Option<&[u8]>) -> Errno {
        if self.len() >= self.cap {
            return Errno::EAGAIN;
        }
        let pkt_at = match pkt {
            None => 0,
            Some(pkt) => {
                if padded(pkt.len()) > self.slot_bytes as usize {
                    return Errno::EMSGSIZE;
                }
                let start = self.slot_at(self.tail());
                self.slot(start)[..pkt.len()].copy_from_slice(pkt);
                self.pad(start, pkt.len());
                self.slot_base + start as u32
            }
        };
        self.publish(tag, cmd, flags, result, pkt_at);
        Errno::OK
    }

    /// `write` fills the slot's payload and answers the packet's length, or
    /// none when it did not fit.
    pub fn push_in_place(
        &self,
        tag: u32,
        cmd: u32,
        flags: u32,
        result: i32,
        write: impl FnOnce(&mut [u8]) -> Option<u32>,
    ) -> Errno {
        match self.lay(tag, cmd, flags, result, write) {
            Ok(_) => {
                self.advance_tail();
                Errno::OK
            }
            Err(e) => e,
        }
    }

    /// Lays `push_in_place`'s frame at the tail without publishing it, and
    /// answers where its words begin in the memory. The next push overwrites
    /// it.
    pub fn stage_in_place(
        &self,
        tag: u32,
        cmd: u32,
        flags: u32,
        write: impl FnOnce(&mut [u8]) -> Option<u32>,
    ) -> Result<usize, Errno> {
        let at = self.lay(tag, cmd, flags, 0, write)?;
        Ok(self.at + 4 * at)
    }

    /// Lays the frame at the tail, with its packet written by `write`, and
    /// answers the word where it begins, counted from the ring's start.
    fn lay(
        &self,
        tag: u32,
        cmd: u32,
        flags: u32,
        result: i32,
        write: impl FnOnce(&mut [u8]) -> Option<u32>,
    ) -> Result<usize, Errno> {
        if self.len() >= self.cap {
            return Err(Errno::EAGAIN);
        }
        let start = self.slot_at(self.tail());
        let Some(n) = write(self.slot(start)).filter(|&n| n <= self.slot_bytes) else {
            return Err(Errno::EMSGSIZE);
        };
        self.pad(start, n as usize);
        Ok(self.entry(tag, cmd, flags, result, self.slot_base + start as u32))
    }

    fn pad(&self, start: usize, len: usize) {
        self.slot(start)[len..padded(len)].fill(0);
    }

    fn publish(&self, tag: u32, cmd: u32, flags: u32, result: i32, pkt_at: u32) {
        self.entry(tag, cmd, flags, result, pkt_at);
        self.advance_tail();
    }

    /// Writes the frame's words at the tail's entry, unpublished, and answers
    /// where they begin.
    fn entry(&self, tag: u32, cmd: u32, flags: u32, result: i32, pkt_at: u32) -> usize {
        let at = self.entry_at(self.tail()) as usize;
        self.set_frame_word(at + frame::TAG as usize, tag);
        self.set_frame_word(at + frame::CMD as usize, cmd_word(cmd, flags));
        self.set_frame_word(at + frame::RESULT as usize, result as u32);
        self.set_frame_word(at + frame::PKT as usize, pkt_at);
        at
    }

    /// Publishes the frame at the tail, whose words and slot are already
    /// written (R2).
    fn advance_tail(&self) {
        self.pos(TAIL)
            .store(self.tail().wrapping_add(1), Ordering::Release);
    }

    /// `peek` answers the frame at the head, or none when the ring is empty. A
    /// reader reads it, then calls `advance`.
    pub fn peek(&self) -> Option<Frame> {
        if self.is_empty() {
            return None;
        }
        let at = self.entry_at(self.head()) as usize;
        let word = self.frame_word(at + frame::CMD as usize);
        Some(Frame {
            tag: self.frame_word(at + frame::TAG as usize),
            cmd: cmd_of(word),
            flags: flags_of(word),
            result: self.frame_word(at + frame::RESULT as usize) as i32,
            pkt_at: self.frame_word(at + frame::PKT as usize),
        })
    }

    /// The frame's packet, valid until `advance`. It is `'static` because the
    /// memory's lifetime is `new`'s contract, and a reader holds the packet
    /// while it acts on the ring. A frame that names bytes outside a slot
    /// answers EBADMSG.
    pub fn pkt(&self, f: &Frame) -> Result<Option<&'static [u8]>, MarshalError> {
        if f.pkt_at == 0 {
            return Ok(None);
        }
        let bad = || MarshalError::new(Errno::EBADMSG);
        let off = f.pkt_at.checked_sub(self.slot_base).ok_or_else(bad)? as usize;
        if !off.is_multiple_of(self.slot_bytes as usize)
            || off + HDR_SZ as usize > slots_bytes(self.cap, self.slot_bytes) as usize
        {
            return Err(bad());
        }
        // Computed in u64, so that a length word near 2^32 reads as too long
        // instead of wrapping on wasm32.
        let payload = self
            .mem
            .word(self.slot_base as usize + off + WORD_SZ as usize);
        let bytes = (u64::from(HDR_SZ) + u64::from(payload)).next_multiple_of(u64::from(WORD_SZ));
        if bytes > u64::from(self.slot_bytes) {
            return Err(bad());
        }
        let bytes = bytes as usize;
        // SAFETY: the memory `new` was given, as its contract requires.
        Ok(Some(unsafe {
            core::slice::from_raw_parts(self.mem.at::<u8>(self.slot_base as usize + off), bytes)
        }))
    }

    /// Lets go of the frame at the head and its slot (R2).
    pub fn advance(&self) {
        if self.is_empty() {
            xpute_kit::bug!(ENOTRECOVERABLE);
        }
        self.pos(HEAD)
            .store(self.head().wrapping_add(1), Ordering::Release);
    }
}

#[cfg(test)]
#[path = "ring.test.rs"]
mod test;
