// cpp/xpute/kit/wire/xtp/encode.cpp

#include "encode.hpp"

#include <algorithm>
#include <cstring>

namespace xpute::xtp {
namespace {

constexpr std::uint64_t word_up(std::uint64_t n) noexcept {
    return (n + WORD_SZ - 1) & ~std::uint64_t{WORD_SZ - 1};
}

} // namespace

PacketWriter::PacketWriter(std::span<std::uint8_t> buf) noexcept : buf_(buf.first(std::min<std::size_t>(buf.size(), MAX_PKT_SZ))) {
    if (buf_.size() < HDR_SZ) err_ = marshal_error(Errno::einval);
    else std::memset(buf_.data(), 0, HDR_SZ);
}

std::optional<std::uint32_t> PacketWriter::spoil(Errno errnum) noexcept {
    if (!err_) err_ = marshal_error(errnum);
    return std::nullopt;
}

std::optional<std::uint32_t> PacketWriter::reserve(std::uint32_t unit, std::uint64_t bytes) noexcept {
    if (err_) return std::nullopt;
    std::uint64_t base = (std::uint64_t{lim_} + unit - 1) & ~std::uint64_t{unit - 1};
    std::uint64_t end = base + bytes;
    if (end > buf_.size()) return spoil(Errno::eoverflow);
    std::memset(buf_.data() + lim_, 0, end - lim_);
    lim_ = static_cast<std::uint32_t>(end);
    return static_cast<std::uint32_t>(base);
}

PacketWriter &PacketWriter::entry(std::optional<std::uint32_t> at, NodeType type) noexcept {
    ensure(frame_.written < frame_.cnt, Errno::enospc, frame_.cnt);
    if (frame_.root) root_type_ = type;
    else if (!err_) set_word(buf_, frame_.base + WORD_SZ + frame_.written * WORD_SZ, at ? *at - frame_.base : 0, type);
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
    if (err_) return std::nullopt;
    std::uint64_t payload = word_up(lim_) + WORD_SZ;
    if (payload > buf_.size()) return spoil(Errno::eoverflow);
    return static_cast<std::uint32_t>(buf_.size() - payload);
}

std::optional<std::uint32_t> PacketWriter::str_commit(std::uint32_t len) noexcept {
    std::uint32_t payload = str_payload();
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
    if (!s) return entry(std::nullopt, type_of(SequenceType::str));
    // Copied the way a string written in place goes in, so the two lay out alike.
    return str_with([&](std::span<char> room) -> std::optional<std::size_t> {
        if (s->size() > room.size()) return std::nullopt;
        std::memcpy(room.data(), s->data(), s->size());
        return s->size();
    });
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

std::optional<std::uint32_t> PacketWriter::branch_open(std::uint32_t cnt) noexcept {
    std::optional<std::uint32_t> at = reserve(WORD_SZ, WORD_SZ + std::uint64_t{cnt} * WORD_SZ);
    if (at) set_word(buf_, *at, cnt, RESERVED);
    return at;
}

// A branch ends on a word, so what follows it starts where the encoder
// starts it.
std::optional<std::uint32_t> PacketWriter::branch_close(std::optional<std::uint32_t> at) noexcept {
    if (!at || err_) return at;
    std::uint64_t end = word_up(lim_);
    if (end > buf_.size()) return spoil(Errno::eoverflow);
    std::memset(buf_.data() + lim_, 0, end - lim_);
    lim_ = static_cast<std::uint32_t>(end);
    return at;
}

PacketWriter &PacketWriter::graft(std::span<const std::uint8_t> pkt) noexcept {
    std::optional<std::uint32_t> at;
    NodeType type = type_of(SpecialType::nil);
    Result<std::uint32_t> magic = pkt.size() < HDR_SZ ? marshal_error(Errno::ebadmsg) : get_len(pkt, 0);
    Result<Desc> d = magic && *magic == MAGIC ? get_desc(pkt, WORD_SZ) : marshal_error(Errno::ebadmsg);
    if (!d) {
        spoil(Errno::ebadmsg);
    } else {
        auto [payload_sz, desc_type] = *d;
        type = desc_type;
        if (payload_sz > pkt.size() - HDR_SZ) {
            spoil(Errno::ebadmsg);
        } else if (payload_sz != 0) {
            Result<std::uint32_t> unit = align_sz(type);
            if (!unit || payload_sz % *unit != 0) {
                spoil(Errno::ebadmsg);
            } else if ((at = reserve(*unit, payload_sz))) {
                std::memcpy(buf_.data() + *at, pkt.data() + HDR_SZ, payload_sz);
            }
        }
    }
    return entry(at, type);
}

Result<std::uint32_t> PacketWriter::finish() noexcept {
    ensure(frame_.root && frame_.written == frame_.cnt, Errno::enotrecoverable, frame_.cnt, frame_.written);
    if (err_) return *err_;
    std::uint64_t end = word_up(lim_);
    if (end > buf_.size()) return marshal_error(Errno::eoverflow);
    std::memset(buf_.data() + lim_, 0, end - lim_);
    set_word(buf_, 0, MAGIC, RESERVED);
    set_word(buf_, WORD_SZ, lim_ - HDR_SZ, root_type_);
    return static_cast<std::uint32_t>(end);
}

} // namespace xpute::xtp
