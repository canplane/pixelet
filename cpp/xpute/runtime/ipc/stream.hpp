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

class Stream {
  public:
    static constexpr std::uint32_t END = 0;

    Stream(Memory mem, std::uint32_t at, std::uint32_t words, std::uint32_t head = 1) noexcept;

    std::uint32_t head_at() const noexcept { return at_; }

    std::uint32_t left() const noexcept { return (words_ - head_ - len_) * 4; }

    // Answers where the `n` words begin. Past the range is a bug: the guest asked
    // more of a turn than it sized the range for.
    std::uint32_t open(std::uint32_t op, std::uint32_t n) noexcept;

    void record(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint32_t> tail = {},
                std::optional<std::span<const std::uint8_t>> bytes = std::nullopt) noexcept;

    // The next turn writes from the start.
    void publish() noexcept;

  private:
    Memory mem_;
    std::uint32_t at_;
    std::uint32_t words_;
    std::uint32_t head_;
    std::uint32_t len_ = 0;
};

} // namespace xpute
