// cpp/xpute/runtime/ipc/stream.cpp

#include "stream.hpp"

#include <cstring>

#include "../../core/status/bug.hpp"

namespace xpute {

Stream::Stream(Memory mem, std::uint32_t at, std::uint32_t words, std::uint32_t head) noexcept : mem_(mem), at_(at), words_(words), head_(head) {
    ensure(head > END && head < words, Errno::einval, head, words);
    mem_.set_word(at_ + 4 * END, 0);
}

std::uint32_t Stream::open(std::uint32_t op, std::uint32_t n) noexcept {
    std::uint32_t at = head_ + len_;
    ensure(std::uint64_t{at} + 2 + n <= words_, Errno::enospc, words_ * 4);
    len_ += 2 + n;
    std::uint32_t off = at_ + 4 * at;
    mem_.set_word(off, op);
    mem_.set_word(off + 4, n);
    return off + 8;
}

void Stream::record(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail,
                    std::optional<std::span<const std::uint8_t>> bytes) noexcept {
    auto a = static_cast<std::uint32_t>(args.size()), t = static_cast<std::uint32_t>(tail.size());
    std::uint32_t byte_words = bytes ? 1 + static_cast<std::uint32_t>((bytes->size() + 3) / 4) : 0;
    std::uint32_t w = open(op, a + t + byte_words);
    for (std::uint32_t i = 0; i < a; i++) mem_.set_word(w + 4 * i, args[i]);
    for (std::uint32_t i = 0; i < t; i++) mem_.set_word(w + 4 * (a + i), tail[i]);
    if (bytes) {
        std::uint32_t k = w + 4 * (a + t);
        mem_.set_word(k, static_cast<std::uint32_t>(bytes->size()));
        if (byte_words > 1) mem_.set_word(k + 4 * (byte_words - 1), 0);
        std::memcpy(mem_.at(k + 4), bytes->data(), bytes->size());
    }
}

void Stream::publish() noexcept {
    mem_.set_word(at_ + 4 * END, len_);
    len_ = 0;
}

} // namespace xpute
