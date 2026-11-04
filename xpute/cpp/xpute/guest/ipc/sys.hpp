// cpp/xpute/guest/ipc/sys.hpp

// A guest's hostcalls, as ipc/sys.json numbers them: each a frame for the
// host's port on the completion ring (ipc/door.hpp `Door`), and what the
// host's answers read as.

#pragma once

#include <cstdint>

#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/status/error.hpp"
#include "sys.spec.hpp"

namespace xpute {

using Fd = std::uint32_t;

// A host's answer as one value: a count, or the errno it refused with. A code
// no errno names is the host breaking the protocol, EPROTO.
inline Result<std::uint32_t> answer_of(std::int32_t res) noexcept {
    if (res < 0) return make_error(ErrorClass::fault, errno_of(res).value_or(Errno::eproto));
    return static_cast<std::uint32_t>(res);
}

// The most frames one descriptor posts in a turn: its open, a read or a write,
// and its close, twice where it is stopped.
inline constexpr std::uint32_t FD_TURN_FRAMES = 4;

} // namespace xpute
