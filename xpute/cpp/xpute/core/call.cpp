// cpp/xpute/core/call.cpp

#include "call.hpp"

#include "xpute/kit/status/errno.spec.hpp"

namespace xpute {

namespace {
Call installed_call = nullptr;
}

void set_call(Call call) noexcept {
    installed_call = call;
}

std::int32_t call(std::uint32_t at) noexcept {
    return installed_call != nullptr ? installed_call(at) : static_cast<std::int32_t>(Errno::enosys);
}

} // namespace xpute
