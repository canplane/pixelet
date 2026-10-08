// cpp/guest/ipc/stream.cpp

#include "stream.hpp"

#include <cstring>

#include "../../kit/status/bug.hpp"

namespace xpute {

Stream::Stream(Memory mem, std::uint32_t at, std::uint32_t words, std::uint32_t head) noexcept : mem_(mem), at_(at), words_(words), head_(head) {
    ensure(head > END && head < words, Errno::einval, head, words);
    mem_.set_word(at_ + WORD * END, 0);
}

std::uint32_t Stream::open(std::uint32_t op, std::uint32_t n, std::source_location from) noexcept {
    std::uint32_t at = head_ + len_;
    ensure(std::uint64_t{at} + 2 + n <= words_, Errno::enospc, words_ * WORD, 0, from);
    len_ += 2 + n;
    std::uint32_t off = at_ + WORD * at;
    mem_.set_word(off, op);
    mem_.set_word(off + WORD, n);
    return off + 2 * WORD;
}

void Stream::record(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail,
                    std::optional<std::span<const std::uint8_t>> bytes, std::source_location from) noexcept {
    auto a = static_cast<std::uint32_t>(args.size()), t = static_cast<std::uint32_t>(tail.size());
    if (mark_ && (spilled_ || !fits(a + t, bytes ? std::optional<std::uint32_t>{static_cast<std::uint32_t>(bytes->size())} : std::nullopt, 0))) {
        spilled_ = true;
        return;
    }
    std::uint32_t byte_words = static_cast<std::uint32_t>(record_words(0, bytes ? std::optional<std::uint64_t>{bytes->size()} : std::nullopt) - 2);
    std::uint32_t w = open(op, a + t + byte_words, from);
    for (std::uint32_t i = 0; i < a; i++) mem_.set_word(w + WORD * i, args[i]);
    for (std::uint32_t i = 0; i < t; i++) mem_.set_word(w + WORD * (a + i), tail[i]);
    if (bytes) {
        std::uint32_t k = w + WORD * (a + t);
        mem_.set_word(k, static_cast<std::uint32_t>(bytes->size()));
        if (byte_words > 1) mem_.set_word(k + WORD * (byte_words - 1), 0);
        std::memcpy(mem_.at(k + WORD), bytes->data(), bytes->size());
    }
}

void Stream::publish() noexcept {
    mem_.set_word(at_ + WORD * END, len_);
    len_ = 0;
}

} // namespace xpute
