// cpp/guest/ipc/sys.hpp

// A guest's system calls, recorded in a stream the host runs after the turn
// returns; what a call names in the guest's memory is kept until the next
// rise. A descriptor is a handle of the guest's own table, so a late answer to
// a closed one names nothing.

#pragma once

#include <cstddef>
#include <cstdint>
#include <source_location>
#include <span>
#include <string_view>
#include <vector>

#include "../../kit/status/errno.spec.hpp"
#include "../mem/memory.hpp"
#include "stream.hpp"
#include "sys.spec.hpp"

namespace xpute {

using Fd = std::uint32_t;

// Each call's arguments, which its record takes as an array of this length.
inline constexpr std::size_t OPENAT_ARGS = 3;
inline constexpr std::size_t READ_ARGS = 3;
inline constexpr std::size_t WRITE_ARGS = 3;
inline constexpr std::size_t CLOSE_ARGS = 1;

// The most one descriptor records in a turn, its name `path` bytes long: its
// open, a read or a write, and its close, twice where it is stopped.
constexpr std::uint64_t descriptor_turn_bytes(std::uint64_t path) noexcept {
    std::uint64_t moved = READ_ARGS > WRITE_ARGS ? READ_ARGS : WRITE_ARGS;
    return record_bytes(OPENAT_ARGS, path) + record_bytes(moved, std::nullopt) + 2 * record_bytes(CLOSE_ARGS, std::nullopt);
}

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

    // `call` where it fits with `reserve` bytes left after it; false, and nothing
    // recorded, where it does not, for the caller to ask again a later turn.
    bool try_call(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint8_t> bytes, std::uint32_t reserve) noexcept;

    // The one call of the turn, published at once; what was recorded before it is
    // dropped.
    void abort(Errno code, const std::source_location &at, std::uint64_t a, std::uint64_t b) noexcept;

  private:
    Memory mem_;
    Stream stream_;
    std::vector<std::vector<std::uint8_t>> held_;
};

} // namespace xpute
