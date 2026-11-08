// xpute-core/ipc/ring.spec.rs
//
// GENERATED from spec/ipc/ring.json
//
// A ring of frames (frame.json) with one producer and one consumer, in memory both sides
// can see. Each side names the ring by its offset in that memory, so the name means the
// same to both. The ring begins with a header of HEADER_WORDS 32-bit words, and every other
// name here is the index of one header word. The frames follow the header; their payloads
// lie apart in slots, as in virtio and AF_XDP. Position n owns frame n modulo the capacity
// and the slot of the same index, so a slot is free exactly when its frame is. Positions
// only count up, and each side writes only its own position word, so the ring needs no lock.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// How many frames the ring holds, a power of two.
pub const CAP: u32 = 0;

/// The consumer's position: the next frame it reads.
pub const HEAD: u32 = 1;

/// The producer's position: where the next frame goes.
pub const TAIL: u32 = 2;

/// The size of one payload slot in bytes, a power of two and a multiple of 8.
pub const SLOT_BYTES: u32 = 3;

/// The offset in the memory where the slots start.
pub const SLOT_BASE: u32 = 4;

/// Reserved; it holds 0.
pub const RESERVED_0: u32 = 5;

/// Reserved; it holds 0.
pub const RESERVED_1: u32 = 6;

/// Reserved; it holds 0.
pub const RESERVED_2: u32 = 7;

/// The header's length in words; the first frame follows it.
pub const HEADER_WORDS: u32 = 8;
