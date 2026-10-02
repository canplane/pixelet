// xpute-conformance/guest.spec.rs
//
// GENERATED from spec/xpute/conformance/guest.json — do not edit.
//
// The guest the contract's traces drive (spec/xpute/conformance/door.tsv): the
// few commands every language's conformance guest implements, and the numbers a trace
// is written against. It computes nothing, and asks for no turn when nothing waits; what
// it does inside is each guest's own, and what the host sees of it is what the traces
// hold it to.
//
// A trace names no production number. These are the fixture's, chosen so that a trace
// can make a turn's output outgrow its room without knowing how a guest reserves it.

// Casing here is the spec's and not Rust's: a name is spelled as the JSON
// spells it, so that one record is indexed by one name on both sides. To
// case them the way Rust would is to rename the protocol.
//
// `dead_code` because a table is generated whole: an entry earns its
// place in the JSON, not by having a caller in this crate.
#![allow(non_upper_case_globals, non_snake_case, dead_code)]

/// The room a guest's shared range asks for: the directory, both rings, a stream and its
/// own state, with room to spare. Where the range begins, and so where the directory is,
/// the guest's boot answers (the contract's rule 0); what else a memory holds — a module's
/// stack and data — is the build's, and no number here.
pub const MEMORY_BYTES: u32 = 262144;

/// Messages the submission ring holds.
pub const SUBMISSION_CAPACITY: u32 = 16;

/// Messages the completion ring holds: fewer than eight commands raising MOST_RAISE
/// signals each leave, so a trace that sends those makes a turn's output outgrow it.
pub const COMPLETION_CAPACITY: u32 = 128;

/// A ring slot's payload: a packet of one number fits.
pub const SLOT_BYTES: u32 = 256;

/// The stream's range, its END word first.
pub const STREAM_WORDS: u32 = 256;

/// The system calls' stream: OPEN and CLOSE are recorded there, and nothing runs them.
pub const SYS_WORDS: u32 = 64;

/// The guest's heap, in its own range: what its file table holds lies there, since the
/// memory a host hands a module never grows and a guest takes nothing it was not given.
pub const HEAP_BYTES: u32 = 65536;

/// Slots in the guest's handle table, slot 0 never handed out.
pub const HANDLES: u32 = 16;

/// The most signals one RAISE asks for; more is refused with EINVAL.
pub const MOST_RAISE: u32 = 32;

/// The guest's own entry in its directory, past xpute's PROGRAM bit.
pub mod key {
    /// Where the stream is, run by the host after every turn.
    pub const STREAM: u32 = 2147483649;
    pub const KEYS: &[&str] = &["STREAM"];
}

/// The commands, `major << 8 | minor`, on the majors a program numbers its own from
/// (xpute's PROGRAM_MAJOR and up), so none is one of xpute's — SYS_OPENED among them. A
/// packet, where one is taken, is a branch whose first child is a number.
pub mod cmd {
    /// Succeeds; replies 0.
    pub const OK: u32 = 4097;
    /// Fails with EINVAL.
    pub const FAIL: u32 = 4098;
    /// Raises the packet's number of SIGNALs, tagged 0 up, then replies with that number.
    pub const RAISE: u32 = 4099;
    /// Writes one record to the stream, its op the packet's number and no words; replies 0.
    pub const RECORD: u32 = 4100;
    /// Takes a handle from the guest's table and replies with it.
    pub const ISSUE: u32 = 4101;
    /// Gives back the packet's handle; fails with EBADF for one that is not live.
    pub const RELEASE: u32 = 4102;
    /// Replies 0 for a live handle; fails with EBADF for any other, 0 among them.
    pub const USE: u32 = 4103;
    /// Reads the host's clock and keeps the reading; replies 0.
    pub const MARK: u32 = 4104;
    /// Reads the host's clock and replies with the whole milliseconds since the last MARK.
    pub const SINCE: u32 = 4105;
    /// Opens a file for the packet's number, its owner, and replies with the descriptor; the host's answer (SYS_OPENED) raises OPENED for that owner while the descriptor is open.
    pub const OPEN: u32 = 4106;
    /// Closes the packet's descriptor, answered or not; fails with EBADF for one not open.
    pub const CLOSE: u32 = 4107;
    /// What RAISE raises.
    pub const SIGNAL: u32 = 4353;
    /// An open answered: tagged with its owner, its result the host's.
    pub const OPENED: u32 = 4354;
    pub const KEYS: &[&str] = &["OK", "FAIL", "RAISE", "RECORD", "ISSUE", "RELEASE", "USE", "MARK", "SINCE", "OPEN", "CLOSE", "SIGNAL", "OPENED"];
}
