// cpp/xpute/core/ipc/stream.spec.hpp
//
// GENERATED from spec/ipc/stream.json
//
// A stream's own words, at its start (xpute-core ipc/stream.rs): a turn's calls from core to
// runtime, run after the turn returns. Word END is the count of record words past the head,
// and word HEAD the head's own count of words, so a runtime finds the records from the stream
// itself; a record is an op, a count n, then n words. The core publishes END with release; the
// runtime loads it with acquire and, once it has run the records, stores 0 with release, which
// hands the stream back to the core for its next records.

#pragma once

#include <cstdint>

namespace xpute::stream {

// Where the count of record words past the head is; 0 while the stream is the core's.
inline constexpr std::uint32_t END = 0u;

// Where the head's own count of words is: the stream's own and its user's.
inline constexpr std::uint32_t HEAD = 1u;

// The stream's own words, before its user's.
inline constexpr std::uint32_t OWN_WORDS = 2u;

} // namespace xpute::stream
