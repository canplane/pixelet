// cpp/xpute/core/math/rng.hpp

// Seeds and the stream drawn from one: reproducible, and for shuffling,
// sampling and scatter — nothing that must resist prediction. splitmix64,
// drawn the way the other two languages draw it (golden/math/rng.tsv).

#pragma once

#include <cmath>
#include <cstdint>

namespace xpute {

inline constexpr std::uint64_t SEED_SALT = 0x9e3779b97f4a7c15u;

// splitmix64's finalizer: enough avalanche that child seeds drawn from one
// parent are independent of each other.
constexpr std::uint64_t mix64(std::uint64_t x) noexcept {
    std::uint64_t z = x + SEED_SALT;
    z = (z ^ z >> 30) * 0xbf58476d1ce4e5b9u;
    z = (z ^ z >> 27) * 0x94d049bb133111ebu;
    return z ^ z >> 31;
}

// A child seed: independent of its siblings, and the same one every run for
// the same parent and tag.
constexpr std::uint64_t derive_seed_u64(std::uint64_t base, std::uint64_t tag) noexcept {
    return mix64(base ^ tag);
}

// A splitmix64 stream drawn as u32: eight bytes, kept on a caller's stack.
class SplitMix64 {
  public:
    constexpr explicit SplitMix64(std::uint64_t seed) noexcept : x_(seed) {}

    std::uint32_t next_u32() noexcept {
        x_ += SEED_SALT;
        std::uint64_t z = x_;
        z = (z ^ z >> 30) * 0xbf58476d1ce4e5b9u;
        z = (z ^ z >> 27) * 0x94d049bb133111ebu;
        z ^= z >> 31;
        // Not the word's low bits: the draw goes through a double, which
        // rounds the word to the nearest one before the modulo, so past 2^53
        // it is not. That is how JavaScript draws it and so how every
        // language does.
        return static_cast<std::uint32_t>(std::fmod(static_cast<double>(z), 4294967296.0));
    }

  private:
    std::uint64_t x_;
};

// A draw as a uniform double in [0, 1).
constexpr double u32_to_unit(std::uint32_t x) noexcept {
    return static_cast<double>(x) / 4294967296.0;
}

} // namespace xpute
