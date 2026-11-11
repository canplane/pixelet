// xpute-conformance/core.spec.rs
//
// GENERATED from spec/conformance/core.json
//
// The conformance core, which the contract's traces drive (spec/conformance/door.tsv):
// the few commands that every language's conformance core implements, and the numbers a
// trace is written against. The core computes nothing, and asks for no turn when nothing
// waits. How it works inside is each core's own; the traces hold only what the runtime
// sees of it.
//
// A trace names no number a production core uses. These numbers are the fixture's own,
// chosen so that a trace can make a turn's output outgrow its room without knowing how a
// core reserves that room.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

/// The size of the range the core shares with its runtime. The range holds the front's
/// header, the frame it receives into, the frame it sends from, the stream and the gaps
/// LAYOUTS draws between them, then the core's own state and heap. The core's boot
/// answers where the range begins, and so where the header is (the contract's rule 0).
/// The rest of the memory, a module's stack and data, is the build's, and no number here
/// sizes it.
pub const MEM_BYTES: u32 = 262144;

/// How many layouts a runtime runs every trace against: one for each seed below this
/// number. From the seed, the core's boot draws with kit's SplitMix32 the order of the
/// parts it shares with its runtime, the header among them, and how many pages to leave
/// before each part. Every core draws the same way. No shared part lies where it lay under
/// another seed, so only a runtime that finds every part through the header passes every
/// layout (the contract's rule 0). No manifest.json fixes where these parts lie, so the fixture
/// lays its front and writes its header itself, as a runtime does from a core's manifest.
pub const LAYOUTS: u32 = 8;

/// The most pages a layout leaves before each part it lays.
pub const GAP_PAGES: u32 = 3;

/// The most bytes a frame's packet takes, either way, enough for a packet of one number.
pub const PKT_BYTES: u32 = 256;

/// The length in words of the stream's range, its END word first. The header's STREAM_AT word
/// points at the stream, so the runtime runs it after every turn as it runs a GPU stream.
pub const STREAM_WORDS: u32 = 256;

/// The size of the stack a module core is linked with, which comes first in its memory
/// (rust/dev/conformance/build.rs, cpp/Makefile).
pub const STACK_BYTES: u32 = 1048576;

/// The size of the core's heap, which lies in a range of its own. The memory a runtime hands
/// a module never grows, and a core takes no memory it was not given.
pub const HEAP_BYTES: u32 = 65536;

/// The core's handle table has this many slots, and slot 0 is never handed out.
pub const HANDLES: u32 = 16;

/// The most frames one RAISE may ask for; a RAISE that asks for more is refused with EINVAL.
pub const MOST_RAISE: u32 = 32;

/// The most bytes one NAMED may name; a NAMED that asks for more is refused with EINVAL.
pub const MOST_NAMED: u32 = 16;

/// The conformance core's own commands, numbered `major << 8 | minor` past xpute's majors
/// (io/frame.json). Where a command takes a packet, the packet is a branch whose first
/// child is a number.
pub mod cmd {
    /// Succeeds and replies 0.
    pub const OK: u32 = 4097;
    /// Fails with EINVAL.
    pub const FAIL: u32 = 4098;
    /// Raises as many NOTEs as the packet's number, tagged from 0 upward, then replies with
    /// that number.
    pub const RAISE: u32 = 4099;
    /// Writes one record to the stream, with the packet's number as its op and no words, and
    /// replies 0.
    pub const RECORD: u32 = 4100;
    /// Takes a handle from the core's table and replies with it.
    pub const ISSUE: u32 = 4101;
    /// Gives back the packet's handle; fails with EBADF for one that is not live.
    pub const RELEASE: u32 = 4102;
    /// Replies 0 for a live handle, and fails with EBADF for any other handle, 0 included.
    pub const USE: u32 = 4103;
    /// Reads the runtime's clock and keeps the reading; replies 0.
    pub const MARK: u32 = 4104;
    /// Reads the runtime's clock and replies with the whole milliseconds since the last MARK.
    pub const SINCE: u32 = 4105;
    /// Opens the file (1, "f") on behalf of an owner, the packet's number, and replies with the
    /// descriptor. The open's FS_OPEN is posted for the file device, which no trace's runtime
    /// takes. When a trace completes the open (`complete`), the core raises OPEN_COMPLETION for
    /// that owner if the descriptor is still open.
    pub const OPEN: u32 = 4106;
    /// Closes the packet's descriptor, whether or not its open has completed; fails with EBADF
    /// for a descriptor that is not open.
    pub const CLOSE: u32 = 4107;
    /// Zeroes a word of its own, calls INC with the packet's number and the word's offset, and
    /// replies with the word as it reads after the call. Where the runtime refuses INC, CALL
    /// fails with the same errno.
    pub const CALL: u32 = 4108;
    /// The call that CALL makes; its packet is a branch of a number and an offset. The runtime
    /// writes the number plus one at the offset and answers 0 before the call returns. It
    /// refuses a number of 0 with EINVAL and writes nothing.
    pub const INC: u32 = 4109;
    /// Replies 0, its reply naming as many bytes of the core's own state as the packet's
    /// number, byte k holding k, which go with the reply's frame; 0 names none.
    pub const NAMED: u32 = 4110;
    /// The import that RAISE raises.
    pub const NOTE: u32 = 4353;
    /// An open's completion, handed on. The frame is tagged with the open's owner, and its
    /// result is what the runtime completed the open with.
    pub const OPEN_COMPLETION: u32 = 4354;
    pub const KEYS: &[&str] = &[
        "OK",
        "FAIL",
        "RAISE",
        "RECORD",
        "ISSUE",
        "RELEASE",
        "USE",
        "MARK",
        "SINCE",
        "OPEN",
        "CLOSE",
        "CALL",
        "INC",
        "NAMED",
        "NOTE",
        "OPEN_COMPLETION",
    ];
}
