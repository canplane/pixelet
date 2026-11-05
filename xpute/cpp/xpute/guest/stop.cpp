// cpp/xpute/guest/stop.cpp

#include "stop.hpp"

namespace xpute::stop {

namespace {
Report installed_report = nullptr;
Hook installed_hook = nullptr;
} // namespace

void install(Report report, Hook hook) noexcept {
    installed_report = report;
    installed_hook = hook;
    set_report([](Errno errnum, const std::source_location &at, std::uint64_t a, std::uint64_t b) {
        if (installed_hook != nullptr) installed_hook(Stop{errnum, at, a, b});
        if (installed_report != nullptr) installed_report(errnum, at, a, b);
    });
}

} // namespace xpute::stop
