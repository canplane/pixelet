// cpp/xpute/guest/log.cpp

#include "log.hpp"

namespace xpute::guest {

void log(abi::log::Level level, std::string_view context, std::string_view msg) noexcept {
    (void)abi::log::log(level, context, msg);
}

} // namespace xpute::guest
