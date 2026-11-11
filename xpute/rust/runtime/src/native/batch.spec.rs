// xpute-runtime/native/batch.spec.rs
//
// GENERATED from spec/io/batch.json
//
// A device's runtime hands its UI what the core sent as one batch (xpute-runtime
// native/serve.rs `take`, @xpute/runtime start.native.ts). Each frame in it is HDR_WORDS
// little-endian words, TAG to NAMED their indices, then its packet and then the memory it
// names, each padded to ALIGN bytes. Two commands no core sends are the runtime's own: STOPPED
// and COMMIT.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The frame's tag.
pub const TAG: u32 = 0;

/// The frame's command, without its flags.
pub const CMD: u32 = 1;

/// The frame's flags.
pub const FLAGS: u32 = 2;

/// The frame's result, a value or a negative errno.
pub const RESULT: u32 = 3;

/// The length in bytes of the frame's packet.
pub const PKT: u32 = 4;

/// The length in bytes of the memory the frame names.
pub const NAMED: u32 = 5;

/// The words before a frame's packet.
pub const HDR_WORDS: u32 = 6;

/// What a frame's packet and the memory it names are each padded to, in bytes.
pub const ALIGN: u32 = 8;

/// The command of the frame that says the core has stopped; it is the last the UI hears.
pub const STOPPED: u32 = 4294967295;

/// The command of the frame that asks the UI to commit a written file, tagged with its
/// descriptor: its packet is the file's path, and the memory it names is the whole file. The
/// UI answers with the close's completion, FS_CLOSE flagged RES.
pub const COMMIT: u32 = 4294967294;
