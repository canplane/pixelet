// xpute-guest/program.spec.rs
//
// GENERATED from spec/program.json
//
// A program's defaults (xpute-guest program.rs `program!`, cpp/xpute/guest/program.hpp
// `XPUTE_PROGRAM`): the port its host speaks as, what its range holds and the stack it
// is linked with, each where the program says nothing else; and the files it may have
// open. The range's offsets follow from the counts, as each guest end computes them.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The port a program's host speaks as unless it says otherwise, and the one a handler's
/// events are raised for: any but the host's, 0 (ipc/frame.json HOST).
pub const PORT: u32 = 1;

/// Each ring's slots: how many commands can wait, and replies and events be left,
/// between two turns.
pub const CAP: u32 = 64;

/// A slot's bytes: the most a command's packet, or an event's, may be.
pub const SLOT_BYTES: u32 = 1024;

/// The hostcalls' stream, in words: what one turn may ask the host to run after it.
pub const SYS_WORDS: u32 = 4096;

/// The events one command may raise beside its reply.
pub const MOST_EVENTS: u32 = 4;

/// The stack a module is linked with (`-z stack-size`), placed first.
pub const STACK_BYTES: u32 = 1048576;

/// The slots of the table its open files are held in, each a read awaited: one fewer may
/// be open at once, and a read past them is refused with EMFILE.
pub const MOST_FILES: u32 = 64;
