// cpp/xpute/kit/math/rng.64.hpp

// The 64-bit family: splitmix64's finalizer, a seed derived through it, and
// splitmix64's stream drawn as u32, each its word's high 32 bits.

#pragma once

#include <cstdint>

namespace xpute {

inline constexpr std::uint64_t SEED_SALT = 0x9e3779b97f4a7c15u;

constexpr std::uint64_t mix64(std::uint64_t x) noexcept {
    std::uint64_t z = x + SEED_SALT;
    z = (z ^ z >> 30) * 0xbf58476d1ce4e5b9u;
    z = (z ^ z >> 27) * 0x94d049bb133111ebu;
    return z ^ z >> 31;
}

constexpr std::uint64_t derive_seed_u64(std::uint64_t base, std::uint64_t tag) noexcept {
    return mix64(base ^ tag);
}

class SplitMix64 {
  public:
    constexpr explicit SplitMix64(std::uint64_t seed) noexcept : x_(seed) {}

    std::uint32_t next_u32() noexcept {
        x_ += SEED_SALT;
        std::uint64_t z = x_;
        z = (z ^ z >> 30) * 0xbf58476d1ce4e5b9u;
        z = (z ^ z >> 27) * 0x94d049bb133111ebu;
        z ^= z >> 31;
        // The high half, as java.util.random's nextInt and rand_xoshiro's
        // SplitMix64 take a u32 from a 64-bit word.
        return static_cast<std::uint32_t>(z >> 32);
    }

  private:
    std::uint64_t x_;
};

} // namespace xpute
