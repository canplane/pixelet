// cpp/guest/ipc/ring.hpp

// A ring of frames, laid out by spec/ipc/ring.json.

#pragma once

#include <cstdint>
#include <optional>
#include <span>

#include "../../kit/status/errno.spec.hpp"
#include "../../kit/status/error.hpp"
#include "../mem/memory.hpp"
#include "frame.hpp"
#include "ring.spec.hpp"

namespace xpute {

class Ring {
  public:
    static constexpr std::uint32_t bytes(std::uint32_t capacity) noexcept { return (ring::HEADER_WORDS + capacity * frame::WORDS) * 4; }

    static constexpr std::uint32_t slots_bytes(std::uint32_t capacity, std::uint32_t slot_bytes) noexcept { return capacity * slot_bytes; }

    // A capacity that is not a power of two is a bug.
    Ring(Memory mem, std::uint32_t at) noexcept;

    static Ring init(Memory mem, std::uint32_t at, std::uint32_t capacity, std::uint32_t slot_base, std::uint32_t slot_bytes) noexcept;

    std::uint32_t capacity() const noexcept { return header(ring::CAPACITY); }
    std::uint32_t slot_bytes() const noexcept { return header(ring::SLOT_BYTES); }
    std::uint32_t head() const noexcept { return header(ring::HEAD); }
    std::uint32_t tail() const noexcept { return header(ring::TAIL); }
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

  private:
    std::uint32_t header(std::uint32_t word) const noexcept { return mem_.word(at_ + 4 * word); }
    void set_header(std::uint32_t word, std::uint32_t w) const noexcept { mem_.set_word(at_ + 4 * word, w); }
    std::uint32_t frame_at(std::uint32_t position) const noexcept;
    std::uint32_t slot_at(std::uint32_t position) const noexcept;
    void pad(std::uint32_t slot, std::uint32_t n) const noexcept;
    void publish(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::uint32_t packet_at) const noexcept;

    Memory mem_;
    std::uint32_t at_;
};

} // namespace xpute
