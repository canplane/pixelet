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
    // The program stops: the report is all that is said.
    std::abort();
}

} // namespace xpute

#ifdef __wasm__
// Where libc++ stops a program whose use of the library broke one of the
// library's own preconditions — a variant read as the wrong alternative, say.
// Its own definition prints a sentence to stderr, and a WebAssembly module
// has none: printing through the WASI SDK's libc imports WASI's file calls,
// a second way to its host beside the contract. libc++ lets a program define
// this in its place, and a guest's broken invariant is a bug, said as a
// number where it happened.
void std::__libcpp_verbose_abort(char const *, ...) noexcept {
    xpute::bug(xpute::Errno::enotrecoverable);
}
#endif
