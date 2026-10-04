// cpp/xpute/core/wire/xtp/encode.cpp

#include "encode.hpp"

#include <algorithm>
#include <cstring>

namespace xpute::xtp {
namespace {

constexpr std::uint64_t word_up(std::uint64_t n) noexcept {
    return (n + WORD_SZ - 1) & ~std::uint64_t{WORD_SZ - 1};
}

// Two units for a character past the basic plane, which UTF-8 encodes in four bytes.
std::uint32_t utf16_units(std::span<const std::uint8_t> utf8) noexcept {
    std::uint32_t units = 0;
    for (std::uint8_t b : utf8) {
        if ((b & 0xc0) != 0x80) units++;
        if (b >= 0xf0) units++;
    }
    return units;
}

} // namespace

PacketWriter::PacketWriter(std::span<std::uint8_t> buf) noexcept : buf_(buf.first(std::min<std::size_t>(buf.size(), MAX_PKT_SZ))) {
    if (buf_.size() < HDR_SZ) error_ = marshal_error(Errno::einval);
    else std::memset(buf_.data(), 0, HDR_SZ);
}

std::optional<std::uint32_t> PacketWriter::spoil(Errno code) noexcept {
    if (!error_) error_ = marshal_error(code);
    return std::nullopt;
}

std::optional<std::uint32_t> PacketWriter::reserve(std::uint32_t unit, std::uint64_t bytes) noexcept {
    if (error_) return std::nullopt;
    std::uint64_t base = (std::uint64_t{lim_} + unit - 1) & ~std::uint64_t{unit - 1};
    std::uint64_t end = base + bytes;
    if (end > buf_.size()) return spoil(Errno::eoverflow);
    std::memset(buf_.data() + lim_, 0, end - lim_);
    lim_ = static_cast<std::uint32_t>(end);
    return static_cast<std::uint32_t>(base);
}

PacketWriter &PacketWriter::entry(std::optional<std::uint32_t> at, NodeType type) noexcept {
    ensure(frame_.written < frame_.count, Errno::enospc, frame_.count);
    if (frame_.root) root_type_ = type;
    else if (!error_) set_word(buf_, frame_.base + WORD_SZ + frame_.written * WORD_SZ, at ? *at - frame_.base : 0, type);
    frame_.written++;
    return *this;
}

std::optional<std::uint32_t> PacketWriter::seq(std::uint32_t len, std::uint64_t bytes) noexcept {
    std::optional<std::uint32_t> at = reserve(WORD_SZ, WORD_SZ + word_up(bytes));
    if (at) set_word(buf_, *at, len, RESERVED);
    return at;
}

PacketWriter &PacketWriter::bitset(std::optional<std::span<const std::uint8_t>> bits) noexcept {
    std::optional<std::uint32_t> at;
    if (bits) {
        auto len = static_cast<std::uint32_t>(bits->size());
        at = seq(len, (std::uint64_t{len} + 7) >> 3);
        if (at) {
            std::uint8_t *payload = buf_.data() + *at + WORD_SZ;
            for (std::uint32_t i = 0; i < len; i++) {
                if ((*bits)[i] != 0) payload[i >> 3] |= static_cast<std::uint8_t>(1u << (i & 7));
            }
        }
    }
    return entry(at, type_of(SequenceType::bitset));
}

std::uint32_t PacketWriter::str_payload() const noexcept {
    return static_cast<std::uint32_t>(word_up(lim_) + WORD_SZ);
}

std::optional<std::uint32_t> PacketWriter::str_room() noexcept {
    if (error_) return std::nullopt;
    std::uint64_t payload = word_up(lim_) + WORD_SZ;
    if (payload > buf_.size()) return spoil(Errno::eoverflow);
    return static_cast<std::uint32_t>(buf_.size() - payload);
}

std::optional<std::uint32_t> PacketWriter::str_commit(std::uint32_t len) noexcept {
    std::uint32_t payload = str_payload();
    std::uint32_t room = static_cast<std::uint32_t>(buf_.size() - payload);
    if (utf16_units(buf_.subspan(payload, len)) > room >> 2) return spoil(Errno::eoverflow);
    std::uint64_t end = word_up(std::uint64_t{payload} + len + 1);
    if (end > buf_.size()) return spoil(Errno::eoverflow);
    std::uint32_t base = payload - WORD_SZ;
    std::memset(buf_.data() + lim_, 0, base - lim_);
    std::memset(buf_.data() + payload + len, 0, end - (payload + len));
    set_word(buf_, base, len, RESERVED);
    lim_ = static_cast<std::uint32_t>(end);
    return base;
}

PacketWriter &PacketWriter::str(std::optional<std::string_view> s) noexcept {
    std::optional<std::uint32_t> at;
    if (s) {
        if (std::optional<std::uint32_t> room = str_room()) {
            if (s->size() > *room) {
                spoil(Errno::eoverflow);
            } else {
                std::memcpy(buf_.data() + str_payload(), s->data(), s->size());
                at = str_commit(static_cast<std::uint32_t>(s->size()));
            }
        }
    }
    return entry(at, type_of(SequenceType::str));
}

PacketWriter &PacketWriter::strs(std::optional<std::span<const std::string_view>> items) noexcept {
    std::optional<std::uint32_t> at;
    if (items) {
        auto n = static_cast<std::uint32_t>(items->size());
        std::uint64_t lane = 4 * (std::uint64_t{n} + 1);
        std::uint64_t blob = 0;
        for (std::string_view s : *items) blob += s.size();
        at = seq(n, lane + blob);
        if (at) {
            std::uint8_t *payload = buf_.data() + *at + WORD_SZ;
            std::uint32_t off = 0;
            store<std::uint32_t>(payload, off);
            for (std::uint32_t i = 0; i < n; i++) {
                std::string_view s = (*items)[i];
                std::memcpy(payload + lane + off, s.data(), s.size());
                off += static_cast<std::uint32_t>(s.size());
                store<std::uint32_t>(payload + 4 * (i + 1), off);
            }
        }
    }
    return entry(at, type_of(SequenceType::strs));
}

std::optional<std::uint32_t> PacketWriter::branch_open(std::uint32_t count) noexcept {
    std::optional<std::uint32_t> at = reserve(WORD_SZ, WORD_SZ + std::uint64_t{count} * WORD_SZ);
    if (at) set_word(buf_, *at, count, RESERVED);
    return at;
}

// A branch ends on a word, so what follows it starts where the encoder
// starts it.
std::optional<std::uint32_t> PacketWriter::branch_close(std::optional<std::uint32_t> at) noexcept {
    if (!at || error_) return at;
    std::uint64_t end = word_up(lim_);
    if (end > buf_.size()) return spoil(Errno::eoverflow);
    std::memset(buf_.data() + lim_, 0, end - lim_);
    lim_ = static_cast<std::uint32_t>(end);
    return at;
}

PacketWriter &PacketWriter::graft(std::span<const std::uint8_t> packet) noexcept {
    std::optional<std::uint32_t> at;
    NodeType type = type_of(SpecialType::nil);
    if (packet.size() < HDR_SZ || get_word(packet, 0)[0] != MAGIC) {
        spoil(Errno::ebadmsg);
    } else {
        auto [payload_sz, desc] = get_word(packet, WORD_SZ);
        type = static_cast<NodeType>(desc & DESC_TYPE_MASK);
        if (payload_sz > packet.size() - HDR_SZ) {
            spoil(Errno::ebadmsg);
        } else if (payload_sz != 0) {
            Result<std::uint32_t> unit = align_sz(type);
            if (!unit || payload_sz % *unit != 0) {
                spoil(Errno::ebadmsg);
            } else if ((at = reserve(*unit, payload_sz))) {
                std::memcpy(buf_.data() + *at, packet.data() + HDR_SZ, payload_sz);
            }
        }
    }
    return entry(at, type);
}

Result<std::uint32_t> PacketWriter::finish() noexcept {
    ensure(frame_.root && frame_.written == frame_.count, Errno::enotrecoverable, frame_.count, frame_.written);
    if (error_) return *error_;
    std::uint64_t end = word_up(lim_);
    if (end > buf_.size()) return marshal_error(Errno::eoverflow);
    std::memset(buf_.data() + lim_, 0, end - lim_);
    set_word(buf_, 0, MAGIC, RESERVED);
    set_word(buf_, WORD_SZ, lim_ - HDR_SZ, root_type_);
    return static_cast<std::uint32_t>(end);
}

} // namespace xpute::xtp
