// xpute_core/ipc/frame.rs

//! One command across a thread boundary, laid out in the buffer that carries
//! it.
//!
//! The buffer is the message: a request is written into it, the buffer is
//! transferred, the handler writes its reply into the same buffer, and it is
//! transferred back. Ownership moves with it — the sender's handle is
//! detached while it is away — so the two sides never share bytes and never
//! race, and nothing is copied except what the command itself chooses to
//! copy in and out.
//!
//! Layout, platform-endian (every host this runs on is little-endian, and a
//! WASM reader is): a fixed header of eight u32 words, then the command's
//! args, then its body at an 8-aligned offset. A body too large for the
//! buffer travels in a second buffer, the spill, and the header says so.
//!
//!   0  tx_id        worker id in the high bits, a sequence in the low
//!   1  cmd | flags  cmd in the low 16 bits, FrameFlag in the high 16
//!   2  errno        0, or a negative Errno
//!   3  args_bytes   args start at FRAME_HEADER_BYTES
//!   4  body_offset  8-aligned, or BODY_IN_SPILL
//!   5  body_bytes
//!   6  reserved
//!   7  reserved
//!
//! The shape is what April's frame had after its routing, selectors and
//! string commands were taken off (issue/20260913-boundaries-commands-and-
//! what-comes-back-from-april.md): a worker id in tx_id so a reply routes
//! without a lookup, `ACKREQ` as behaviour, an errno reply.

use crate::kit::array::{ArrayBuffer, TypedArray};

pub const FRAME_HEADER_WORDS: u32 = 8;
pub const FRAME_HEADER_BYTES: u32 = FRAME_HEADER_WORDS * 4;

/// body_offset when the body is in the spill buffer rather than the frame.
pub const BODY_IN_SPILL: u32 = 0xffffffff;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FrameFlag {
    /// A reply. Set by the runtime that sends it, never by a handler.
    RES = 1,
    /// Exactly one reply is sent; without it, none is.
    ACKREQ = 2,
}

pub const WORKER_ID_SHIFT: u32 = 24;
const SEQ_MASK: u32 = (1 << WORKER_ID_SHIFT) - 1;

pub fn make_tx_id(worker_id: u32, seq: u32) -> u32 {
    (worker_id << WORKER_ID_SHIFT) | (seq & SEQ_MASK)
}

pub fn tx_worker_id(tx_id: u32) -> u32 {
    tx_id >> WORKER_ID_SHIFT
}

pub fn tx_seq(tx_id: u32) -> u32 {
    tx_id & SEQ_MASK
}

/// In u64, as the TypeScript's is in doubles: a size near 2^32 must not wrap
/// to a small offset that every "does it fit" check then passes.
pub fn align8(n: u64) -> u64 {
    (n + 7) & !7
}

/// The message a link carries: the frame, and the spill when there is one.
pub struct FrameMessage {
    pub frame: ArrayBuffer,
    pub spill: Option<ArrayBuffer>,
}

/// A header read or written in place. One view per arrival: a transferred
/// buffer detaches every view the other side held, so there is nothing to
/// keep between messages.
pub struct Frame {
    pub buf: ArrayBuffer,
    words: TypedArray<u32>,
}

impl Frame {
    pub fn new(buf: ArrayBuffer) -> Frame {
        if (buf.byte_length() as u32) < FRAME_HEADER_BYTES {
            panic!("ipc: frame of {} bytes is shorter than its header", buf.byte_length());
        }
        let words = TypedArray::over(buf.clone(), 0, FRAME_HEADER_WORDS as usize);
        Frame { buf, words }
    }

    pub fn tx_id(&self) -> u32 {
        self.words[0]
    }
    pub fn set_tx_id(&self, v: u32) {
        self.words.put(0, v);
    }

    pub fn cmd(&self) -> u32 {
        self.words[1] & 0xffff
    }
    pub fn set_cmd(&self, v: u32) {
        self.words.put(1, (self.words[1] & 0xffff0000) | (v & 0xffff));
    }

    pub fn flags(&self) -> u32 {
        self.words[1] >> 16
    }
    pub fn set_flags(&self, v: u32) {
        self.words.put(1, ((v & 0xffff) << 16) | (self.words[1] & 0xffff));
    }

    /// 0, or a negative Errno, as the word reads signed.
    pub fn errno(&self) -> i32 {
        self.words[2] as i32
    }
    pub fn set_errno(&self, v: i32) {
        self.words.put(2, v as u32);
    }

    pub fn args_bytes(&self) -> u32 {
        self.words[3]
    }
    pub fn body_offset(&self) -> u32 {
        self.words[4]
    }
    pub fn body_bytes(&self) -> u32 {
        self.words[5]
    }

    /// Starts a frame: header cleared, command and flags set, no args, no body.
    pub fn begin(&self, tx_id: u32, cmd: u32, flags: u32) {
        self.words.fill(0);
        self.set_tx_id(tx_id);
        self.set_cmd(cmd);
        self.set_flags(flags);
    }

    /// Where the args live, for `args_bytes` bytes.
    pub fn args_offset(&self) -> u32 {
        FRAME_HEADER_BYTES
    }

    /// Declares the args and the body that follow them, and says where the
    /// body goes: in the frame when it fits, otherwise in a spill of its own.
    /// Returns the offset to write the body at, or BODY_IN_SPILL.
    pub fn lay_out(&self, args_bytes: u32, body_bytes: u32) -> u32 {
        let body_offset = align8(FRAME_HEADER_BYTES as u64 + args_bytes as u64);
        if body_offset > self.buf.byte_length() as u64 {
            panic!("ipc: args of {args_bytes} bytes do not fit a {}-byte frame", self.buf.byte_length());
        }
        let in_frame = body_offset + body_bytes as u64 <= self.buf.byte_length() as u64;
        self.words.put(3, args_bytes);
        self.words.put(4, if in_frame { body_offset as u32 } else { BODY_IN_SPILL });
        self.words.put(5, body_bytes);
        self.words[4]
    }
}

#[cfg(test)]
#[path = "frame.test.rs"]
mod test;
