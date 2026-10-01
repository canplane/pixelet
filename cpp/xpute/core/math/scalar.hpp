// cpp/xpute/core/math/scalar.hpp

// Functions of one double: comparison within a tolerance, the three ways a
// half rounds and the one the project takes, interpolation, and clamping or
// wrapping into a range. Every one is exact in IEEE arithmetic, so a double
// here is the same bits as Rust's (golden/math/scalar.tsv holds both).

#pragma once

// Bit for bit with Rust, which never fuses a multiply and an add: held here,
// not only in the build's flags.
#pragma STDC FP_CONTRACT OFF

#include <cmath>
#include <limits>

namespace xpute {

inline constexpr double EPS = 1e-5;

// Precision equality using both absolute and relative tolerance: as
// magnitudes grow, the allowed error scales with them.
inline bool approx_eq(double a, double b, double eps = EPS) noexcept {
    if (a == b) return true;
    double diff = std::fabs(a - b);
    return diff < eps || diff < eps * std::fmax(std::fabs(a), std::fabs(b));
}

// Proximity by absolute distance alone.
inline bool near(double a, double b, double tol) noexcept {
    return std::fabs(a - b) < tol;
}

// The three ways a half rounds, each by where it goes. The project rounds to
// even — IEEE 754's default, and WGSL's — so a value the CPU and a shader both
// round lands on one integer; the other two are for agreeing with a language
// that chose otherwise.

// Toward +∞: JavaScript's `Math.round`. -2.5 is -2.
inline double round_half_up(double x) noexcept {
    return x - std::trunc(x) == -0.5 ? std::ceil(x) : std::round(x);
}

// Away from zero: C's `round`. -2.5 is -3.
inline double round_half_away(double x) noexcept {
    return std::round(x);
}

// To even: IEEE 754's. 2.5 is 2, -2.5 is -2. `nearbyint` rounds by the
// current mode, which nothing here changes from its default, to nearest even.
inline double round_half_even(double x) noexcept {
    // x − floor(x) is exact for every double, and 0 past 2^52, where every
    // double is whole: so the tie is told exactly, whatever the rounding mode.
    double f = std::floor(x), d = x - f;
    double r = d < 0.5 ? f : d > 0.5 ? f + 1 : std::fmod(f, 2.0) == 0 ? f : f + 1;
    // A zero keeps the sign of what was rounded, as Rust's round_ties_even
    // gives -0 for -0.5.
    return r == 0 ? std::copysign(0.0, x) : r;
}

// The project's rounding, which is to even.
inline double round(double x) noexcept {
    return round_half_even(x);
}

// `v` as an integer of type T, as Rust's `as` casts a float: toward zero,
// held to T's range at either end, and NaN as 0 — where a plain cast is
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

// Remaps a value from one range into another.
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
