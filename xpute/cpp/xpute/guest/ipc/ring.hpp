// cpp/xpute/guest/ipc/ring.hpp

// A ring of frames, laid out by spec/ipc/ring.json.

#pragma once

#include <cstdint>
#include <optional>
#include <span>

#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/guest/mem.hpp"
#include "frame.hpp"
#include "ring.spec.hpp"

namespace xpute {

constexpr std::uint32_t ring_bytes(std::uint32_t cap) noexcept {
    return (ring::HEADER_WORDS + cap * frame::WORDS) * 4;
}

constexpr std::uint32_t slots_bytes(std::uint32_t cap, std::uint32_t slot_bytes) noexcept {
    return cap * slot_bytes;
}

class Ring {
  public:
    // A cap that is not a power of two is a bug.
    Ring(Memory mem, std::uint32_t at) noexcept;

    static Ring init(Memory mem, std::uint32_t at, std::uint32_t cap, std::uint32_t slot_base, std::uint32_t slot_bytes) noexcept;

    std::uint32_t cap() const noexcept { return cap_; }
    std::uint32_t slot_bytes() const noexcept { return slot_bytes_; }
    std::uint32_t head() const noexcept { return header(ring::HEAD); }
    std::uint32_t tail() const noexcept { return header(ring::TAIL); }
    std::uint32_t len() const noexcept { return tail() - head(); }
    bool empty() const noexcept { return len() == 0; }

    // The word, from the ring's start, where the frame at `position` begins.
    std::uint32_t entry_at(std::uint32_t position) const noexcept { return ring::HEADER_WORDS + (position & (cap_ - 1)) * frame::WORDS; }

    // EAGAIN when full, EMSGSIZE when larger than a slot.
    Errno push(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result,
               std::optional<std::span<const std::uint8_t>> packet = std::nullopt) const noexcept;

    // `write` gets the whole slot payload and answers the length, or none if it
    // did not fit (EMSGSIZE).
    template <class Write> Errno push_in_place(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, Write &&write) const noexcept {
        if (len() >= cap()) return Errno::eagain;
        std::uint32_t slot = slot_at(tail());
        std::optional<std::uint32_t> n = write(std::span<std::uint8_t>(mem_.at(slot), slot_bytes()));
        if (!n || *n > slot_bytes()) return Errno::emsgsize;
        pad(slot, *n);
        publish(tag, cmd, flags, result, slot);
        return Errno::ok;
    }

    // The head's frame, none when empty. Read it, then `advance`.
    std::optional<Frame> peek() const noexcept;

    // Valid until `advance`. EBADMSG when the frame names bytes outside a slot.
    Result<std::span<const std::uint8_t>> packet(const Frame &f) const noexcept;

    void advance() const noexcept;

  private:
    std::uint32_t header(std::uint32_t word) const noexcept { return mem_.word(at_ + 4 * word); }
    void set_header(std::uint32_t word, std::uint32_t w) const noexcept { mem_.set_word(at_ + 4 * word, w); }
    std::uint32_t slot_at(std::uint32_t position) const noexcept;
    void pad(std::uint32_t slot, std::uint32_t n) const noexcept;
    void publish(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::uint32_t packet_at) const noexcept;

    Memory mem_;
    std::uint32_t at_;
    // Read once, when the ring is made: the host's later writes to them are not the ring's.
    std::uint32_t cap_;
    std::uint32_t slot_bytes_;
    std::uint32_t slot_base_;
};

} // namespace xpute
