// cpp/xpute/runtime/ipc/sys.hpp

// A guest's system calls, recorded in a stream the host runs after the turn
// returns; what a call names in the guest's memory is kept until the next
// rise. A descriptor is a handle of the guest's own table, so a late answer to
// a closed one names nothing.

#pragma once

#include <cstdint>
#include <source_location>
#include <span>
#include <string_view>
#include <vector>

#include "../../core/status/errno.spec.hpp"
#include "../mem/memory.hpp"
#include "stream.hpp"
#include "sys.spec.hpp"

namespace xpute {

using Fd = std::uint32_t;

class Sys {
  public:
    // `at` is a range nothing else writes, named under SYS in the directory.
    Sys(Memory mem, std::uint32_t at, std::uint32_t words) noexcept : mem_(mem), stream_(mem, at, words, sys::HEAD) {}

    // What the host ran after the last turn is done, so what was kept for it goes.
    void rise() noexcept { held_.clear(); }

    void fall() noexcept { stream_.publish(); }

    // Answered by SYS_OPENED on a later turn.
    void openat(Fd fd, std::uint32_t root, std::uint32_t flags, std::string_view path) noexcept;

    // `dst` is filled from the next turn on and must be kept until then.
    void read(Fd fd, std::span<std::uint8_t> dst) noexcept;

    void write(Fd fd, std::vector<std::uint8_t> bytes) noexcept;

    // A write is committed, an open not yet answered cancelled.
    void close(Fd fd) noexcept;

    // `op` carries the PROGRAM bit.
    void call(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint8_t> bytes) noexcept;

    // The one call of the turn, published at once; what was recorded before it is
    // dropped.
    void abort(Errno code, const std::source_location &at, std::uint64_t a, std::uint64_t b) noexcept;

  private:
    Memory mem_;
    Stream stream_;
    std::vector<std::vector<std::uint8_t>> held_;
};

} // namespace xpute
