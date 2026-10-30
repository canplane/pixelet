// cpp/xpute/guest/hostcall.cpp

#include "hostcall.hpp"

#include "xpute/kit/status/errno.spec.hpp"

namespace xpute {

namespace {
Hostcall installed_hostcall = nullptr;
}

void set_hostcall(Hostcall hostcall) noexcept {
    installed_hostcall = hostcall;
}

std::int32_t hostcall(std::uint32_t at) noexcept {
    return installed_hostcall != nullptr ? installed_hostcall(at) : static_cast<std::int32_t>(Errno::enosys);
}

} // namespace xpute
