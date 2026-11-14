// cpp/xpute/guest/stop.cpp

#include "stop.hpp"

#include <cstdint>
#include <source_location>

#include "xpute/guest/abi/stop.hpp"
#include "xpute/guest/link.hpp"
#include "xpute/kit/status/bug.hpp"

namespace xpute::guest::stop {

void hook() noexcept {
    set_report([](Errno errnum, const std::source_location &at, std::uint64_t n1, std::uint64_t n2) {
        if (in_guest()) (void)abi::stop::stopped(static_cast<std::int32_t>(errnum), at.file_name(), at.line(), n1, n2);
    });
}

} // namespace xpute::guest::stop
