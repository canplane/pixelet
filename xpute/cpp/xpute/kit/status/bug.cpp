// cpp/xpute/kit/status/bug.cpp

#include "bug.hpp"
#include "verbose_abort.hpp"

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
sigjmp_buf *&bug_detail::catcher() noexcept {
    thread_local sigjmp_buf *innermost = nullptr;
    return innermost;
}
#endif

void bug(Errno errnum, std::uint64_t a, std::uint64_t b, std::source_location at) noexcept {
    if (Report report = installed_report.load()) report(errnum, at, a, b);
#ifndef __wasm__
    if (sigjmp_buf *c = bug_detail::catcher()) siglongjmp(*c, 1);
#endif
    std::abort();
}

} // namespace xpute

void xpute_verbose_abort() noexcept {
    xpute::bug(xpute::Errno::enotrecoverable);
}

#ifdef __wasm__
// libc++'s compiled library calls its own handler, not the macro, and its
// definition prints to stderr. On WebAssembly that imports WASI's file calls,
// which would give the module a second way to reach the host beside the
// contract, so a module's handler is the bug as well.
void std::__libcpp_verbose_abort(char const *, ...) noexcept {
    xpute_verbose_abort();
}

// libc++abi's `__cxa_pure_virtual`, which an abstract class's vtable names,
// prints to stderr in the same way, so it is replaced as well.
extern "C" void __cxa_pure_virtual() {
    xpute::bug(xpute::Errno::enotrecoverable);
}

// So is libc++abi's `__abort_message`, which its type information's vtables
// reach, and which a class over one of libc++'s, as the kit's `HeapMalloc`
// memory resource is, links in.
extern "C" void __abort_message(const char *, ...) {
    xpute::bug(xpute::Errno::enotrecoverable);
}
#endif
