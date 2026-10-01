// cpp/xpute/runtime/ipc/stream.hpp

// A turn's calls from a guest to its host, as records in one range of the
// memory: the guest writes them during a turn, and the host runs them all
// once the turn has returned, before the next begins — so a record written
// in turn N has been run by the time turn N + 1 rises, which is what lets a
// record name memory the host writes into and the guest read it next turn.
//
// The range opens on `head` words, of which word 0 is END: how many words of
// records the turn wrote, set when the turn ends. The rest of the head is the
// user's. Past it, a record is its op, the count of the words that follow,
// and those words; bytes go as their count, then the bytes padded to a word.

#pragma once

#include <cstdint>
#include <optional>
#include <span>

#include "../mem/memory.hpp"

namespace xpute {

class Stream {
  public:
    // The head word holding how many words of records the turn wrote.
    static constexpr std::uint32_t END = 0;

    // A stream over `words` words at `at`, its first `head` of them the head,
    // empty.
    Stream(Memory mem, std::uint32_t at, std::uint32_t words, std::uint32_t head = 1) noexcept;

    // Where the head lies: END, then the user's words.
    std::uint32_t head_at() const noexcept { return at_; }

    // Bytes left for records this turn.
    std::uint32_t left() const noexcept { return (words_ - head_ - len_) * 4; }

    // Room for a record of `n` words after its op and count: where those
    // words begin, to be filled. One past the range is a guest that asked
    // more of a turn than it sized the range for, and a bug.
    std::uint32_t open(std::uint32_t op, std::uint32_t n) noexcept;

    // A record of `op`: `args`, then `tail`, then `bytes` if any.
    void record(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail = {},
                std::optional<std::span<const std::uint8_t>> bytes = std::nullopt) noexcept;

    // Ends the turn's records: END says how many words there are, and the
    // next turn writes from the start.
    void publish() noexcept;

  private:
    Memory mem_;
    std::uint32_t at_;
    std::uint32_t words_;
    std::uint32_t head_;
    // Words of records written this turn.
    std::uint32_t len_ = 0;
};

} // namespace xpute
