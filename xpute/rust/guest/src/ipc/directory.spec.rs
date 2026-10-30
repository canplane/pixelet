// xpute-guest/ipc/directory.spec.rs
//
// GENERATED from spec/ipc/directory.json — do not edit.
//
// The directory: where a host finds what its guest laid out in the memory. The guest's
// first turn writes it, and the host reads it once, at the one offset it knows outside
// the memory (xpute-guest ipc/directory.rs, and the contract's rule 0 in
// ARCHITECTURE.md).
//
// Its words: MAGIC, VERSION, how many entries follow, and a word kept at 0; then that
// many entries, a key and a value each, no key twice. A reader looks up the keys it
// knows and passes over the rest, the way a process reads its ELF auxiliary vector, so
// a directory that grows an entry is still read by a host that predates it.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// "XPUT" as four bytes read little-endian: what tells a directory from memory nothing wrote.
pub const MAGIC: u32 = 1414877272;

/// Raised when a key comes to mean something else. A new key does not raise it.
pub const VERSION: u32 = 1;

/// Words before the first entry.
pub const HEAD: u32 = 4;

/// What an entry's value is. 3 to 5 are retired and given to nothing else: they were the
/// numbers of a host's scheduling policy, which the program now injects when the host
/// boots (the host's sched/policy.ts).
pub mod key {
    /// Where the submission ring's words begin.
    pub const SQ: u32 = 1;
    /// Where the completion ring's words begin.
    pub const CQ: u32 = 2;
    /// Where the system-call stream's words begin (ipc/sys.json).
    pub const SYS: u32 = 6;
    /// The bit every key a program numbers for itself carries: xpute gives none of
    /// them a meaning, and a host that is not the program's passes over them.
    pub const PROGRAM: u32 = 2147483648;
    pub const KEYS: &[&str] = &["SQ", "CQ", "SYS", "PROGRAM"];
}
