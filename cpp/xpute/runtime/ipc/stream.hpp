// cpp/xpute/runtime/ipc/stream.hpp

// A turn's calls from guest to host as records in one range. The host runs
// them after the turn returns and before the next, so a record may name
// memory the host writes and the guest reads next turn.

#pragma once

#include <cstdint>
#include <optional>
#include <span>

#include "../mem/memory.hpp"

namespace xpute {

// A record's words: its op and count, `args`, and `bytes` as their count then words.
constexpr std::uint64_t record_words(std::uint64_t args, std::optional<std::uint64_t> bytes) noexcept {
    return 2 + args + (bytes ? 1 + (*bytes + 3) / 4 : 0);
}

class Stream {
  public:
    static constexpr std::uint32_t END = 0;

    Stream(Memory mem, std::uint32_t at, std::uint32_t words, std::uint32_t head = 1) noexcept;

    std::uint32_t head_at() const noexcept { return at_; }

    std::uint32_t left() const noexcept { return (words_ - head_ - len_) * 4; }

    // Whether a record of `args` words and `bytes` would fit whole with
    // `reserve` bytes still left after it.
    bool fits(std::uint32_t args, std::optional<std::uint32_t> bytes, std::uint32_t reserve) const noexcept {
        return std::uint64_t{left()} >= record_words(args, bytes) * 4 + reserve;
    }

    // Answers where the `n` words begin. Past the range is a bug: the guest asked
    // more of a turn than it sized the range for.
    std::uint32_t open(std::uint32_t op, std::uint32_t n) noexcept;

    void record(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail = {},
                std::optional<std::span<const std::uint8_t>> bytes = std::nullopt) noexcept;

    // The next turn writes from the start.
    void publish() noexcept;

    // The records from here to `commit` are one: one that does not fit drops
    // them all, and those after it, where outside a transaction it is a bug.
    void begin() noexcept {
        mark_ = len_;
        spilled_ = false;
    }

    // False where the transaction did not fit: the stream is as at `begin`.
    bool commit() noexcept {
        bool fit = !spilled_;
        if (mark_ && !fit) len_ = *mark_;
        mark_.reset();
        spilled_ = false;
        return fit;
    }

  private:
    Memory mem_;
    std::uint32_t at_;
    std::uint32_t words_;
    std::uint32_t head_;
    std::uint32_t len_ = 0;
    std::optional<std::uint32_t> mark_;
    bool spilled_ = false;
};

} // namespace xpute
