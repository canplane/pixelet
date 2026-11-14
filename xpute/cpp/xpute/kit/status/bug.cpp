// cpp/xpute/kit/status/bug.cpp

#include "bug.hpp"

#include <atomic>
#include <cstdlib>

namespace xpute {

namespace {
std::atomic<Report> installed_report{nullptr};
}

void set_report(Report report) noexcept {
    Report none = nullptr;
    installed_report.compare_exchange_strong(none, report);
}

#ifndef __wasm__
std::jmp_buf *&bug_detail::catcher() noexcept {
    thread_local std::jmp_buf *innermost = nullptr;
    return innermost;
}
#endif

void bug(Errno errnum, std::uint64_t a, std::uint64_t b, std::source_location at) noexcept {
    if (Report report = installed_report.load()) report(errnum, at, a, b);
#ifndef __wasm__
    if (std::jmp_buf *c = bug_detail::catcher()) std::longjmp(*c, 1);
#endif
    std::abort();
}

} // namespace xpute

#ifdef __wasm__
// libc++'s own definition prints to stderr. On WebAssembly that imports
// WASI's file calls, which would give the module a second way to reach the
// runtime beside the contract, so a failed libc++ check is a bug instead.
void std::__libcpp_verbose_abort(char const *, ...) noexcept {
    xpute::bug(xpute::Errno::enotrecoverable);
}

// libc++abi's `__cxa_pure_virtual`, which an abstract class's vtable names,
// prints to stderr in the same way, so it is replaced as well.
extern "C" void __cxa_pure_virtual() {
    xpute::bug(xpute::Errno::enotrecoverable);
}
#endif
