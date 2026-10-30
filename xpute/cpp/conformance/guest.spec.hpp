// cpp/conformance/guest.spec.hpp
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

#pragma once

#include <cstdint>

namespace xpute::conformance {

// The room a guest's shared range asks for: the directory, both rings, the two streams
// and the gaps LAYOUTS draws between them, then its own state and heap. Where the range
// begins, and so where the directory is, the guest's boot answers (the contract's rule 0);
// what else a memory holds — a module's stack and data — is the build's, and no number here.
inline constexpr std::uint32_t MEM_BYTES = 262144u;

// The layouts a host runs every trace against, one for each seed below this. The guest's
// boot takes the seed and draws from it (kit's SplitMix32) the order of what it shares
// with its host, the directory among them, and the pages left before each, every guest by
// the same draw; so nothing it shares lies where it lay under another seed, and only a
// host that finds it all through the directory passes every layout (the contract's rule 0).
inline constexpr std::uint32_t LAYOUTS = 8u;

// The most pages a layout leaves before each part it lays.
inline constexpr std::uint32_t GAP_PAGES = 3u;

// Frames the submission ring holds.
inline constexpr std::uint32_t SQ_CAP = 16u;

// Frames the completion ring holds: fewer than eight commands raising MOST_RAISE
// events each leave, so a trace that sends those makes a turn's output outgrow it.
inline constexpr std::uint32_t CQ_CAP = 128u;

// A ring slot's payload: a packet of one number fits.
inline constexpr std::uint32_t SLOT_BYTES = 256u;

// The stream's range, its END word first.
inline constexpr std::uint32_t STREAM_WORDS = 256u;

// The system calls' stream: OPEN and CLOSE are recorded there, and nothing runs them.
inline constexpr std::uint32_t SYS_WORDS = 64u;

// The stack a module guest is linked with, first in its memory (rust/conformance/build.rs, cpp/Makefile).
inline constexpr std::uint32_t STACK_BYTES = 1048576u;

// The guest's heap, in its own range, since the memory a host hands a module never grows
// and a guest takes nothing it was not given.
inline constexpr std::uint32_t HEAP_BYTES = 65536u;

// Slots in the guest's handle table, slot 0 never handed out.
inline constexpr std::uint32_t HANDLES = 16u;

// The most events one RAISE asks for; more is refused with EINVAL.
inline constexpr std::uint32_t MOST_RAISE = 32u;

// The guest's own entry in its directory, past xpute's PROGRAM bit.
namespace key {

// Where the stream is, run by the host after every turn.
inline constexpr std::uint32_t STREAM = 2147483649u;
} // namespace key

// The guest's own hostcall, past xpute's PROGRAM bit.
namespace hostcall {

// A number and an offset: the host writes the number plus one at the offset and answers 0,
// before the hostcall returns; a number of 0 it refuses with EINVAL, writing nothing.
inline constexpr std::uint32_t ANSWER = 2147483649u;
} // namespace hostcall

// The commands, `major << 8 | minor`, on the majors a program numbers its own from
// (xpute's PROGRAM_MAJOR and up), so none is one of xpute's — SYS_OPENED among them. A
// packet, where one is taken, is a branch whose first child is a number.
namespace cmd {

// Succeeds; replies 0.
inline constexpr std::uint32_t OK = 4097u;

// Fails with EINVAL.
inline constexpr std::uint32_t FAIL = 4098u;

// Raises the packet's number of EVENTs, tagged 0 up, then replies with that number.
inline constexpr std::uint32_t RAISE = 4099u;

// Writes one record to the stream, its op the packet's number and no words; replies 0.
inline constexpr std::uint32_t RECORD = 4100u;

// Takes a handle from the guest's table and replies with it.
inline constexpr std::uint32_t ISSUE = 4101u;

// Gives back the packet's handle; fails with EBADF for one that is not live.
inline constexpr std::uint32_t RELEASE = 4102u;

// Replies 0 for a live handle; fails with EBADF for any other, 0 among them.
inline constexpr std::uint32_t USE = 4103u;

// Reads the host's clock and keeps the reading; replies 0.
inline constexpr std::uint32_t MARK = 4104u;

// Reads the host's clock and replies with the whole milliseconds since the last MARK.
inline constexpr std::uint32_t SINCE = 4105u;

// Opens a file for the packet's number, its owner, and replies with the descriptor; the host's answer (SYS_OPENED) raises OPENED for that owner while the descriptor is open.
inline constexpr std::uint32_t OPEN = 4106u;

// Closes the packet's descriptor, answered or not; fails with EBADF for one not open.
inline constexpr std::uint32_t CLOSE = 4107u;

// Zeroes a word of its own, makes the hostcall ANSWER of the packet's number and the word's offset, and replies with the word as it then reads it; fails with the host's errno where the host refused.
inline constexpr std::uint32_t HOSTCALL = 4108u;

// What RAISE raises.
inline constexpr std::uint32_t EVENT = 4353u;

// An open answered: tagged with its owner, its result the host's.
inline constexpr std::uint32_t OPENED = 4354u;
} // namespace cmd

} // namespace xpute::conformance
