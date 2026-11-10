// cpp/dev/conformance/core.spec.hpp
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

#pragma once

#include <cstdint>

namespace xpute::conformance {

// The size of the range the core shares with its runtime. The range holds the front's
// header, both rings, the stream and the gaps LAYOUTS draws between them, then the core's
// own state and heap. The core's boot answers where the range begins, and so where the
// header is (the contract's rule 0). The rest of the memory, a module's stack and data,
// is the build's, and no number here sizes it.
inline constexpr std::uint32_t MEM_BYTES = 262144u;

// How many layouts a runtime runs every trace against: one for each seed below this
// number. From the seed, the core's boot draws with kit's SplitMix32 the order of the
// parts it shares with its runtime, the header among them, and how many pages to leave
// before each part. Every core draws the same way. No shared part lies where it lay under
// another seed, so only a runtime that finds every part through the header passes every
// layout (the contract's rule 0). No machine fixes where these parts lie, so the fixture
// lays its front and writes its header itself, as a runtime does for a machine.
inline constexpr std::uint32_t LAYOUTS = 8u;

// The most pages a layout leaves before each part it lays.
inline constexpr std::uint32_t GAP_PAGES = 3u;

// How many frames the submission ring holds.
inline constexpr std::uint32_t SQ_CAP = 16u;

// How many frames the completion ring holds. It is less than what eight commands leave
// when each raises MOST_RAISE frames, so a trace that sends eight of them makes a turn's
// output outgrow the ring.
inline constexpr std::uint32_t CQ_CAP = 128u;

// The size of one ring slot in bytes, enough for a packet of one number.
inline constexpr std::uint32_t SLOT_BYTES = 256u;

// The length in words of the stream's range, its END word first. The header's STREAM_AT word
// points at the stream, so the runtime runs it after every turn as it runs a GPU stream.
inline constexpr std::uint32_t STREAM_WORDS = 256u;

// The size of the stack a module core is linked with, which comes first in its memory
// (rust/dev/conformance/build.rs, cpp/Makefile).
inline constexpr std::uint32_t STACK_BYTES = 1048576u;

// The size of the core's heap, which lies in a range of its own. The memory a runtime hands
// a module never grows, and a core takes no memory it was not given.
inline constexpr std::uint32_t HEAP_BYTES = 65536u;

// The core's handle table has this many slots, and slot 0 is never handed out.
inline constexpr std::uint32_t HANDLES = 16u;

// The most frames one RAISE may ask for; a RAISE that asks for more is refused with EINVAL.
inline constexpr std::uint32_t MOST_RAISE = 32u;

// The conformance core's own commands, numbered `major << 8 | minor` past xpute's majors
// (ipc/frame.json). Where a command takes a packet, the packet is a branch whose first
// child is a number.
namespace cmd {

// Succeeds and replies 0.
inline constexpr std::uint32_t OK = 4097u;

// Fails with EINVAL.
inline constexpr std::uint32_t FAIL = 4098u;

// Raises as many NOTEs as the packet's number, tagged from 0 upward, then replies with
// that number.
inline constexpr std::uint32_t RAISE = 4099u;

// Writes one record to the stream, with the packet's number as its op and no words, and
// replies 0.
inline constexpr std::uint32_t RECORD = 4100u;

// Takes a handle from the core's table and replies with it.
inline constexpr std::uint32_t ISSUE = 4101u;

// Gives back the packet's handle; fails with EBADF for one that is not live.
inline constexpr std::uint32_t RELEASE = 4102u;

// Replies 0 for a live handle, and fails with EBADF for any other handle, 0 included.
inline constexpr std::uint32_t USE = 4103u;

// Reads the runtime's clock and keeps the reading; replies 0.
inline constexpr std::uint32_t MARK = 4104u;

// Reads the runtime's clock and replies with the whole milliseconds since the last MARK.
inline constexpr std::uint32_t SINCE = 4105u;

// Opens a file on behalf of an owner, the packet's number, and replies with the
// descriptor. The open's FS_OPENAT is posted for the file device, which no trace's runtime
// takes. When a trace completes the open (`complete`), the core raises OPENED for that
// owner if the descriptor is still open.
inline constexpr std::uint32_t OPEN = 4106u;

// Closes the packet's descriptor, whether or not its open has completed; fails with EBADF
// for a descriptor that is not open.
inline constexpr std::uint32_t CLOSE = 4107u;

// Zeroes a word of its own, calls INC with the packet's number and the word's offset, and
// replies with the word as it reads after the call. Where the runtime refuses INC, CALL
// fails with the same errno.
inline constexpr std::uint32_t CALL = 4108u;

// The call that CALL makes; its packet is a branch of a number and an offset. The runtime
// writes the number plus one at the offset and answers 0 before the call returns. It
// refuses a number of 0 with EINVAL and writes nothing.
inline constexpr std::uint32_t INC = 4109u;

// The import that RAISE raises.
inline constexpr std::uint32_t NOTE = 4353u;

// An open has completed. The frame is tagged with the open's owner, and its result is
// what the runtime completed the open with.
inline constexpr std::uint32_t OPENED = 4354u;
} // namespace cmd

} // namespace xpute::conformance
