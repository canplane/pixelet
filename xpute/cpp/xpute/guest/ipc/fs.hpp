// cpp/xpute/guest/ipc/fs.hpp

// The file device's guest end, as ipc/fs.json numbers it: a descriptor, and
// what one posts in a turn. Its calls are `Door`'s (ipc/door.hpp), its opens'
// owners `Files`' (ipc/file.hpp).

#pragma once

#include <cstdint>

#include "fs.spec.hpp"

namespace xpute {

using Fd = std::uint32_t;

// An FS_OPENAT completed (abi/fs.json): its descriptor, the frame's tag; the
// file's size, or -errno; and whatever number the host refused with, for a
// reader to print.
struct FsOpened {
    Fd fd = 0;
    std::int32_t res = 0;
    std::uint32_t detail = 0;
};

// An FS_CLOSE completed: its descriptor, and 0, or -errno where a file opened
// for writing could not be replaced.
struct FsClosed {
    Fd fd = 0;
    std::int32_t res = 0;
};

// The most frames one descriptor posts in a turn: its open, a read or a write,
// and its close, twice where it is stopped.
inline constexpr std::uint32_t FD_TURN_FRAMES = 4;

} // namespace xpute
