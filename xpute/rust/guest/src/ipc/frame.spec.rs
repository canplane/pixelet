// xpute-guest/ipc/frame.spec.rs
//
// GENERATED from spec/ipc/frame.json — do not edit.
//
// A message in a ring (ring.json): four words, a frame. The submitter's tag comes back on
// the reply; the command shares its word with the flags; a reply carries a value or a
// negative errno; and a packet, where there is one, lies in the ring's payload slot.
//
// Reserved for later, so taking them moves no word: the tag's high bits (a sender id for
// routing), flag bits 2 and up, and the result word on a submission.

// Casing here is the spec's and not Rust's: a name is spelled as the JSON
// spells it, so that one record is indexed by one name on both sides. To
// case them the way Rust would is to rename the protocol.
//
// `dead_code` because a table is generated whole: an entry earns its
// place in the JSON, not by having a caller in this crate.
#![allow(non_upper_case_globals, non_snake_case, dead_code)]

/// The submitter's, echoed on the reply; a signal's subject.
pub const TAG: u32 = 0;

/// The command in the low FLAG_SHIFT bits, the flags above them.
pub const CMD: u32 = 1;

/// On a reply, a value or a negative errno; else 0.
pub const RESULT: u32 = 2;

/// Where its XTP packet is, as the transport names it; 0 for none.
pub const PACKET: u32 = 3;

/// Words a frame takes.
pub const WORDS: u32 = 4;

/// Where the flags start in the CMD word.
pub const FLAG_SHIFT: u32 = 16;

/// A flag: a reply, as against a signal. Set by the sender, never by a handler.
pub const RES: u32 = 1;

/// A flag: always reply; without it, only a failure is replied to.
pub const ACKREQ: u32 = 2;
