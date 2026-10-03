// xpute-runtime/ipc/ring.spec.rs
//
// GENERATED from spec/xpute/ipc/ring.json — do not edit.
//
// A single-producer, single-consumer ring of frames (frame.json) in memory both sides
// see, named by its offset so it means the same to either. A header, then a frame a
// message; the payloads lie apart in slots, as in virtio and AF_XDP. Message n owns
// frame and slot n modulo the capacity, so a slot is free exactly when its frame is.
// Positions only grow, and each side writes only its own word, so no lock is needed.

// Casing here is the spec's and not Rust's: a name is spelled as the JSON
// spells it, so that one record is indexed by one name on both sides. To
// case them the way Rust would is to rename the protocol.
//
// `dead_code` because a table is generated whole: an entry earns its
// place in the JSON, not by having a caller in this crate.
#![allow(non_upper_case_globals, non_snake_case, dead_code)]

/// How many frames, a power of two.
pub const CAPACITY: u32 = 0;

/// The consumer's position: the next message it reads.
pub const HEAD: u32 = 1;

/// The producer's position: where the next message goes.
pub const TAIL: u32 = 2;

/// A payload slot's size, a power of two and a multiple of 8.
pub const SLOT_BYTES: u32 = 3;

/// Where the slots start, as an offset in the memory.
pub const SLOT_BASE: u32 = 4;

/// Kept at 0.
pub const RESERVED_0: u32 = 5;

/// Kept at 0.
pub const RESERVED_1: u32 = 6;

/// Kept at 0.
pub const RESERVED_2: u32 = 7;

/// Words before the first frame.
pub const HEADER_WORDS: u32 = 8;
