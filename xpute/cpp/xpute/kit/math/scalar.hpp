// cpp/xpute/kit/math/scalar.hpp

// These are scalar functions of a float or a double. Each is computed in plain
// IEEE arithmetic, with no fused operations, so its bits match the Rust kit's.

#pragma once

// Rust never fuses a multiply and an add, so neither may this code. Clang is
// held to that by this pragma as well as by the build's flags; GCC ignores
// the pragma and relies on the flag alone.
#if defined(__clang__)
#pragma STDC FP_CONTRACT OFF
#endif

#include <cmath>
#include <limits>
#include <type_traits>

namespace xpute {

inline constexpr double EPS = 1e-5;

// These are the float types these functions take: double, and float for a
// value that is stored as one.
template <class T>
concept Float = std::is_floating_point_v<T>;

// Equal within `eps`, by default `EPS`. The tolerance is absolute near zero
// and relative to the larger magnitude beyond.
template <Float T> inline bool approx_eq(T a, T b, T eps = static_cast<T>(EPS)) noexcept {
    if (a == b) return true;
    T diff = std::fabs(a - b);
    return diff < eps || diff < eps * std::fmax(std::fabs(a), std::fabs(b));
}

template <Float T> inline bool near(T a, T b, T tol) noexcept {
    return std::fabs(a - b) < tol;
}

// Rounds as JavaScript's `Math.round` does: half rounds up, so -2.5 becomes
// -2.
template <Float T> inline T round_half_up(T x) noexcept {
    return x - std::trunc(x) == T(-0.5) ? std::ceil(x) : std::round(x);
}

// Rounds as C's and Rust's `round` do: half rounds away from zero, so -2.5
// becomes -3.
template <Float T> inline T round_half_away(T x) noexcept {
    return std::round(x);
}

// Rounds as IEEE 754's default and WGSL's `round` do: half rounds to even, so
// 2.5 becomes 2 and -2.5 becomes -2.
template <Float T> inline T round_half_even(T x) noexcept {
    // `x - floor(x)` is exact for every value, and 0 once `x` is too large to
    // have a fraction, so a tie is detected exactly in any rounding mode.
    T f = std::floor(x), d = x - f;
    T r = d < T(0.5) ? f : d > T(0.5) ? f + 1 : std::fmod(f, T(2)) == 0 ? f : f + 1;
    // A zero result keeps the sign of `x`, so -0.5 rounds to -0, as in Rust's
    // `round_ties_even`.
    return r == 0 ? std::copysign(T(0), x) : r;
}

// The kit's one rounding, half to even as WGSL's `round`, so the CPU and a
// shader land on the same integer. `std::round` rounds half away from zero
// instead.
template <Float T> inline T round(T x) noexcept {
    return round_half_even(x);
}

// Converts as Rust's `as` does: toward zero, saturating at the type's bounds,
// and NaN as 0. A plain `static_cast` is undefined outside the type's range.
template <class T> constexpr T saturate_as(double v) noexcept {
    if (v != v) return 0;
    if (v <= static_cast<double>(std::numeric_limits<T>::min())) return std::numeric_limits<T>::min();
    if (v >= static_cast<double>(std::numeric_limits<T>::max())) return std::numeric_limits<T>::max();
    return static_cast<T>(v);
}

template <Float T> inline T lerp(T a, T b, T t) noexcept {
    return a + (b - a) * t;
}

template <Float T> inline T inv_lerp(T x, T min, T max) noexcept {
    return (x - min) / (max - min);
}

template <Float T> inline T remap(T x, T in_min, T in_max, T out_min, T out_max) noexcept {
    return lerp(out_min, out_max, inv_lerp(x, in_min, in_max));
}

template <Float T> inline T clamp(T x, T min, T max) noexcept {
    return x < min ? min : x > max ? max : x;
}

template <Float T> inline T wrap(T x, T min, T max) noexcept {
    T range = max - min;
    return std::fmod(std::fmod(x - min, range) + range, range) + min;
}

template <Float T> inline T floor_to_step(T x, T step) noexcept {
    return std::floor(x / step) * step;
}

template <Float T> inline T round_to_step(T x, T step) noexcept {
    return round(x / step) * step;
}

template <Float T> inline T ceil_to_step(T x, T step) noexcept {
    return std::ceil(x / step) * step;
}

} // namespace xpute
