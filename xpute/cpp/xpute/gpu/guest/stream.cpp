// cpp/xpute/gpu/guest/stream.cpp

#include "stream.hpp"

#include <algorithm>
#include <cstring>

#include "xpute/kit/status/bug.hpp"
#include "xpute/gpu/guest/abi/gpu.hpp"
#include "xpute/guest/heap.hpp"

namespace xpute::gpu {

Stream::Stream(std::size_t cap) : buf_(std::pmr::vector<std::uint32_t>(&guest::heap())), cap_(cap) {
    std::get<0>(buf_).reserve(cap);
}

std::size_t Stream::len() const noexcept {
    const auto *words = std::get_if<0>(&buf_);
    return words != nullptr ? words->size() : 0;
}

bool Stream::is_free() {
    if (Run *run = std::get_if<1>(&buf_); run != nullptr && run->done()) {
        std::pmr::vector<std::uint32_t> words = std::move(run->get().first);
        words.clear();
        buf_ = std::move(words);
    }
    return buf_.index() == 0;
}

void Stream::flush() {
    if (len() == 0 || !is_free()) return;
    std::pmr::vector<std::uint32_t> words = std::move(std::get<0>(buf_));
    buf_.emplace<1>(abi::gpu::gpu::run(std::move(words)));
}

std::span<std::uint32_t> Stream::open(std::uint32_t op, std::size_t n, std::source_location at) {
    std::size_t off = len();
    ensure(off + 2 + n <= cap_, Errno::enospc, cap_ * WORD, 0, at);
    ensure(is_free(), Errno::ebusy, op, 0, at);
    auto &words = std::get<0>(buf_);
    // Within the capacity the stream was made with, so nothing moves.
    words.resize(off + 2 + n, 0);
    words[off] = op;
    words[off + 1] = static_cast<std::uint32_t>(n);
    return std::span<std::uint32_t>(words).subspan(off + 2);
}

bool Stream::commit() noexcept {
    Mark mark = std::exchange(mark_, Mark{});
    if (mark.state != Mark::spilled) return true;
    if (auto *words = std::get_if<0>(&buf_)) words->resize(mark.at);
    return false;
}

std::optional<std::span<std::uint32_t>> Stream::reserve(std::uint32_t op, std::size_t n, std::source_location at) {
    if (mark_.state == Mark::spilled) return std::nullopt;
    if (mark_.state == Mark::open && left() < (2 + n) * WORD) {
        mark_.state = Mark::spilled;
        return std::nullopt;
    }
    return open(op, n, at);
}

void Stream::record(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail,
                    std::optional<std::span<const std::uint8_t>> bytes, std::source_location at) {
    std::size_t n = rec_words(args.size() + tail.size(), bytes ? std::optional<std::size_t>(bytes->size()) : std::nullopt) - 2;
    std::optional<std::span<std::uint32_t>> w = reserve(op, n, at);
    if (!w) return;
    // `args`, `tail`, then `bytes` as their count followed by the bytes in
    // words.
    std::copy(args.begin(), args.end(), w->begin());
    std::copy(tail.begin(), tail.end(), w->begin() + args.size());
    if (bytes) {
        std::size_t k = args.size() + tail.size();
        (*w)[k] = static_cast<std::uint32_t>(bytes->size());
        std::span<std::uint32_t> dst = w->subspan(k + 1);
        if (!dst.empty()) dst.back() = 0;
        if (!bytes->empty()) std::memcpy(dst.data(), bytes->data(), bytes->size());
    }
}

} // namespace xpute::gpu
