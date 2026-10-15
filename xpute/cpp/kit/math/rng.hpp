// cpp/kit/math/rng.hpp

// Reproducible seeds and streams; not for anything that must resist prediction.
// Two families, a file each: 64-bit (splitmix64, rng.64.hpp) and 32-bit
// (Murmur3's finalizer, rng.32.hpp), the latter gpu-portable, since WebGPU has
// no 64-bit integer.

#pragma once

#include <cstdint>

#include "rng.32.hpp"
#include "rng.64.hpp"

namespace xpute {

/// Uniform in [0, 1).
constexpr double u32_to_unit(std::uint32_t x) noexcept {
    return static_cast<double>(x) / 4294967296.0;
}

} // namespace xpute
