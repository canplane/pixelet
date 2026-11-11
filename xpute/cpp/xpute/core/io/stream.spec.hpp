// cpp/xpute/core/io/stream.spec.hpp
//
// GENERATED from spec/io/stream.json
//
// A stream carries a turn's calls from the core to the runtime as records, which the runtime
// runs once the core publishes them (xpute-core io/stream.rs). The stream begins with a head,
// and the names here are the indices of the head words that belong to the stream itself.
// Word END holds how many words of records follow the head, and word HEAD holds the head's
// length in words, so a runtime finds the records from the stream alone. A record is an op,
// a count n, and then n words. The core publishes END with release. The runtime loads it
// with acquire, runs the records, and then stores 0 with release; that store hands the
// stream back to the core for its next records.

#pragma once

#include <cstdint>

namespace xpute::stream {

// Holds how many words of records follow the head; 0 while the stream belongs to the core.
inline constexpr std::uint32_t END = 0u;

// Holds the head's length in words, counting the stream's own words and its user's.
inline constexpr std::uint32_t HEAD = 1u;

// How many head words belong to the stream itself; the user's words follow them.
inline constexpr std::uint32_t OWN_WORDS = 2u;

} // namespace xpute::stream
