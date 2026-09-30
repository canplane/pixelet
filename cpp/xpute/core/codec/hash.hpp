// cpp/xpute/core/codec/hash.hpp

// FNV-1a, 64 bits, as one word: the same word the TypeScript
// (@xpute/core codec/hash.ts), the Rust (the `fnv` crate) and the spec
// generator compute, so an id hashed anywhere is one number. Over bytes, and
// over a string's bytes as they are — a string here is already UTF-8.

#pragma once

#include <cstdint>
#include <span>
#include <string_view>

namespace xpute {

inline constexpr std::uint64_t FNV64_OFFSET = 0xcbf29ce484222325u;
inline constexpr std::uint64_t FNV64_PRIME = 0x100000001b3u;

constexpr std::uint64_t fnv1a64(std::span<const std::uint8_t> bytes) noexcept {
    std::uint64_t h = FNV64_OFFSET;
    for (std::uint8_t b : bytes) {
        h ^= b;
        h *= FNV64_PRIME;
    }
    return h;
}

constexpr std::uint64_t fnv1a64(std::string_view s) noexcept {
    std::uint64_t h = FNV64_OFFSET;
    for (char c : s) {
        h ^= static_cast<std::uint8_t>(c);
        h *= FNV64_PRIME;
    }
    return h;
}

} // namespace xpute
