// cpp/xpute/guest/ipc/stream.hpp

// A turn's calls from guest to host as records in one range. The host runs
// them after the turn returns and before the next, so a record may name
// memory the host writes and the guest reads next turn.

#pragma once

#include <cstdint>
#include <optional>
#include <source_location>
#include <span>

#include "xpute/guest/mem.hpp"

namespace xpute {

inline constexpr std::uint32_t WORD = sizeof(std::uint32_t);

// A record's words: its op and count, `args`, and `bytes` as their count then words.
constexpr std::uint64_t record_words(std::uint64_t args, std::optional<std::uint64_t> bytes) noexcept {
    return 2 + args + (bytes ? 1 + (*bytes + WORD - 1) / WORD : 0);
}

// A record's size as a stream's room is counted.
constexpr std::uint64_t record_bytes(std::uint64_t args, std::optional<std::uint64_t> bytes) noexcept {
    return record_words(args, bytes) * WORD;
}

class Stream {
  public:
    static constexpr std::uint32_t END = 0;

    Stream(Memory mem, std::uint32_t at, std::uint32_t words, std::uint32_t head = 1) noexcept;

    std::uint32_t head_at() const noexcept { return at_; }

    Memory mem() const noexcept { return mem_; }

    std::uint32_t left() const noexcept { return (words_ - head_ - len_) * WORD; }

    // Whether records of `bytes` (`record_bytes`, gpu/encode's `call::*_bytes`)
    // fit with `leave` bytes left after them.
    bool fits(std::uint64_t bytes, std::uint32_t leave) const noexcept { return std::uint64_t{left()} >= bytes + leave; }

    // Answers where the `n` words begin. Past the range is a bug, reported at
    // the caller.
    std::uint32_t open(std::uint32_t op, std::uint32_t n, std::source_location at = std::source_location::current()) noexcept;

    // A record of `n` words for its writer to fill; none where an open
    // transaction no longer fits it, which drops the transaction.
    std::optional<std::uint32_t> reserve(std::uint32_t op, std::uint32_t n, std::source_location at = std::source_location::current()) noexcept;

    void record(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail = {},
                std::optional<std::span<const std::uint8_t>> bytes = std::nullopt, std::source_location at = std::source_location::current()) noexcept;

    // A record laid past the last the turn has recorded and never published,
    // for a hostcall: its offset in the memory, or none where it does not
    // fit. The next record written takes its words.
    std::optional<std::uint32_t> past(std::uint32_t op, std::span<const std::uint32_t> args,
                                      std::optional<std::span<const std::uint8_t>> bytes = std::nullopt) noexcept;

    void publish() noexcept;

    // The records from here to `commit` are one: one that does not fit drops
    // them all, and those after it, where outside a transaction it is a bug.
    void begin() noexcept { mark_ = {Mark::open, len_}; }

    // False where the transaction did not fit: the stream is as at `begin`.
    bool commit() noexcept {
        bool fit = mark_.state != Mark::spilled;
        if (!fit) len_ = mark_.at;
        mark_ = {};
        return fit;
    }

  private:
    // A record's words past its op and count, from `w`: `args`, `tail`, and
    // `bytes` as their count then words.
    void fill(std::uint32_t w, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail,
              std::optional<std::span<const std::uint8_t>> bytes) noexcept;

    Memory mem_;
    std::uint32_t at_;
    std::uint32_t words_;
    std::uint32_t head_;
    std::uint32_t len_ = 0;
    // Whether a transaction is open, where it began, and whether a record
    // that did not fit has dropped it.
    struct Mark {
        enum State { off, open, spilled } state = off;
        std::uint32_t at = 0;
    };
    Mark mark_;
};

} // namespace xpute
