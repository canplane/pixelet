// cpp/xpute/core/ipc/head.spec.hpp
//
// GENERATED from spec/ipc/head.json
//
// The header at the front of a core's memory. The front is what the runtime lays out before
// the core's first turn: this header, the two rings and their slots, and the GPU stream
// (ARCHITECTURE.md). The header lies on the page past the core's stack. The runtime writes it
// as io_uring's setup lays its rings, and lays each ring's own header too (ring.json), with
// its positions at 0. Both ends compute every number in the header from the same machine, so
// the header records what the runtime laid; it does not tell the core where to look. On its
// first turn the core holds each word to its machine and stops with ENOEXEC where one
// differs (xpute-core machine.rs).

#pragma once

#include <cstdint>

namespace xpute::head {

// The bytes "XPUT" read as a little-endian word. It tells a laid header from memory that
// nothing has written.
inline constexpr std::uint32_t MAGIC = 1414877272u;

// The index of each header word, counted from the header's start.
namespace word {

inline constexpr std::uint32_t MAGIC = 0u;

// How many frames the submission ring holds.
inline constexpr std::uint32_t SQ_CAP = 1u;

// How many frames the completion ring holds.
inline constexpr std::uint32_t CQ_CAP = 2u;

// The size of one ring slot in bytes, the same for both rings.
inline constexpr std::uint32_t SLOT_BYTES = 3u;

// The offset in the memory where the submission ring begins.
inline constexpr std::uint32_t SQ_AT = 4u;

// The offset in the memory where the completion ring begins.
inline constexpr std::uint32_t CQ_AT = 5u;

// The offset of the GPU stream, which the runtime runs after each turn; 0 when the machine
// asks for no GPU.
inline constexpr std::uint32_t GPU_AT = 6u;
} // namespace word

// The header's length in words.
inline constexpr std::uint32_t WORDS = 7u;

} // namespace xpute::head
