// cpp/xpute/core/abi/number.hpp

// A number a packet carries is read as its field's type. What the runtime
// sends is checked here, so that no code past the door (ipc/door.hpp) meets a
// NaN, an infinity, or an integer forced into range by a cast.

#pragma once

#include <cmath>
#include <cstdint>
#include <limits>
#include <source_location>
#include <utility>

#include "xpute/kit/status/error.hpp"

namespace xpute::abi {

// The error for a number that no member of an enum is given, where the
// enum has no member for such a number to read as.
constexpr Error unknown(std::source_location at = std::source_location::current()) noexcept {
    return marshal_error(Errno::einval, at);
}

inline Result<double> finite(double n, std::source_location at = std::source_location::current()) noexcept {
    if (!std::isfinite(n)) return unknown(at);
    return n;
}

// Reads a whole number that the type holds. A number outside the type's range
// is refused, never cast into it.
template <class T> Result<T> integer(double n, std::source_location at = std::source_location::current()) noexcept {
    bool whole = std::isfinite(n) && n == std::trunc(n) && std::fabs(n) <= static_cast<double>(std::uint64_t{1} << std::numeric_limits<double>::digits);
    if (!whole) return unknown(at);
    auto i = static_cast<std::int64_t>(n);
    if (!std::in_range<T>(i)) return unknown(at);
    return static_cast<T>(i);
}

} // namespace xpute::abi
