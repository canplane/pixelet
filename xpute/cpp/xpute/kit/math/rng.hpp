// cpp/xpute/kit/math/rng.hpp

// These seeds and streams are reproducible, and not for anything that must
// resist prediction. There are two families, a file each: a 64-bit one built
// on splitmix64 (rng.64.hpp), and a 32-bit one built on Murmur3's finalizer
// (rng.32.hpp), which a GPU shader can compute as well.

#pragma once

#include <cstdint>

#include "rng.32.hpp"
#include "rng.64.hpp"

namespace xpute {

// Maps a `u32` onto [0, 1), uniformly.
constexpr double u32_to_unit(std::uint32_t x) noexcept {
    return static_cast<double>(x) / 4294967296.0;
}

} // namespace xpute
