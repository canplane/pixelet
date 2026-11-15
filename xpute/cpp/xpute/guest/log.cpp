// cpp/xpute/guest/log.cpp

#include "log.hpp"

namespace xpute::guest {

void log(abi::log::Level level, std::string_view context, std::string_view message) noexcept {
    (void)abi::log::log(level, context, message);
}

} // namespace xpute::guest
