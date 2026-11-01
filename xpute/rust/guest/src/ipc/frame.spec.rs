// xpute-guest/ipc/frame.spec.rs
//
// GENERATED from spec/ipc/frame.json
//
// A frame in a ring (ring.json): four words. The tag names the port at the guest's
// far side in its high bits — who sent a frame the guest takes, who is to have one it
// posts — and below them an id, so a reply, which keeps its submission's tag, goes back
// to its sender; the command shares its word with the flags; a reply carries a value or
// a negative errno; and a packet, where there is one, lies in the ring's payload slot.
//
// The host is port 0, and a command it sends is xpute's own (abi/cmd.json); every
// other number is the program's to give, and so is what its commands mean. The host
// hands a frame to the port it names where one is attached, and drops it where none
// is.
//
// Reserved for later, so taking them moves no word: flag bits 2 and up, and the result
// word on a submission.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The far port from PORT_SHIFT up; below it the sender's id, echoed on the
/// reply, or an event's subject.
pub const TAG: u32 = 0;

/// The command in the low FLAG_SHIFT bits, the flags above them.
pub const CMD: u32 = 1;

/// On a reply, a value or a negative errno; else 0.
pub const RESULT: u32 = 2;

/// Where its XTP packet is, as the transport names it; 0 for none.
pub const PKT: u32 = 3;

/// Words a frame takes.
pub const WORDS: u32 = 4;

/// Where the far port starts in the TAG word: one of 256.
pub const PORT_SHIFT: u32 = 24;

/// The host's port, whose commands are xpute's own.
pub const HOST: u32 = 0;

/// Where the flags start in the CMD word.
pub const FLAG_SHIFT: u32 = 16;

/// A flag: a reply, as against an event. Set by the sender, never by a handler.
pub const RES: u32 = 1;

/// A flag: always reply; without it, only a failure is replied to.
pub const ACKREQ: u32 = 2;
