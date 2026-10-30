// cpp/xpute/guest/ipc/ring.spec.hpp
//
// GENERATED from spec/ipc/ring.json
//
// A single-producer, single-consumer ring of frames (frame.json) in memory both sides
// see, named by its offset so it means the same to either. A header, then the frames;
// their payloads lie apart in slots, as in virtio and AF_XDP. Position n owns frame
// and slot n modulo CAP, so a slot is free exactly when its frame is.
// Positions only grow, and each side writes only its own word, so no lock is needed.

#pragma once

#include <cstdint>

namespace xpute::ring {

// How many frames, a power of two.
inline constexpr std::uint32_t CAP = 0u;

// The consumer's position: the next frame it reads.
inline constexpr std::uint32_t HEAD = 1u;

// The producer's position: where the next frame goes.
inline constexpr std::uint32_t TAIL = 2u;

// A payload slot's size, a power of two and a multiple of 8.
inline constexpr std::uint32_t SLOT_BYTES = 3u;

// Where the slots start, as an offset in the memory.
inline constexpr std::uint32_t SLOT_BASE = 4u;

// Kept at 0.
inline constexpr std::uint32_t RESERVED_0 = 5u;

// Kept at 0.
inline constexpr std::uint32_t RESERVED_1 = 6u;

// Kept at 0.
inline constexpr std::uint32_t RESERVED_2 = 7u;

// Words before the first frame.
inline constexpr std::uint32_t HEADER_WORDS = 8u;

} // namespace xpute::ring
