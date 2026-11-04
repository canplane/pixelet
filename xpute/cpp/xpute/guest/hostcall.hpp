// cpp/xpute/guest/hostcall.hpp

// The host's `hostcall`: a call's frame at an offset, answered before it
// returns, a value or -errno. Until the host installs one, ENOSYS, which
// tests run on.

#pragma once

#include <cstdint>

namespace xpute {

using Hostcall = std::int32_t (*)(std::uint32_t at);

void set_hostcall(Hostcall hostcall) noexcept;

// `at` is the frame's offset in the core memory.
std::int32_t hostcall(std::uint32_t at) noexcept;

} // namespace xpute
