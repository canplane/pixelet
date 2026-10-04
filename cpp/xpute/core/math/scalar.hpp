// cpp/xpute/core/math/scalar.hpp

// Functions of one float or double, each exact in IEEE arithmetic so the bits
// match Rust's.

#pragma once

// Rust never fuses a multiply and an add; held here, not only in the flags.
// GCC does not read the pragma, so there the flag alone holds it.
#if defined(__clang__)
#pragma STDC FP_CONTRACT OFF
#endif

#include <cmath>
#include <limits>
#include <type_traits>

namespace xpute {

inline constexpr double EPS = 1e-5;

// What these functions take: double, and float where a value lives as one.
template <class T>
concept Float = std::is_floating_point_v<T>;

// Absolute and relative tolerance, so the allowed error scales with magnitude.
template <Float T> inline bool approx_eq(T a, T b, T eps = static_cast<T>(EPS)) noexcept {
    if (a == b) return true;
    T diff = std::fabs(a - b);
    return diff < eps || diff < eps * std::fmax(std::fabs(a), std::fabs(b));
}

template <Float T> inline bool near(T a, T b, T tol) noexcept {
    return std::fabs(a - b) < tol;
}

// The project rounds to even, as IEEE 754 and WGSL do, so a value the CPU and
// a shader both round lands on one integer.

// -2.5 is -2, as JavaScript's `Math.round`.
template <Float T> inline T round_half_up(T x) noexcept {
    return x - std::trunc(x) == T(-0.5) ? std::ceil(x) : std::round(x);
}

// -2.5 is -3, as C's `round`.
template <Float T> inline T round_half_away(T x) noexcept {
    return std::round(x);
}

// 2.5 is 2, -2.5 is -2.
template <Float T> inline T round_half_even(T x) noexcept {
    // x - floor(x) is exact for every value, and 0 past the mantissa, so the
    // tie is told exactly whatever the rounding mode.
    T f = std::floor(x), d = x - f;
    T r = d < T(0.5) ? f : d > T(0.5) ? f + 1 : std::fmod(f, T(2)) == 0 ? f : f + 1;
    // -0.5 rounds to -0, as Rust's round_ties_even.
    return r == 0 ? std::copysign(T(0), x) : r;
}

template <Float T> inline T round(T x) noexcept {
    return round_half_even(x);
}

// As Rust's `as`: toward zero, saturating, NaN as 0, where a plain cast is
// undefined past the range.
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
