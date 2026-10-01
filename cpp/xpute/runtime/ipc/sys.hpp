// cpp/xpute/runtime/ipc/sys.hpp

// A guest's system calls (spec/xpute/ipc/sys.json): what it asks of its host
// that no command answers, recorded in a stream the host runs once the turn
// has returned. A call that names the guest's memory is done by the time the
// next turn rises, and what it names is kept that long.
//
// Files are reached as Plan 9 reaches them: the guest issues the descriptor
// — a handle of its own table, so a late answer to one it closed names
// nothing the slot holds after — and names a file by a root and a path below
// it. `openat` is answered by the command SYS_OPENED on a later turn;
// `read`, `write` and `close` are done before the next turn rises.
//
// A program's own calls go in the same stream through `call`, numbered past
// the PROGRAM bit.

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

// A descriptor: a handle the guest issued (abi/handle.hpp).
using Fd = std::uint32_t;

class Sys {
  public:
    // The calls over `words` words at `at`, a range of the memory nothing
    // else writes, which the directory names under SYS.
    Sys(Memory mem, std::uint32_t at, std::uint32_t words) noexcept : mem_(mem), stream_(mem, at, words, sys::HEAD) {}

    // The turn's rise: what the host ran after the last turn is done, so what
    // was kept for it goes.
    void rise() noexcept { held_.clear(); }

    // The turn's fall: its calls, for the host to run once it returns.
    void fall() noexcept { stream_.publish(); }

    // Opens the file at `path` below `root` as `fd`, with `flags`
    // (sys::flag). Answered by SYS_OPENED.
    void openat(Fd fd, std::uint32_t root, std::uint32_t flags, std::string_view path) noexcept;

    // The opened file's first `dst.size()` bytes, in `dst` from the next turn
    // on; `dst` must be kept until then.
    void read(Fd fd, std::span<std::uint8_t> dst) noexcept;

    // `bytes` added to what the file opened for writing as `fd` holds.
    void write(Fd fd, std::vector<std::uint8_t> bytes) noexcept;

    // Ends `fd`: a write is committed, an open not yet answered cancelled.
    void close(Fd fd) noexcept;

    // A program's own call: `op` with the PROGRAM bit, its arguments and its
    // bytes.
    void call(std::uint32_t op, std::span<const std::uint32_t> args, std::span<const std::uint8_t> bytes) noexcept;

    // A broken invariant's report, as the one call of the turn and published
    // at once, for the host to find when the stop reaches it: what was
    // recorded before it is dropped, since none of it is to be done.
    void abort(Errno code, const std::source_location &at, std::uint64_t a, std::uint64_t b) noexcept;

  private:
    Memory mem_;
    Stream stream_;
    // What a `write` names, kept until the host has run it.
    std::vector<std::vector<std::uint8_t>> held_;
};

} // namespace xpute
