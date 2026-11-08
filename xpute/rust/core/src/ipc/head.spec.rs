// xpute-core/ipc/head.spec.rs
//
// GENERATED from spec/ipc/head.json
//
// The header at the front of a core's memory. The front is what the runtime lays out before
// the core's first turn: this header, the two rings and their slots, and the GPU stream
// (ARCHITECTURE.md). The header lies on the page past the core's stack. The runtime writes it
// as io_uring's setup lays its rings, and lays each ring's own header too (ring.json), with
// its positions at 0. Both ends compute every number in the header from the same machine, so
// the header records what the runtime laid; it does not tell the core where to look. On its
// first turn the core holds each word to its machine and stops with ENOEXEC where one
// differs (xpute-core machine.rs).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The bytes "XPUT" read as a little-endian word. It tells a laid header from memory that
/// nothing has written.
pub const MAGIC: u32 = 1414877272;

/// The index of each header word, counted from the header's start.
pub mod word {
    pub const MAGIC: u32 = 0;
    /// How many frames the submission ring holds.
    pub const SQ_CAP: u32 = 1;
    /// How many frames the completion ring holds.
    pub const CQ_CAP: u32 = 2;
    /// The size of one ring slot in bytes, the same for both rings.
    pub const SLOT_BYTES: u32 = 3;
    /// The offset in the memory where the submission ring begins.
    pub const SQ_AT: u32 = 4;
    /// The offset in the memory where the completion ring begins.
    pub const CQ_AT: u32 = 5;
    /// The offset of the GPU stream, which the runtime runs after each turn; 0 when the machine
    /// asks for no GPU.
    pub const GPU_AT: u32 = 6;
    pub const KEYS: &[&str] = &[
        "MAGIC",
        "SQ_CAP",
        "CQ_CAP",
        "SLOT_BYTES",
        "SQ_AT",
        "CQ_AT",
        "GPU_AT",
    ];
}

/// The header's length in words.
pub const WORDS: u32 = 7;
