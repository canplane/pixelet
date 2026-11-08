// cpp/xpute/core/ipc/fs.hpp

// The core's end of the file device, as ipc/fs.json numbers it: a
// descriptor, and what one descriptor posts in a turn. The file calls are
// `Door`'s (ipc/door.hpp), and `Files` keeps the owner of each open
// (ipc/file.hpp).

#pragma once

#include <cstdint>

#include "fs.spec.hpp"

namespace xpute {

using Fd = std::uint32_t;

// A completed FS_OPENAT (abi/fs.json): its descriptor, which is the frame's
// tag; the file's size, or a negative errno; and any number the runtime
// refused with, kept only for a reader to print.
struct FsOpened {
    Fd fd = 0;
    std::int32_t res = 0;
    std::uint32_t detail = 0;
};

// A completed FS_CLOSE: its descriptor, and 0, or a negative errno when a
// file opened for writing could not be replaced.
struct FsClosed {
    Fd fd = 0;
    std::int32_t res = 0;
};

// The most frames one descriptor posts in a turn: its open, a read or a
// write, and its close, which a stopped descriptor may post twice.
inline constexpr std::uint32_t FD_TURN_FRAMES = 4;

} // namespace xpute
