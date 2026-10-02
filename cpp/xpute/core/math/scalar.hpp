// cpp/xpute/core/math/scalar.hpp

// Functions of one double, each exact in IEEE arithmetic so the bits match
// Rust's.

#pragma once

// Rust never fuses a multiply and an add; held here, not only in the flags.
#pragma STDC FP_CONTRACT OFF

#include <cmath>
#include <limits>

namespace xpute {

inline constexpr double EPS = 1e-5;

// Absolute and relative tolerance, so the allowed error scales with magnitude.
inline bool approx_eq(double a, double b, double eps = EPS) noexcept {
    if (a == b) return true;
    double diff = std::fabs(a - b);
    return diff < eps || diff < eps * std::fmax(std::fabs(a), std::fabs(b));
}

inline bool near(double a, double b, double tol) noexcept {
    return std::fabs(a - b) < tol;
}

// The project rounds to even, as IEEE 754 and WGSL do, so a value the CPU and
// a shader both round lands on one integer.

// -2.5 is -2, as JavaScript's `Math.round`.
inline double round_half_up(double x) noexcept {
    return x - std::trunc(x) == -0.5 ? std::ceil(x) : std::round(x);
}

// -2.5 is -3, as C's `round`.
inline double round_half_away(double x) noexcept {
    return std::round(x);
}

// 2.5 is 2, -2.5 is -2.
inline double round_half_even(double x) noexcept {
    // x - floor(x) is exact for every double, and 0 past 2^52, so the tie is told
    // exactly whatever the rounding mode.
    double f = std::floor(x), d = x - f;
    double r = d < 0.5 ? f : d > 0.5 ? f + 1 : std::fmod(f, 2.0) == 0 ? f : f + 1;
    // -0.5 rounds to -0, as Rust's round_ties_even.
    return r == 0 ? std::copysign(0.0, x) : r;
}

inline double round(double x) noexcept {
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

inline double lerp(double a, double b, double t) noexcept {
    return a + (b - a) * t;
}

inline double inv_lerp(double x, double min, double max) noexcept {
    return (x - min) / (max - min);
}

inline double remap(double x, double in_min, double in_max, double out_min, double out_max) noexcept {
    return lerp(out_min, out_max, inv_lerp(x, in_min, in_max));
}

inline double clamp(double x, double min, double max) noexcept {
    return x < min ? min : x > max ? max : x;
}

inline double wrap(double x, double min, double max) noexcept {
    double range = max - min;
    return std::fmod(std::fmod(x - min, range) + range, range) + min;
}

inline double floor_to_step(double x, double step) noexcept {
    return std::floor(x / step) * step;
}

inline double round_to_step(double x, double step) noexcept {
    return round(x / step) * step;
}

inline double ceil_to_step(double x, double step) noexcept {
    return std::ceil(x / step) * step;
}

} // namespace xpute
