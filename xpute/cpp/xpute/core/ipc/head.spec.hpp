// cpp/xpute/core/ipc/head.spec.hpp
//
// GENERATED from spec/ipc/head.json
//
// The front's header: the words a runtime writes on the page past a core's stack before its
// first turn, as io_uring's setup lays its rings, and that the core's boot holds to its
// machine, stopping with ENOEXEC where one differs (xpute-core machine.rs). Both ends
// compute every number in it from the same machine, so it names what the runtime laid, not
// where to look: the rings' sizes and places, and the GPU stream's. The runtime lays each
// ring's own header too (ring.json), its positions at 0.

#pragma once

#include <cstdint>

namespace xpute::head {

// "XPUT" as four bytes read little-endian: what tells a header from memory nothing wrote.
inline constexpr std::uint32_t MAGIC = 1414877272u;

// Where each of the header's words is, from its start.
namespace word {

// Where MAGIC is.
inline constexpr std::uint32_t MAGIC = 0u;

// Frames the submission ring holds.
inline constexpr std::uint32_t SQ_CAP = 1u;

// Frames the completion ring holds.
inline constexpr std::uint32_t CQ_CAP = 2u;

// A ring slot's payload, the same for both rings.
inline constexpr std::uint32_t SLOT_BYTES = 3u;

// Where the submission ring's words begin.
inline constexpr std::uint32_t SQ_AT = 4u;

// Where the completion ring's words begin.
inline constexpr std::uint32_t CQ_AT = 5u;

// Where the GPU stream is, which the runtime runs after each turn; 0 where the machine asks for no GPU.
inline constexpr std::uint32_t GPU_AT = 6u;
} // namespace word

// Words the header takes.
inline constexpr std::uint32_t WORDS = 7u;

} // namespace xpute::head
