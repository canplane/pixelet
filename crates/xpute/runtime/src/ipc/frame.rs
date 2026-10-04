// xpute-runtime/ipc/frame.rs

//! A message, four words:
//!
//!   0  tag          the submitter's, echoed on the reply; a signal's subject
//!   1  cmd | flags  command in the low 16 bits, FrameFlag in the high 16
//!   2  result       on a reply, a value or a negative Errno; else 0
//!   3  packet       where its XTP packet is, as the transport names it; 0 for none
//!
//! Reserved for later, so taking them moves no word: `tag`'s high bits (a
//! sender id for routing), FrameFlag bits 2 and up, and word 2 on a submission.

pub const FRAME_WORDS: u32 = 4;

pub const FRAME_TAG: u32 = 0;
pub const FRAME_CMD: u32 = 1;
pub const FRAME_RESULT: u32 = 2;
pub const FRAME_PACKET: u32 = 3;

pub const FLAG_SHIFT: u32 = 16;
pub const CMD_MASK: u32 = (1 << FLAG_SHIFT) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FrameFlag {
    /// A reply, as against a signal. Set by the sender, never by a handler.
    RES = 1,
    /// Always reply; without it, only a failure is replied to.
    ACKREQ = 2,
}

pub fn cmd_word(cmd: u32, flags: u32) -> u32 {
    (flags << FLAG_SHIFT) | (cmd & CMD_MASK)
}

pub fn cmd_of(word: u32) -> u32 {
    word & CMD_MASK
}

pub fn flags_of(word: u32) -> u32 {
    word >> FLAG_SHIFT
}
