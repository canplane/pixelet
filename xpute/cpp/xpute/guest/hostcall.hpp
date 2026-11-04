// cpp/xpute/guest/hostcall.hpp

// The host's `hostcall`: a call's frame at an offset, answered before it
// returns, a value or -errno. Until the host installs one, ENOSYS, which
// tests run on.

#pragma once

#include <cstdint>

#include "xpute/kit/status/errno.spec.hpp"
#include "xpute/kit/status/error.hpp"

namespace xpute {

using Hostcall = std::int32_t (*)(std::uint32_t at);

void set_hostcall(Hostcall hostcall) noexcept;

// `at` is the frame's offset in the core memory.
std::int32_t hostcall(std::uint32_t at) noexcept;

// A host's answer as one value: a count, or the errno it refused with. A code
// no errno names is the host breaking the protocol, EPROTO.
inline Result<std::uint32_t> answer_of(std::int32_t res) noexcept {
    if (res < 0) return make_error(ErrorClass::fault, errno_of(res).value_or(Errno::eproto));
    return static_cast<std::uint32_t>(res);
}

} // namespace xpute
