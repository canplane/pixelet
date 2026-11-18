// cpp/xpute/kit/status/bug.hpp

// A broken invariant stops the program. A bug is an errno, its place in the
// source, at most two numbers and an optional message. It is handed to the
// report that `set_report` installed, and then the program stops: a
// WebAssembly module traps, and a native build returns from the innermost
// `caught` it runs in, as Rust's panic unwinds to its `catch_unwind`, or
// aborts where it runs in none. The message is a static string, never one
// built at the bug, as `std::bad_alloc::what()` is: a full heap is itself a
// bug, so reporting one allocates nothing, and what varies goes in the two
// numbers. A failed check of libc++'s own is told with libc++'s message,
// written into static storage at the stop (bug.cpp `abort_bug`), as a Rust
// panic `bug` did not raise is told with its own.

#pragma once

#include <cstdint>
#include <optional>
#include <source_location>
#include <utility>

#include "errno.spec.hpp"
#include "error.hpp"

#ifndef __wasm__
#include <setjmp.h>
#endif

namespace xpute {

using Report = void (*)(Errno errnum, const std::source_location &at, std::uint64_t a, std::uint64_t b, const char *msg);

// Installs the function a bug is reported to. Only the first call takes
// effect.
void set_report(Report report) noexcept;

// `msg` is what a person reads of the bug, or null.
[[noreturn]] void bug(Errno errnum, std::uint64_t a = 0, std::uint64_t b = 0, const char *msg = nullptr, std::source_location at = std::source_location::current()) noexcept;

inline void ensure(bool cond, Errno errnum, std::uint64_t a = 0, std::uint64_t b = 0, std::source_location at = std::source_location::current()) noexcept {
    if (!cond) bug(errnum, a, b, nullptr, at);
}

template <class T> T or_bug(std::optional<T> v, Errno errnum, std::source_location at = std::source_location::current()) noexcept {
    if (!v) bug(errnum, 0, 0, nullptr, at);
    return *v;
}

template <class T> T or_bug(Result<T> v, Errno errnum, std::source_location at = std::source_location::current()) noexcept {
    ensure(v.ok(), errnum, 0, 0, at);
    return std::move(*v);
}

#ifndef __wasm__
namespace bug_detail {
// The innermost `caught` on this thread, which a bug returns to.
sigjmp_buf *&catcher() noexcept;
} // namespace bug_detail

// Runs `f` and answers what it answers, or `trap` where a bug stopped it. A
// bug leaves by `siglongjmp`, so what `f` held is never destroyed: the
// program it belongs to is over. The signal mask is neither saved nor
// restored, so an entry costs no system call.
template <class T, class F> T caught(T trap, F &&f) noexcept {
    sigjmp_buf here;
    sigjmp_buf *outer = std::exchange(bug_detail::catcher(), &here);
    if (sigsetjmp(here, 0) != 0) {
        bug_detail::catcher() = outer;
        return trap;
    }
    T v = f();
    bug_detail::catcher() = outer;
    return v;
}
#endif

} // namespace xpute
