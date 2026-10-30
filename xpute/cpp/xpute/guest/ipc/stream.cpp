// cpp/xpute/guest/ipc/stream.cpp

#include "stream.hpp"

#include <cstring>

#include "xpute/kit/status/bug.hpp"

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

std::optional<std::uint32_t> Stream::reserve(std::uint32_t op, std::uint32_t n, std::source_location from) noexcept {
    if (mark_.state == Mark::spilled) return std::nullopt;
    if (mark_.state == Mark::open && std::uint64_t{left()} < (std::uint64_t{2} + n) * WORD) {
        mark_.state = Mark::spilled;
        return std::nullopt;
    }
    return open(op, n, from);
}

static std::uint32_t byte_words_of(std::optional<std::span<const std::uint8_t>> bytes) noexcept {
    return static_cast<std::uint32_t>(record_words(0, bytes ? std::optional<std::uint64_t>{bytes->size()} : std::nullopt) - 2);
}

void Stream::record(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail,
                    std::optional<std::span<const std::uint8_t>> bytes, std::source_location from) noexcept {
    std::optional<std::uint32_t> w = reserve(op, static_cast<std::uint32_t>(args.size() + tail.size()) + byte_words_of(bytes), from);
    if (!w) return;
    fill(*w, args, tail, bytes);
}

std::optional<std::uint32_t> Stream::past(std::uint32_t op, std::span<const std::uint32_t> args,
                                          std::optional<std::span<const std::uint8_t>> bytes) noexcept {
    std::uint32_t n = static_cast<std::uint32_t>(args.size()) + byte_words_of(bytes);
    std::uint32_t at = head_ + len_;
    if (std::uint64_t{at} + 2 + n > words_) return std::nullopt;
    std::uint32_t off = at_ + WORD * at;
    mem_.set_word(off, op);
    mem_.set_word(off + WORD, n);
    fill(off + 2 * WORD, args, {}, bytes);
    return off;
}

void Stream::fill(std::uint32_t w, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail,
                  std::optional<std::span<const std::uint8_t>> bytes) noexcept {
    auto a = static_cast<std::uint32_t>(args.size()), t = static_cast<std::uint32_t>(tail.size());
    for (std::uint32_t i = 0; i < a; i++) mem_.set_word(w + WORD * i, args[i]);
    for (std::uint32_t i = 0; i < t; i++) mem_.set_word(w + WORD * (a + i), tail[i]);
    if (bytes) {
        std::uint32_t byte_words = byte_words_of(bytes);
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
