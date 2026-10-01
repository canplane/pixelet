// cpp/xpute/runtime/ipc/ring.hpp

// A single-producer, single-consumer ring of frames in the memory both sides
// see (ARCHITECTURE.xpute.md, "The message, and the rings that carry it"). A
// ring is named by where its words begin, an offset into the memory, so it
// means the same to either side however each is built.
//
// Its words: capacity (a power of two), head (the consumer's position), tail
// (the producer's), the bytes of a slot's payload (a power of two, and a
// multiple of 8), where the payloads begin, and three kept at 0; then a frame
// of four words per slot. Message n takes slot n modulo the capacity, and its
// packet lies in that slot's payload, so a payload is free exactly when its
// slot is. Positions only grow, so tail - head is how many wait.
//
// A push fails when the ring is full (EAGAIN) and for no other reason; a
// packet larger than a slot is EMSGSIZE whatever the ring holds.

#pragma once

#include <cstdint>
#include <optional>
#include <span>

#include "../../core/status/errno.spec.hpp"
#include "../../core/status/error.hpp"
#include "../mem/memory.hpp"
#include "frame.hpp"

namespace xpute {

class Ring {
  public:
    static constexpr std::uint32_t HEADER_WORDS = 8;

    // Bytes a ring of `capacity` messages takes for its words.
    static constexpr std::uint32_t bytes(std::uint32_t capacity) noexcept { return (HEADER_WORDS + capacity * frame::WORDS) * 4; }

    // Bytes the payloads of `capacity` slots of `slot_bytes` take.
    static constexpr std::uint32_t slots_bytes(std::uint32_t capacity, std::uint32_t slot_bytes) noexcept { return capacity * slot_bytes; }

    // The ring already laid out whose words begin at `at`. A capacity that is
    // not a power of two is no ring, and a bug.
    Ring(Memory mem, std::uint32_t at) noexcept;

    // Lays an empty ring out at `at`, its payloads at `slot_base`.
    static Ring init(Memory mem, std::uint32_t at, std::uint32_t capacity, std::uint32_t slot_base, std::uint32_t slot_bytes) noexcept;

    std::uint32_t capacity() const noexcept { return header(CAPACITY); }
    std::uint32_t slot_bytes() const noexcept { return header(SLOT_BYTES); }
    std::uint32_t head() const noexcept { return header(HEAD); }
    std::uint32_t tail() const noexcept { return header(TAIL); }
    // Messages waiting to be consumed.
    std::uint32_t len() const noexcept { return tail() - head(); }
    bool empty() const noexcept { return len() == 0; }

    // ---- producer ----

    // A message at the tail, its packet copied into the payload of the slot
    // it takes: EAGAIN, and nothing written, when the ring is full; EMSGSIZE
    // when the packet is larger than a slot.
    Errno push(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result,
               std::optional<std::span<const std::uint8_t>> packet = std::nullopt) const noexcept;

    // A message at the tail, its packet written in place by `write`: handed
    // the whole payload of the slot it takes, it answers the packet's length,
    // or none when the packet did not fit — EMSGSIZE, which does not depend
    // on what was sent before it.
    template <class Write> Errno push_in_place(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, Write &&write) const noexcept {
        if (len() >= capacity()) return Errno::eagain;
        std::uint32_t slot = slot_at(tail());
        std::optional<std::uint32_t> n = write(std::span<std::uint8_t>(mem_.at(slot), slot_bytes()));
        if (!n || *n > slot_bytes()) return Errno::emsgsize;
        pad(slot, *n);
        publish(tag, cmd, flags, result, slot);
        return Errno::ok;
    }

    // ---- consumer ----

    // A message as the consumer finds it at the head.
    struct Message {
        std::uint32_t tag;
        std::uint32_t cmd;
        std::uint32_t flags;
        std::int32_t result;
        // Where its packet lies in the memory, 0 for none.
        std::uint32_t packet_at;
    };

    // The message at the head, or none when the ring is empty. Read it, then
    // `advance`.
    std::optional<Message> peek() const noexcept;

    // The message's packet, its bytes to the word and valid until `advance`,
    // or empty when it has none. EBADMSG when it names bytes outside a slot.
    Result<std::span<const std::uint8_t>> packet(const Message &m) const noexcept;

    // Consumes the message at the head; one from an empty ring is a bug.
    void advance() const noexcept;

  private:
    enum Header : std::uint32_t { CAPACITY, HEAD, TAIL, SLOT_BYTES, SLOT_BASE };

    std::uint32_t header(Header h) const noexcept { return mem_.word(at_ + 4 * h); }
    void set_header(Header h, std::uint32_t w) const noexcept { mem_.set_word(at_ + 4 * h, w); }
    std::uint32_t frame_at(std::uint32_t position) const noexcept;
    // Where the payload of the message at `position` lies in the memory.
    std::uint32_t slot_at(std::uint32_t position) const noexcept;
    // Zeroes the slot at `slot` from `n` to the next word.
    void pad(std::uint32_t slot, std::uint32_t n) const noexcept;
    // Writes the frame at the tail and moves the tail past it.
    void publish(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::uint32_t packet_at) const noexcept;

    Memory mem_;
    std::uint32_t at_;
};

} // namespace xpute
