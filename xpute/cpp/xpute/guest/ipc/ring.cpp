// cpp/xpute/guest/ipc/ring.cpp

#include "ring.hpp"

#include <cstring>

#include "xpute/kit/status/bug.hpp"
#include "xpute/kit/wire/xtp/spec.hpp"

namespace xpute {

static bool power_of_two(std::uint32_t n) noexcept {
    return n != 0 && (n & (n - 1)) == 0;
}

// Payloads are padded to XTP's word (`WORD_SZ`).
static constexpr std::uint64_t padded(std::uint64_t n) noexcept {
    return (n + xtp::WORD_SZ - 1) / xtp::WORD_SZ * xtp::WORD_SZ;
}

Ring::Ring(Memory mem, std::uint32_t at) noexcept
    : mem_(mem), at_(at), cap_(header(ring::CAP)), slot_bytes_(header(ring::SLOT_BYTES)), slot_base_(header(ring::SLOT_BASE)) {
    ensure(power_of_two(cap_), Errno::einval, cap_, at);
}

Ring Ring::init(Memory mem, std::uint32_t at, std::uint32_t cap, std::uint32_t slot_base, std::uint32_t slot_bytes) noexcept {
    ensure(power_of_two(cap), Errno::einval, cap);
    ensure(power_of_two(slot_bytes) && slot_bytes % xtp::WORD_SZ == 0, Errno::einval, slot_bytes);
    std::memset(mem.at(at), 0, ring_bytes(cap));
    mem.set_word(at + 4 * ring::CAP, cap);
    mem.set_word(at + 4 * ring::SLOT_BYTES, slot_bytes);
    mem.set_word(at + 4 * ring::SLOT_BASE, slot_base);
    return Ring(mem, at);
}

std::uint32_t Ring::slot_at(std::uint32_t pos) const noexcept {
    return slot_base_ + (pos & (cap_ - 1)) * slot_bytes_;
}

void Ring::pad(std::uint32_t slot, std::uint32_t n) const noexcept {
    std::memset(mem_.at(slot + n), 0, padded(n) - n);
}

void Ring::publish(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::uint32_t pkt_at) const noexcept {
    std::uint32_t t = tail();
    std::uint32_t f = at_ + 4 * entry_at(t);
    mem_.set_word(f + 4 * frame::TAG, tag);
    mem_.set_word(f + 4 * frame::CMD, frame::cmd_word(cmd, flags));
    mem_.set_word(f + 4 * frame::RESULT, static_cast<std::uint32_t>(result));
    mem_.set_word(f + 4 * frame::PKT, pkt_at);
    // The frame is whole before the tail says so.
    set_header(ring::TAIL, t + 1);
}

Errno Ring::push(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result, std::optional<std::span<const std::uint8_t>> pkt) const noexcept {
    if (len() >= cap()) return Errno::eagain;
    std::uint32_t pkt_at = 0;
    if (pkt) {
        if (padded(pkt->size()) > slot_bytes()) return Errno::emsgsize;
        pkt_at = slot_at(tail());
        auto n = static_cast<std::uint32_t>(pkt->size());
        std::memcpy(mem_.at(pkt_at), pkt->data(), n);
        pad(pkt_at, n);
    }
    publish(tag, cmd, flags, result, pkt_at);
    return Errno::ok;
}

std::optional<Frame> Ring::peek() const noexcept {
    if (empty()) return std::nullopt;
    std::uint32_t f = at_ + 4 * entry_at(head());
    std::uint32_t word = mem_.word(f + 4 * frame::CMD);
    return Frame{
        .tag = mem_.word(f + 4 * frame::TAG),
        .cmd = frame::cmd_of(word),
        .flags = frame::flags_of(word),
        .result = static_cast<std::int32_t>(mem_.word(f + 4 * frame::RESULT)),
        .pkt_at = mem_.word(f + 4 * frame::PKT),
    };
}

Result<std::span<const std::uint8_t>> Ring::pkt(const Frame &f) const noexcept {
    if (f.pkt_at == 0) return std::span<const std::uint8_t>();
    std::uint32_t base = slot_base_, slot = slot_bytes_;
    if (f.pkt_at < base) return marshal_error(Errno::ebadmsg);
    std::uint64_t off = f.pkt_at - base;
    if (off % slot != 0 || off + xtp::HDR_SZ > slots_bytes(cap(), slot)) return marshal_error(Errno::ebadmsg);
    // The header, then the payload its second word names, to the word.
    std::uint64_t n = padded(xtp::HDR_SZ + std::uint64_t{mem_.word(f.pkt_at + xtp::WORD_SZ)});
    if (n > slot) return marshal_error(Errno::ebadmsg);
    return std::span<const std::uint8_t>(mem_.at(f.pkt_at), static_cast<std::size_t>(n));
}

void Ring::advance() const noexcept {
    ensure(!empty(), Errno::enotrecoverable);
    set_header(ring::HEAD, head() + 1);
}

} // namespace xpute
