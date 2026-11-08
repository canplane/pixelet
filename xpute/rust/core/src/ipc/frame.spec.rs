// xpute-core/ipc/frame.spec.rs
//
// GENERATED from spec/ipc/frame.json
//
// A frame is one message in a ring (ring.json), four 32-bit words; TAG, CMD, RESULT and
// PKT are their indices. The tag is the sender's id, and a reply carries the same tag, so
// the reply reaches the call that sent it. On an import the tag says what the import is
// about instead. The command shares its word with the flags. A reply carries a value or a
// negative errno. A packet, when the frame has one, lies in the ring's payload slot.
//
// All commands share one number space, `major << 8 | minor`. Majors 0x00 to 0x0F are
// xpute's own (abi/sys.json, abi/gpu.json, abi/fs.json). Every other major is the
// program's to assign, and so is what its commands mean. A runtime hands each frame to
// whatever answers its command's major.
//
// Flag bits 2 and up, and the result word of a submission, are reserved, so that a later
// use of them moves no other word.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The sender's id, which the reply carries back; on an import, what the import is about.
pub const TAG: u32 = 0;

/// The command in the low FLAG_SHIFT bits, and the flags above them.
pub const CMD: u32 = 1;

/// On a reply, a value or a negative errno; on any other frame, 0.
pub const RESULT: u32 = 2;

/// Where the frame's XTP packet lies, as the transport names a place; 0 when it has none.
pub const PKT: u32 = 3;

/// The frame's length in words.
pub const WORDS: u32 = 4;

/// The first major a program's commands may take; the majors below it are xpute's own.
pub const PROGRAM_MAJOR: u32 = 16;

/// The bit at which the flags start in the CMD word.
pub const FLAG_SHIFT: u32 = 16;

/// A flag that marks the frame as a reply; a frame the core posts without it is an import.
/// The ring code that posts the reply sets it, and a command's handler never does.
pub const RES: u32 = 1;

/// A flag that asks for a reply in every case; without it, only a failure is replied to.
pub const ACKREQ: u32 = 2;
