// cpp/xpute/core/status/bug.hpp

// A broken invariant, said as a number and a place, as xpute-core's
// status/bug.rs says one: the errno of what did not hold, where in the source
// it did not, and at most two numbers the place cannot give back. Nothing
// else, and no sentence. The failure is fatal (error.hpp's policy): what
// reports it is installed by whoever assembles the program (`set_report`),
// and the program stops after.

#pragma once

#include <cstdint>
#include <optional>
#include <source_location>

#include "errno.spec.hpp"

namespace xpute {

// Where a report goes before the program stops.
using Report = void (*)(Errno code, const std::source_location &at, std::uint64_t a, std::uint64_t b);

// Installs where a report goes; the first installed stands.
void set_report(Report report) noexcept;

// Reports `code` at the caller's place, with `a` and `b`, and stops.
[[noreturn]] void bug(Errno code, std::uint64_t a = 0, std::uint64_t b = 0, std::source_location at = std::source_location::current()) noexcept;

// A bug unless `cond` holds.
inline void ensure(bool cond, Errno code, std::uint64_t a = 0, std::uint64_t b = 0, std::source_location at = std::source_location::current()) noexcept {
    if (!cond) bug(code, a, b, at);
}

// A value an invariant says is there, or a bug at the caller's place.
template <class T> T or_bug(std::optional<T> v, Errno code, std::source_location at = std::source_location::current()) noexcept {
    if (!v) bug(code, 0, 0, at);
    return *v;
}

} // namespace xpute
