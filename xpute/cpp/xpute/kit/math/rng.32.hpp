// cpp/xpute/kit/math/rng.32.hpp

// The 32-bit family is the counterpart of rng.64.hpp in `u32` arithmetic alone:
// WebGPU has no 64-bit integer, so a shader can compute these same numbers. The
// family holds Murmur3's finalizer, a seed derived through it, and a stream
// that applies it to a Weyl sequence.

#pragma once

#include <cstdint>

namespace xpute {

inline constexpr std::uint32_t SEED_SALT_32 = 0x9e3779b9u;

constexpr std::uint32_t mix32(std::uint32_t x) noexcept {
    std::uint32_t z = x + SEED_SALT_32;
    z = (z ^ z >> 16) * 0x85ebca6bu;
    z = (z ^ z >> 13) * 0xc2b2ae35u;
    return z ^ z >> 16;
}

constexpr std::uint32_t seed_mix32(std::uint32_t base, std::uint32_t tag) noexcept {
    return mix32(base ^ tag);
}

class SplitMix32 {
  public:
    constexpr explicit SplitMix32(std::uint32_t seed) noexcept : x_(seed) {}

    constexpr std::uint32_t next_u32() noexcept {
        std::uint32_t z = mix32(x_);
        x_ += SEED_SALT_32;
        return z;
    }

  private:
    std::uint32_t x_;
};

} // namespace xpute
