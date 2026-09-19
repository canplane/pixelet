// xpute-runtime/ipc/frame.rs

//! A frame: one message, four words.
//!
//! Every boundary carries the same four things — who asked, what is asked,
//! what came back, and the packet that goes with it. Those are the format; a
//! transport says only where the words sit and how word 3 names the packet.
//! The ring (ipc/ring.rs) lays a frame out as a slot's descriptor and names
//! the packet by its address in the memory both sides see.
//!
//!   0  tag          the submitter's, echoed on the reply; on a signal, its
//!                   subject
//!   1  cmd | flags  cmd (cmd.rs: major << 8 | minor) in the low 16 bits,
//!                   FrameFlag in the high 16
//!   2  result       on a reply: a value, or a negative Errno; else 0
//!   3  packet       where the message's XTP packet is, as the transport
//!                   names it, or 0 for none — its length is in its header
//!
//! ## Reserved
//!
//! Reserved here is not spare room: each is spoken for, and kept free so
//! that taking it moves no word already on the wire
//! (issue/20260913-boundaries-commands-and-what-comes-back-from-april.md).
//!
//! **`tag`'s high bits — routing.** One ring joins two sides, so the whole
//! word is the submitter's and a reply needs no address. Put a switch between
//! them — several workers, or a server — and a reply has to find its way back
//! without a lookup, which is what a sender id in the high bits buys. Take
//! them from the top; a sequence keeps the low bits it has.
//!
//! **`FrameFlag`'s bits 2 and up — what the low half of word 1 means.** The
//! low half is a command. A signal dispatched by vector differs from one only
//! in how its number is read, so it takes a flag bit rather than a second
//! word: set, the low half is a vector; clear, a command.
//!
//! **Word 2 while `RES` is clear.** A submission has nothing to report, so
//! the word is 0 and is free to mean something on the way out.

/// Words a frame takes.
pub const FRAME_WORDS: u32 = 4;

pub const FRAME_TAG: u32 = 0;
pub const FRAME_CMD: u32 = 1;
pub const FRAME_RESULT: u32 = 2;
pub const FRAME_PACKET: u32 = 3;

/// Where the flags start in word 1.
pub const FLAG_SHIFT: u32 = 16;
/// The low half of word 1.
pub const CMD_MASK: u32 = (1 << FLAG_SHIFT) - 1;

/// What a frame asks for, or reports. Bits 2 and up are reserved above.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FrameFlag {
    /// A reply. Set by the side that sends it, never by a handler: on a
    /// completion it tells a reply from a signal the other side raised.
    RES = 1,
    /// Exactly one reply is sent; without it, none is.
    ACKREQ = 2,
}

/// Word 1 from a command and its flags.
pub fn cmd_word(cmd: u32, flags: u32) -> u32 {
    (flags << FLAG_SHIFT) | (cmd & CMD_MASK)
}

/// The command word 1 carries.
pub fn cmd_of(word: u32) -> u32 {
    word & CMD_MASK
}

/// The flags word 1 carries.
pub fn flags_of(word: u32) -> u32 {
    word >> FLAG_SHIFT
}
