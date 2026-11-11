// xpute-core/io/head.spec.rs
//
// GENERATED from spec/io/head.json
//
// The header at the front of a core's memory. The front is what the runtime lays out before
// the core's first turn: this header and the GPU stream (ARCHITECTURE.md). The header lies on
// the page past the core's stack. Both ends compute every number in the header from the same
// manifest, so the header records what the runtime laid; it does not tell the core where to
// look. On its first turn the core holds each word to its manifest and stops with ENOEXEC
// where one differs (xpute-core manifest.rs).

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
    /// The most bytes a frame's packet takes, either way.
    pub const PKT_BYTES: u32 = 1;
    /// The offset of the stream, which is run after each turn; 0 when the manifest
    /// asks for none.
    pub const STREAM_AT: u32 = 2;
    pub const KEYS: &[&str] = &["MAGIC", "PKT_BYTES", "STREAM_AT"];
}

/// The header's length in words.
pub const WORDS: u32 = 3;
