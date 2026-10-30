// cpp/xpute/guest/hostcall.hpp

// The host's `hostcall`: a record at an offset, run before it returns, and
// its answer, a value or -errno. Until the host installs one, ENOSYS, which
// tests run on.

#pragma once

#include <cstdint>

namespace xpute {

using Hostcall = std::int32_t (*)(std::uint32_t at);

void set_hostcall(Hostcall hostcall) noexcept;

// `at` is the record's offset in the core memory.
std::int32_t hostcall(std::uint32_t at) noexcept;

} // namespace xpute
