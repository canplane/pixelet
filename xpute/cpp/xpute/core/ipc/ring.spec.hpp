// cpp/xpute/core/ipc/ring.spec.hpp
//
// GENERATED from spec/ipc/ring.json
//
// A ring of frames (frame.json) with one producer and one consumer, in memory both sides
// can see. Each side names the ring by its offset in that memory, so the name means the
// same to both. The ring begins with a header of HEADER_WORDS 32-bit words, and every other
// name here is the index of one header word. The frames follow the header; their payloads
// lie apart in slots, as in virtio and AF_XDP. Position n owns frame n modulo the capacity
// and the slot of the same index, so a slot is free exactly when its frame is. Positions
// only count up, and each side writes only its own position word, so the ring needs no lock.

#pragma once

#include <cstdint>

namespace xpute::ring {

// How many frames the ring holds, a power of two.
inline constexpr std::uint32_t CAP = 0u;

// The consumer's position: the next frame it reads.
inline constexpr std::uint32_t HEAD = 1u;

// The producer's position: where the next frame goes.
inline constexpr std::uint32_t TAIL = 2u;

// The size of one payload slot in bytes, a power of two and a multiple of 8.
inline constexpr std::uint32_t SLOT_BYTES = 3u;

// The offset in the memory where the slots start.
inline constexpr std::uint32_t SLOT_BASE = 4u;

// Reserved; it holds 0.
inline constexpr std::uint32_t RESERVED_0 = 5u;

// Reserved; it holds 0.
inline constexpr std::uint32_t RESERVED_1 = 6u;

// Reserved; it holds 0.
inline constexpr std::uint32_t RESERVED_2 = 7u;

// The header's length in words; the first frame follows it.
inline constexpr std::uint32_t HEADER_WORDS = 8u;

} // namespace xpute::ring
