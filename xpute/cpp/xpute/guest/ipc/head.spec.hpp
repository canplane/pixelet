// cpp/xpute/guest/ipc/head.spec.hpp
//
// GENERATED from spec/ipc/head.json
//
// The front's header: the words a host writes on the page past a guest's stack before its
// first turn, as io_uring's setup lays its rings, and that the guest's boot holds to its
// machine, stopping with ENOEXEC where one differs (xpute-guest machine.rs). Both ends
// compute every number in it from the same machine, so it names what the host laid, not
// where to look: the rings' sizes and places, and the GPU stream's. The host lays each
// ring's own header too (ring.json), its positions at 0.

#pragma once

#include <cstdint>

namespace xpute::head {

// "XPUT" as four bytes read little-endian: what tells a header from memory nothing wrote.
inline constexpr std::uint32_t MAGIC = 1414877272u;

// Raised when a word comes to mean something else. 1 was the toc's, a directory of keys.
inline constexpr std::uint32_t VERSION = 2u;

// Where each of the header's words is, from its start.
namespace word {

// Where MAGIC is.
inline constexpr std::uint32_t MAGIC = 0u;

// Where VERSION is.
inline constexpr std::uint32_t VERSION = 1u;

// Frames the submission ring holds.
inline constexpr std::uint32_t SQ_CAP = 2u;

// Frames the completion ring holds.
inline constexpr std::uint32_t CQ_CAP = 3u;

// A ring slot's payload, the same for both rings.
inline constexpr std::uint32_t SLOT_BYTES = 4u;

// Where the submission ring's words begin.
inline constexpr std::uint32_t SQ_AT = 5u;

// Where the completion ring's words begin.
inline constexpr std::uint32_t CQ_AT = 6u;

// Where the GPU stream is, which the host runs after each turn; 0 without the GPU socket.
inline constexpr std::uint32_t GPU_AT = 7u;
} // namespace word

// Words the header takes.
inline constexpr std::uint32_t WORDS = 8u;

} // namespace xpute::head
