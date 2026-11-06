// cpp/xpute/core/ipc/ring.hpp

// A ring of frames, laid out by spec/ipc/ring.json.
//
// A ring's two sides may take turns on one thread, as a page's runtime and its
// module do, or run at once on two, as a native runtime and a core on its own
// thread do. The same rules, io_uring's, hold either way:
//
// - R1. Each position word has one writer: TAIL is the producer's and HEAD
//   the consumer's.
// - R2. A position word is stored with release and loaded with acquire. The
//   producer writes a frame and its slot, then stores TAIL past them; the
//   consumer loads TAIL before it reads them, and stores HEAD past a frame
//   once it is done with the frame and its slot; the producer loads HEAD
//   before it writes into that frame or slot again.
// - R3. Memory a frame names by its address is the receiver's from the
//   moment the frame is published until the receiver stores HEAD past it.
//   The sender writes it again only once it has loaded a HEAD past that
//   frame (`is_consumed`).
//
// The frames' and slots' own words are plain memory, ordered by the position
// words alone. Where a target has no threads, as WebAssembly without its
// atomics feature, the atomic accesses are its plain loads and stores.

#pragma once

#include <atomic>
#include <cstdint>
#include <optional>
#include <span>

#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/core/mem.hpp"
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
    // A cap that is not a power of two, or a ring not on a word, is a bug.
    Ring(Memory mem, std::uint32_t at) noexcept;

    static Ring init(Memory mem, std::uint32_t at, std::uint32_t cap, std::uint32_t slot_base, std::uint32_t slot_bytes) noexcept;

    const Memory &mem() const noexcept { return mem_; }
    std::uint32_t cap() const noexcept { return cap_; }
    std::uint32_t slot_bytes() const noexcept { return slot_bytes_; }
    std::uint32_t head() const noexcept { return pos(ring::HEAD).load(std::memory_order_acquire); }
    std::uint32_t tail() const noexcept { return pos(ring::TAIL).load(std::memory_order_acquire); }
    std::uint32_t len() const noexcept { return tail() - head(); }
    bool empty() const noexcept { return len() == 0; }

    // Whether the consumer has stored its head past the frame published at
    // `pos`: what that frame named is the producer's again (R3). The
    // producer's to ask.
    bool is_consumed(std::uint32_t pos) const noexcept {
        std::uint32_t t = tail();
        return t - head() < t - pos;
    }

    // The word, from the ring's start, where the frame at `position` begins.
    std::uint32_t entry_at(std::uint32_t pos) const noexcept { return ring::HEADER_WORDS + (pos & (cap_ - 1)) * frame::WORDS; }

    // EAGAIN when full, EMSGSIZE when larger than a slot.
    Errno push(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result,
               std::optional<std::span<const std::uint8_t>> pkt = std::nullopt) const noexcept;

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

    // `push_in_place`'s frame laid at the tail and not published: where its
    // words begin in the memory; EAGAIN where the ring is full, EMSGSIZE where
    // the packet did not fit. The next push takes its place.
    template <class Write> Result<std::uint32_t> stage_in_place(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, Write &&write) const noexcept {
        if (len() >= cap()) return make_error(ErrorClass::fault, Errno::eagain);
        std::uint32_t slot = slot_at(tail());
        std::optional<std::uint32_t> n = write(std::span<std::uint8_t>(mem_.at(slot), slot_bytes()));
        if (!n || *n > slot_bytes()) return make_error(ErrorClass::fault, Errno::emsgsize);
        pad(slot, *n);
        return entry(tag, cmd, flags, 0, slot);
    }


    // The head's frame, none when empty. Read it, then `advance`.
    std::optional<Frame> peek() const noexcept;

    // Valid until `advance`. EBADMSG when the frame names bytes outside a slot.
    Result<std::span<const std::uint8_t>> pkt(const Frame &f) const noexcept;

    void advance() const noexcept;

  private:
    std::uint32_t header(std::uint32_t word) const noexcept { return mem_.word(at_ + 4 * word); }
    // A position word (HEAD, TAIL), only ever touched as an atomic (R2).
    std::atomic_ref<std::uint32_t> pos(std::uint32_t word) const noexcept {
        return std::atomic_ref<std::uint32_t>(*reinterpret_cast<std::uint32_t *>(mem_.at(at_ + 4 * word)));
    }
    std::uint32_t slot_at(std::uint32_t pos) const noexcept;
    void pad(std::uint32_t slot, std::uint32_t n) const noexcept;
    void publish(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::uint32_t pkt_at) const noexcept;
    // The frame's words at the tail's entry, unpublished: where they begin in the memory.
    std::uint32_t entry(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::uint32_t pkt_at) const noexcept;

    Memory mem_;
    std::uint32_t at_;
    // Read once, when the ring is made: the runtime's later writes to them are not the ring's.
    std::uint32_t cap_;
    std::uint32_t slot_bytes_;
    std::uint32_t slot_base_;
};

} // namespace xpute
