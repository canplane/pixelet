// cpp/xpute/runtime/ipc/ring.hpp

// A single-producer, single-consumer ring of frames in shared memory, named by
// its offset so it means the same to either side. Words: capacity, head, tail,
// slot bytes, slot base, three zeros, then four words per frame. Message n takes
// slot n modulo capacity; positions only grow, so tail - head is how many wait.

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

    static constexpr std::uint32_t bytes(std::uint32_t capacity) noexcept { return (HEADER_WORDS + capacity * frame::WORDS) * 4; }

    static constexpr std::uint32_t slots_bytes(std::uint32_t capacity, std::uint32_t slot_bytes) noexcept { return capacity * slot_bytes; }

    // A capacity that is not a power of two is a bug.
    Ring(Memory mem, std::uint32_t at) noexcept;

    static Ring init(Memory mem, std::uint32_t at, std::uint32_t capacity, std::uint32_t slot_base, std::uint32_t slot_bytes) noexcept;

    std::uint32_t capacity() const noexcept { return header(CAPACITY); }
    std::uint32_t slot_bytes() const noexcept { return header(SLOT_BYTES); }
    std::uint32_t head() const noexcept { return header(HEAD); }
    std::uint32_t tail() const noexcept { return header(TAIL); }
    std::uint32_t len() const noexcept { return tail() - head(); }
    bool empty() const noexcept { return len() == 0; }

    // EAGAIN when full, EMSGSIZE when larger than a slot.
    Errno push(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result,
               std::optional<std::span<const std::uint8_t>> packet = std::nullopt) const noexcept;

    // `write` gets the whole slot payload and answers the length, or none if it
    // did not fit (EMSGSIZE).
    template <class Write> Errno push_in_place(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, Write &&write) const noexcept {
        if (len() >= capacity()) return Errno::eagain;
        std::uint32_t slot = slot_at(tail());
        std::optional<std::uint32_t> n = write(std::span<std::uint8_t>(mem_.at(slot), slot_bytes()));
        if (!n || *n > slot_bytes()) return Errno::emsgsize;
        pad(slot, *n);
        publish(tag, cmd, flags, result, slot);
        return Errno::ok;
    }

    struct Message {
        std::uint32_t tag;
        std::uint32_t cmd;
        std::uint32_t flags;
        std::int32_t result;
        // 0 for none.
        std::uint32_t packet_at;
    };

    // Read it, then `advance`.
    std::optional<Message> peek() const noexcept;

    // Valid until `advance`. EBADMSG when it names bytes outside a slot.
    Result<std::span<const std::uint8_t>> packet(const Message &m) const noexcept;

    void advance() const noexcept;

    // A message with no packet, or Errno::eagain when the ring is full.
    Errno push(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept;

  private:
    enum Header : std::uint32_t { CAPACITY, HEAD, TAIL, SLOT_BYTES, SLOT_BASE };

    std::uint32_t header(Header h) const noexcept { return mem_.word(at_ + 4 * h); }
    void set_header(Header h, std::uint32_t w) const noexcept { mem_.set_word(at_ + 4 * h, w); }
    std::uint32_t frame_at(std::uint32_t position) const noexcept;
    std::uint32_t slot_at(std::uint32_t position) const noexcept;
    void pad(std::uint32_t slot, std::uint32_t n) const noexcept;
    void publish(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::uint32_t packet_at) const noexcept;

    Memory mem_;
    std::uint32_t at_;
};

} // namespace xpute
