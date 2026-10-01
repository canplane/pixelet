// cpp/xpute/runtime/ipc/door.cpp

#include "door.hpp"

#include <cstdlib>

namespace xpute {

void Door::post(std::uint32_t tag, std::uint32_t cmd, std::uint32_t flags, std::int32_t result) const noexcept {
    if (cq_.push(tag, cmd, flags, result) != Errno::ok) std::abort();
}

} // namespace xpute
