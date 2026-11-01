// xpute-conformance/guest.spec.rs
//
// GENERATED from spec/conformance/guest.json
//
// The guest the contract's traces drive (spec/conformance/door.tsv): the
// few commands every language's conformance guest implements, and the numbers a trace
// is written against. It computes nothing, and asks for no turn when nothing waits; what
// it does inside is each guest's own, and what the host sees of it is what the traces
// hold it to.
//
// A trace names no production number. These are the fixture's, chosen so that a trace
// can make a turn's output outgrow its room without knowing how a guest reserves it.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The room a guest's shared range asks for: the toc, both rings, the two streams
/// and the gaps LAYOUTS draws between them, then its own state and heap. Where the range
/// begins, and so where the toc is, the guest's boot answers (the contract's rule 0);
/// what else a memory holds — a module's stack and data — is the build's, and no number here.
pub const MEM_BYTES: u32 = 262144;

/// The layouts a host runs every trace against, one for each seed below this. The guest's
/// boot takes the seed and draws from it (kit's SplitMix32) the order of what it shares
/// with its host, the toc among them, and the pages left before each, every guest by
/// the same draw; so nothing it shares lies where it lay under another seed, and only a
/// host that finds it all through the toc passes every layout (the contract's rule 0).
pub const LAYOUTS: u32 = 8;

/// The most pages a layout leaves before each part it lays.
pub const GAP_PAGES: u32 = 3;

/// Frames the submission ring holds.
pub const SQ_CAP: u32 = 16;

/// Frames the completion ring holds: fewer than eight commands raising MOST_RAISE
/// events each leave, so a trace that sends those makes a turn's output outgrow it.
pub const CQ_CAP: u32 = 128;

/// A ring slot's payload: a packet of one number fits.
pub const SLOT_BYTES: u32 = 256;

/// The stream's range, its END word first.
pub const STREAM_WORDS: u32 = 256;

/// The hostcalls' stream: OPEN and CLOSE are recorded there, and nothing runs them.
pub const SYS_WORDS: u32 = 64;

/// The stack a module guest is linked with, first in its memory (rust/conformance/build.rs, cpp/Makefile).
pub const STACK_BYTES: u32 = 1048576;

/// The guest's heap, in its own range, since the memory a host hands a module never grows
/// and a guest takes nothing it was not given.
pub const HEAP_BYTES: u32 = 65536;

/// Slots in the guest's handle table, slot 0 never handed out.
pub const HANDLES: u32 = 16;

/// The most events one RAISE asks for; more is refused with EINVAL.
pub const MOST_RAISE: u32 = 32;

/// The guest's own entry in its toc, past xpute's PROGRAM bit.
pub mod key {
    /// Where the stream is, run by the host after every turn.
    pub const STREAM: u32 = 2147483649;
    pub const KEYS: &[&str] = &["STREAM"];
}

/// The guest's own hostcall, past xpute's PROGRAM bit.
pub mod hostcall {
    /// A number and an offset: the host writes the number plus one at the offset and answers 0,
    /// before the hostcall returns; a number of 0 it refuses with EINVAL, writing nothing.
    pub const ANSWER: u32 = 2147483649;
    pub const KEYS: &[&str] = &["ANSWER"];
}

/// The port a trace sends the guest's own commands as, and the one the guest raises
/// its events for (ipc/frame.json PORT_SHIFT).
pub const SENDER: u32 = 1;

/// The guest's own commands, `major << 8 | minor`, which a trace sends as SENDER. A packet,
/// where one is taken, is a branch whose first child is a number.
pub mod cmd {
    /// Succeeds; replies 0.
    pub const OK: u32 = 4097;
    /// Fails with EINVAL.
    pub const FAIL: u32 = 4098;
    /// Raises the packet's number of EVENTs, tagged 0 up, then replies with that number.
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
    /// Zeroes a word of its own, makes the hostcall ANSWER of the packet's number and the word's offset, and replies with the word as it then reads it; fails with the host's errno where the host refused.
    pub const HOSTCALL: u32 = 4108;
    /// What RAISE raises.
    pub const EVENT: u32 = 4353;
    /// An open answered: tagged with its owner, its result the host's.
    pub const OPENED: u32 = 4354;
    pub const KEYS: &[&str] = &[
        "OK", "FAIL", "RAISE", "RECORD", "ISSUE", "RELEASE", "USE", "MARK", "SINCE", "OPEN",
        "CLOSE", "HOSTCALL", "EVENT", "OPENED",
    ];
}
