// cpp/xpute/runtime/ipc/ring.cpp

#include "ring.hpp"

#include <cstring>

#include "frame.hpp"

namespace xpute {

Ring Ring::init(Memory mem, std::uint32_t at, std::uint32_t capacity, std::uint32_t slot_base, std::uint32_t slot_bytes) noexcept {
    std::memset(mem.at(at), 0, bytes(capacity));
    Ring r(mem, at);
    r.set_header(CAPACITY, capacity);
    r.set_header(SLOT_BYTES, slot_bytes);
    r.set_header(SLOT_BASE, slot_base);
    return r;
}

std::uint32_t Ring::frame_at(std::uint32_t position) const noexcept {
    std::uint32_t slot = position & (capacity() - 1);
    return at_ + 4 * (HEADER_WORDS + slot * frame::WORDS);
}

std::optional<Ring::Message> Ring::peek() const noexcept {
    std::uint32_t head = header(HEAD);
    if (head == header(TAIL)) return std::nullopt;
    std::uint32_t f = frame_at(head);
    std::uint32_t word = mem_.word(f + 4 * frame::CMD);
    std::uint32_t packet = mem_.word(f + 4 * frame::PACKET);
    return Message{
        .tag = mem_.word(f + 4 * frame::TAG),
        .cmd = frame::cmd_of(word),
        .flags = frame::flags_of(word),
        .packet = packet != 0 ? std::span<const std::uint8_t>(mem_.at(packet), header(SLOT_BYTES)) : std::span<const std::uint8_t>(),
    };
}

void Ring::advance() const noexcept {
    set_header(HEAD, header(HEAD) + 1);
}

Errno Ring::push(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept {
    std::uint32_t tail = header(TAIL);
    if (tail - header(HEAD) == capacity()) return Errno::eagain;
    std::uint32_t f = frame_at(tail);
    mem_.set_word(f + 4 * frame::TAG, tag);
    mem_.set_word(f + 4 * frame::CMD, frame::cmd_word(cmd, flags));
    mem_.set_word(f + 4 * frame::RESULT, static_cast<std::uint32_t>(result));
    mem_.set_word(f + 4 * frame::PACKET, 0);
    // The frame is whole before the tail says so.
    set_header(TAIL, tail + 1);
    return Errno::ok;
}

} // namespace xpute
