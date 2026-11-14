// cpp/xpute/kit/status/bug.hpp

// A broken invariant stops the program. A bug is an errno, its place in the
// source and at most two numbers. It is handed to the report that `set_report`
// installed, and then the program stops: a WebAssembly module traps, and a
// native build returns from the innermost `caught` it runs in, as Rust's
// panic unwinds to its `catch_unwind`, or aborts where it runs in none.

#pragma once

#include <cstdint>
#include <optional>
#include <source_location>
#include <utility>

#include "errno.spec.hpp"
#include "error.hpp"

#ifndef __wasm__
#include <csetjmp>
#endif

namespace xpute {

using Report = void (*)(Errno errnum, const std::source_location &at, std::uint64_t a, std::uint64_t b);

// Installs the function a bug is reported to. Only the first call takes
// effect.
void set_report(Report report) noexcept;

[[noreturn]] void bug(Errno errnum, std::uint64_t a = 0, std::uint64_t b = 0, std::source_location at = std::source_location::current()) noexcept;

inline void ensure(bool cond, Errno errnum, std::uint64_t a = 0, std::uint64_t b = 0, std::source_location at = std::source_location::current()) noexcept {
    if (!cond) bug(errnum, a, b, at);
}

template <class T> T or_bug(std::optional<T> v, Errno errnum, std::source_location at = std::source_location::current()) noexcept {
    if (!v) bug(errnum, 0, 0, at);
    return *v;
}

template <class T> T or_bug(Result<T> v, Errno errnum, std::source_location at = std::source_location::current()) noexcept {
    ensure(v.ok(), errnum, 0, 0, at);
    return std::move(*v);
}

#ifndef __wasm__
namespace bug_detail {
// The innermost `caught` on this thread, which a bug returns to.
std::jmp_buf *&catcher() noexcept;
} // namespace bug_detail

// Runs `f` and answers what it answers, or `trap` where a bug stopped it. A
// bug leaves by `longjmp`, so what `f` held is never destroyed: the program
// it belongs to is over.
template <class T, class F> T caught(T trap, F &&f) noexcept {
    std::jmp_buf here;
    std::jmp_buf *outer = std::exchange(bug_detail::catcher(), &here);
    if (setjmp(here) != 0) {
        bug_detail::catcher() = outer;
        return trap;
    }
    T v = f();
    bug_detail::catcher() = outer;
    return v;
}
#endif

} // namespace xpute
