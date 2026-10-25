// cpp/xpute/kit/status/bug.hpp

// A broken invariant, said as an errno, a place and at most two numbers. It is
// fatal: the installed report runs and the program stops.

#pragma once

#include <cstdint>
#include <optional>
#include <source_location>

#include "errno.spec.hpp"
#include "error.hpp"

namespace xpute {

using Report = void (*)(Errno errnum, const std::source_location &at, std::uint64_t a, std::uint64_t b);

// The first installed stands.
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

} // namespace xpute
