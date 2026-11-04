// cpp/xpute/guest/ipc/fs.hpp

// The file device's guest end, as ipc/fs.json numbers it: a descriptor, and
// what one posts in a turn. Its calls are `Door`'s (ipc/door.hpp), its opens'
// owners `Files`' (ipc/file.hpp).

#pragma once

#include <cstdint>

#include "fs.spec.hpp"

namespace xpute {

using Fd = std::uint32_t;

// The most frames one descriptor posts in a turn: its open, a read or a write,
// and its close, twice where it is stopped.
inline constexpr std::uint32_t FD_TURN_FRAMES = 4;

} // namespace xpute
