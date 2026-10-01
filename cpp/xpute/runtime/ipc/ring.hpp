// cpp/xpute/runtime/ipc/ring.hpp

// A single-producer, single-consumer ring of frames in the memory both sides
// see (ARCHITECTURE.xpute.md, "The message, and the rings that carry it"). A
// ring is named by where its words begin, an offset into the memory, so it
// means the same to either side however each is built.
//
// Its words: capacity (a power of two), head (the consumer's position), tail
// (the producer's), the bytes of a slot's payload, where the payloads begin,
// and three kept at 0; then a frame of four words per slot. Message n takes
// slot n modulo the capacity, and its packet lies in that slot's payload.
// Positions only grow, so tail - head is how many wait.

#pragma once

#include <cstdint>
#include <optional>
#include <span>

#include "../../core/status/errno.spec.hpp"
#include "../mem/memory.hpp"

namespace xpute {

class Ring {
  public:
    static constexpr std::uint32_t HEADER_WORDS = 8;

    // Bytes a ring of `capacity` messages takes for its words.
    static constexpr std::uint32_t bytes(std::uint32_t capacity) noexcept { return (HEADER_WORDS + capacity * 4) * 4; }

    // The ring whose words begin at `at`.
    Ring(Memory mem, std::uint32_t at) noexcept : mem_(mem), at_(at) {}

    // Lays an empty ring out at `at`, its payloads at `slot_base`.
    static Ring init(Memory mem, std::uint32_t at, std::uint32_t capacity, std::uint32_t slot_base, std::uint32_t slot_bytes) noexcept;

    std::uint32_t capacity() const noexcept { return header(CAPACITY); }
    std::uint32_t len() const noexcept { return header(TAIL) - header(HEAD); }

    // A message as the consumer finds it at the head.
    struct Message {
        std::uint32_t tag;
        std::uint32_t cmd;
        std::uint32_t flags;
        // The packet's bytes, as far as its slot runs; empty for none.
        std::span<const std::uint8_t> packet;
    };

    std::optional<Message> peek() const noexcept;
    void advance() const noexcept;

    // A message with no packet, or Errno::eagain when the ring is full.
    Errno push(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept;

  private:
    enum Header : std::uint32_t { CAPACITY, HEAD, TAIL, SLOT_BYTES, SLOT_BASE };

    std::uint32_t header(Header h) const noexcept { return mem_.word(at_ + 4 * h); }
    void set_header(Header h, std::uint32_t w) const noexcept { mem_.set_word(at_ + 4 * h, w); }
    std::uint32_t frame_at(std::uint32_t position) const noexcept;

    Memory mem_;
    std::uint32_t at_;
};

} // namespace xpute
