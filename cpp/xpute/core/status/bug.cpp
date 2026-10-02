// cpp/xpute/core/status/bug.cpp

#include "bug.hpp"

#include <atomic>
#include <cstdlib>

namespace xpute {

namespace {
std::atomic<Report> installed{nullptr};
}

void set_report(Report report) noexcept {
    Report none = nullptr;
    installed.compare_exchange_strong(none, report);
}

void bug(Errno code, std::uint64_t a, std::uint64_t b, std::source_location at) noexcept {
    if (Report report = installed.load()) report(code, at, a, b);
    std::abort();
}

} // namespace xpute

#ifdef __wasm__
// libc++'s own definition prints to stderr, which on wasm imports WASI's file
// calls, a second way to the host beside the contract.
void std::__libcpp_verbose_abort(char const *, ...) noexcept {
    xpute::bug(xpute::Errno::enotrecoverable);
}
#endif
