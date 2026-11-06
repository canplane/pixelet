// xpute-core/ipc/head.spec.rs
//
// GENERATED from spec/ipc/head.json
//
// The front's header: the words a runtime writes on the page past a core's stack before its
// first turn, as io_uring's setup lays its rings, and that the core's boot holds to its
// machine, stopping with ENOEXEC where one differs (xpute-core machine.rs). Both ends
// compute every number in it from the same machine, so it names what the runtime laid, not
// where to look: the rings' sizes and places, and the GPU stream's. The runtime lays each
// ring's own header too (ring.json), its positions at 0.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// "XPUT" as four bytes read little-endian: what tells a header from memory nothing wrote.
pub const MAGIC: u32 = 1414877272;

/// Raised when a word comes to mean something else. 1 was the toc's, a directory of keys.
pub const VERSION: u32 = 2;

/// Where each of the header's words is, from its start.
pub mod word {
    /// Where MAGIC is.
    pub const MAGIC: u32 = 0;
    /// Where VERSION is.
    pub const VERSION: u32 = 1;
    /// Frames the submission ring holds.
    pub const SQ_CAP: u32 = 2;
    /// Frames the completion ring holds.
    pub const CQ_CAP: u32 = 3;
    /// A ring slot's payload, the same for both rings.
    pub const SLOT_BYTES: u32 = 4;
    /// Where the submission ring's words begin.
    pub const SQ_AT: u32 = 5;
    /// Where the completion ring's words begin.
    pub const CQ_AT: u32 = 6;
    /// Where the GPU stream is, which the runtime runs after each turn; 0 where the machine asks for no GPU.
    pub const GPU_AT: u32 = 7;
    pub const KEYS: &[&str] = &[
        "MAGIC",
        "VERSION",
        "SQ_CAP",
        "CQ_CAP",
        "SLOT_BYTES",
        "SQ_AT",
        "CQ_AT",
        "GPU_AT",
    ];
}

/// Words the header takes.
pub const WORDS: u32 = 8;
