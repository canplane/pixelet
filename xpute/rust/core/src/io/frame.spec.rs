// xpute-core/io/frame.spec.rs
//
// GENERATED from spec/io/frame.json
//
// A frame is one method called across the link, four 32-bit words; TAG, CMD, RESULT and
// PKT are their indices, little-endian. The tag is the request, unique among the sender's
// pending ones, and a later answer carries it back. The command is the RES bit, the handle
// the callee issued above METHOD_BITS, and the method's place among its object's below
// them. An answer carries a value or a negative errno. The method's args, an XTP packet,
// lie right past the words, and PKT says how long they are (wit/link.wit).

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The request, which a later answer carries back.
pub const TAG: u32 = 0;

/// RES, the handle and the method.
pub const CMD: u32 = 1;

/// On an answer, a value or a negative errno; on a request, 0.
pub const RESULT: u32 = 2;

/// The length in bytes of the frame's packet, which lies right past its words; 0 when it has none.
pub const PKT: u32 = 3;

/// The frame's length in words.
pub const WORDS: u32 = 4;

/// The bit of the command that marks a later answer.
pub const RES: u32 = 2147483648;

/// The bits of the command below the handle, which hold the method.
pub const METHOD_BITS: u32 = 8;
