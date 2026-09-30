// cpp/xpute/runtime/ipc/stream.hpp

// A turn's calls from a guest to its host, as records in one range of the
// memory: the guest writes them during a turn, and the host runs them all
// once the turn has returned, before the next begins. The range opens on its
// END word, how many words of records the turn wrote; past it, a record is
// its op, the count of the words that follow, and those words.

#pragma once

#include <cstdint>
#include <span>

#include "../mem/memory.hpp"

namespace xpute {

class Stream {
  public:
    // A stream over `words` words at `at`, empty.
    Stream(Memory mem, std::uint32_t at, std::uint32_t words) noexcept;

    // A record of `op` and its words. One past the range is a guest that
    // asked more of a turn than it sized the range for, and stops.
    void record(std::uint32_t op, std::span<const std::uint32_t> words) noexcept;

    // Ends the turn's records: END says how many words there are, and the
    // next turn writes from the start.
    void publish() noexcept;

  private:
    Memory mem_;
    std::uint32_t at_;
    std::uint32_t words_;
    // Words of records written this turn.
    std::uint32_t len_ = 0;
};

} // namespace xpute
