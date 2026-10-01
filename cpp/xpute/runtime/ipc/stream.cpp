// cpp/xpute/runtime/ipc/stream.cpp

#include "stream.hpp"

#include <cstdlib>

namespace xpute {

Stream::Stream(Memory mem, std::uint32_t at, std::uint32_t words) noexcept : mem_(mem), at_(at), words_(words) {
    mem_.set_word(at_, 0);
}

void Stream::record(std::uint32_t op, std::span<const std::uint32_t> words) noexcept {
    auto n = static_cast<std::uint32_t>(words.size());
    // The END word, then what is written, then this record.
    if (1 + len_ + 2 + n > words_) std::abort();
    std::uint32_t off = at_ + 4 * (1 + len_);
    mem_.set_word(off, op);
    mem_.set_word(off + 4, n);
    for (std::uint32_t i = 0; i < n; i++) mem_.set_word(off + 8 + 4 * i, words[i]);
    len_ += 2 + n;
}

void Stream::publish() noexcept {
    mem_.set_word(at_, len_);
    len_ = 0;
}

} // namespace xpute
