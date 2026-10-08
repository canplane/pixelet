// cpp/guest/ipc/ring.cpp

#include "ring.hpp"

#include <cstring>

#include "../../kit/status/bug.hpp"

namespace xpute {

static bool power_of_two(std::uint32_t n) noexcept {
    return n != 0 && (n & (n - 1)) == 0;
}

// Payloads are padded to 8 bytes.
static constexpr std::uint64_t padded(std::uint64_t n) noexcept {
    return (n + 7) / 8 * 8;
}

Ring::Ring(Memory mem, std::uint32_t at) noexcept : mem_(mem), at_(at) {
    ensure(power_of_two(capacity()), Errno::einval, capacity(), at);
}

Ring Ring::init(Memory mem, std::uint32_t at, std::uint32_t capacity, std::uint32_t slot_base, std::uint32_t slot_bytes) noexcept {
    ensure(power_of_two(capacity), Errno::einval, capacity);
    ensure(power_of_two(slot_bytes) && slot_bytes % 8 == 0, Errno::einval, slot_bytes);
    std::memset(mem.at(at), 0, bytes(capacity));
    mem.set_word(at + 4 * ring::CAPACITY, capacity);
    mem.set_word(at + 4 * ring::SLOT_BYTES, slot_bytes);
    mem.set_word(at + 4 * ring::SLOT_BASE, slot_base);
    return Ring(mem, at);
}

std::uint32_t Ring::frame_at(std::uint32_t position) const noexcept {
    std::uint32_t slot = position & (capacity() - 1);
    return at_ + 4 * (ring::HEADER_WORDS + slot * frame::WORDS);
}

std::uint32_t Ring::slot_at(std::uint32_t position) const noexcept {
    return header(ring::SLOT_BASE) + (position & (capacity() - 1)) * slot_bytes();
}

void Ring::pad(std::uint32_t slot, std::uint32_t n) const noexcept {
    std::memset(mem_.at(slot + n), 0, padded(n) - n);
}

void Ring::publish(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::uint32_t packet_at) const noexcept {
    std::uint32_t t = tail();
    std::uint32_t f = frame_at(t);
    mem_.set_word(f + 4 * frame::TAG, tag);
    mem_.set_word(f + 4 * frame::CMD, frame::cmd_word(cmd, flags));
    mem_.set_word(f + 4 * frame::RESULT, static_cast<std::uint32_t>(result));
    mem_.set_word(f + 4 * frame::PACKET, packet_at);
    // The frame is whole before the tail says so.
    set_header(ring::TAIL, t + 1);
}

Errno Ring::push(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::optional<std::span<const std::uint8_t>> packet) const noexcept {
    if (len() >= capacity()) return Errno::eagain;
    std::uint32_t packet_at = 0;
    if (packet) {
        if (padded(packet->size()) > slot_bytes()) return Errno::emsgsize;
        packet_at = slot_at(tail());
        auto n = static_cast<std::uint32_t>(packet->size());
        std::memcpy(mem_.at(packet_at), packet->data(), n);
        pad(packet_at, n);
    }
    publish(tag, cmd, flags, result, packet_at);
    return Errno::ok;
}

std::optional<Ring::Message> Ring::peek() const noexcept {
    if (empty()) return std::nullopt;
    std::uint32_t f = frame_at(head());
    std::uint32_t word = mem_.word(f + 4 * frame::CMD);
    return Message{
        .tag = mem_.word(f + 4 * frame::TAG),
        .cmd = frame::cmd_of(word),
        .flags = frame::flags_of(word),
        .result = static_cast<std::int32_t>(mem_.word(f + 4 * frame::RESULT)),
        .packet_at = mem_.word(f + 4 * frame::PACKET),
    };
}

Result<std::span<const std::uint8_t>> Ring::packet(const Message &m) const noexcept {
    if (m.packet_at == 0) return std::span<const std::uint8_t>();
    std::uint32_t base = header(ring::SLOT_BASE), slot = slot_bytes();
    if (m.packet_at < base) return marshal_error(Errno::ebadmsg);
    std::uint64_t off = m.packet_at - base;
    if (off % slot != 0 || off + 16 > slots_bytes(capacity(), slot)) return marshal_error(Errno::ebadmsg);
    // The header, then the payload its second word names, to the word.
    std::uint64_t n = padded(16 + std::uint64_t{mem_.word(m.packet_at + 8)});
    if (n > slot) return marshal_error(Errno::ebadmsg);
    return std::span<const std::uint8_t>(mem_.at(m.packet_at), static_cast<std::size_t>(n));
}

void Ring::advance() const noexcept {
    ensure(!empty(), Errno::enotrecoverable);
    set_header(ring::HEAD, head() + 1);
}

} // namespace xpute
