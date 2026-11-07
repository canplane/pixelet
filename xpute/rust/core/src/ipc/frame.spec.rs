// xpute-core/ipc/frame.spec.rs
//
// GENERATED from spec/ipc/frame.json
//
// A frame in a ring (ring.json): four words. The tag is the sender's id, which a reply
// keeps, so it goes back to the call that sent it, or an import's subject; the command
// shares its word with the flags; a reply carries a value or a negative errno; and a
// packet, where there is one, lies in the ring's payload slot.
//
// Commands share one number space, `major << 8 | minor`: majors 0x00 to 0x0F are
// xpute's own (abi/cmd.json, abi/fs.json), and every other is the program's to give,
// and so is what its commands mean. A runtime hands a frame to what answers its command's
// major.
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

/// The sender's id, echoed on the reply, or an import's subject.
pub const TAG: u32 = 0;

/// The command in the low FLAG_SHIFT bits, the flags above them.
pub const CMD: u32 = 1;

/// On a reply, a value or a negative errno; else 0.
pub const RESULT: u32 = 2;

/// Where its XTP packet is, as the transport names it; 0 for none.
pub const PKT: u32 = 3;

/// Words a frame takes.
pub const WORDS: u32 = 4;

/// The first major a program's commands take; those below it are xpute's own.
pub const PROGRAM_MAJOR: u32 = 16;

/// Where the flags start in the CMD word.
pub const FLAG_SHIFT: u32 = 16;

/// A flag: a reply's frame, as against an import's. Set by the sender, never by a handler.
pub const RES: u32 = 1;

/// A flag: always reply; without it, only a failure is replied to.
pub const ACKREQ: u32 = 2;
