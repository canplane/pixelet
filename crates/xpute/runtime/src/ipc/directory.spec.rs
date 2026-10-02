// xpute-runtime/ipc/directory.spec.rs
//
// GENERATED from spec/xpute/ipc/directory.json — do not edit.
//
// The directory: where a host finds what its guest laid out in the memory. The guest's
// first turn writes it, and the host reads it once, at the one offset it knows outside
// the memory (xpute-runtime ipc/directory.rs, and the contract's rule 0 in
// ARCHITECTURE.xpute.md).
//
// Its words: MAGIC, VERSION, how many entries follow, and a word kept at 0; then that
// many entries, a key and a value each, no key twice. A reader looks up the keys it
// knows and passes over the rest, the way a process reads its ELF auxiliary vector, so
// a directory that grows an entry is still read by a host that predates it.

// Casing here is the spec's and not Rust's: a name is spelled as the JSON
// spells it, so that one record is indexed by one name on both sides. To
// case them the way Rust would is to rename the protocol.
//
// `dead_code` because a table is generated whole: an entry earns its
// place in the JSON, not by having a caller in this crate.
#![allow(non_upper_case_globals, non_snake_case, dead_code)]

/// "XPUT" as four bytes read little-endian: what tells a directory from memory nothing wrote.
pub const MAGIC: u32 = 1414877272;

/// Raised when a key comes to mean something else. A new key does not raise it.
pub const VERSION: u32 = 1;

/// Words before the first entry.
pub const HEAD: u32 = 4;

/// What a quantum policy's number is multiplied by to cross as a word: it crosses in thousandths.
pub const QUANTUM_SCALE: u32 = 1000;

/// What an entry's value is.
pub mod key {
    /// Where the submission ring's words begin.
    pub const SUBMISSION: u32 = 1;
    /// Where the completion ring's words begin.
    pub const COMPLETION: u32 = 2;
    /// The quantum policy's margin share (sched/quantum.rs), in QUANTUM_SCALE.
    pub const MARGIN_SHARE: u32 = 3;
    /// The quantum policy's batch frames, in QUANTUM_SCALE.
    pub const BATCH_FRAMES: u32 = 4;
    /// The quantum policy's settle time, in QUANTUM_SCALE of a millisecond.
    pub const SETTLE_MS: u32 = 5;
    /// Where the system-call stream's words begin (ipc/sys.json).
    pub const SYS: u32 = 6;
    /// The bit every key a program numbers for itself carries: xpute gives none of
    /// them a meaning, and a host that is not the program's passes over them.
    pub const PROGRAM: u32 = 2147483648;
    pub const KEYS: &[&str] = &["SUBMISSION", "COMPLETION", "MARGIN_SHARE", "BATCH_FRAMES", "SETTLE_MS", "SYS", "PROGRAM"];
}
