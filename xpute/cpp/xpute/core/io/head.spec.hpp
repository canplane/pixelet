// cpp/xpute/core/io/head.spec.hpp
//
// GENERATED from spec/io/head.json
//
// The header at the front of a core's memory. The front is what the runtime lays out before
// the core's first turn: this header and the GPU stream (ARCHITECTURE.md). The header lies on
// the page past the core's stack. Both ends compute every number in the header from the same
// manifest, so the header records what the runtime laid; it does not tell the core where to
// look. On its first turn the core holds each word to its manifest and stops with ENOEXEC
// where one differs (xpute-core manifest.rs).

#pragma once

#include <cstdint>

namespace xpute::head {

// The bytes "XPUT" read as a little-endian word. It tells a laid header from memory that
// nothing has written.
inline constexpr std::uint32_t MAGIC = 1414877272u;

// The index of each header word, counted from the header's start.
namespace word {

inline constexpr std::uint32_t MAGIC = 0u;

// The most bytes a frame's packet takes, either way.
inline constexpr std::uint32_t PKT_BYTES = 1u;

// The offset of the stream, which is run after each turn; 0 when the manifest
// asks for none.
inline constexpr std::uint32_t STREAM_AT = 2u;
} // namespace word

// The header's length in words.
inline constexpr std::uint32_t WORDS = 3u;

} // namespace xpute::head
