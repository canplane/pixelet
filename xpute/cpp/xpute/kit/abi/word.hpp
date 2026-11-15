// cpp/xpute/kit/abi/word.hpp

// These are bit fields in 32- and 64-bit words. A field is `nbit` bits wide and
// starts at bit `shamt`; the caller keeps it inside the word. `field_mask` also
// takes a field as wide as the whole word, which a plain shift cannot express.
// A 64-bit word is also two 32-bit ones, its low word first (`split64`,
// `join64`), as a stream of 32-bit words carries it.

#pragma once

#include <array>
#include <concepts>
#include <cstdint>
#include <limits>
#include <type_traits>

namespace xpute {

// The two 32-bit words of `v`, its low word first.
constexpr std::array<std::uint32_t, 2> split64(std::uint64_t v) noexcept {
    return {static_cast<std::uint32_t>(v), static_cast<std::uint32_t>(v >> 32)};
}

// The 64-bit word whose low and high 32-bit words are `lo` and `hi`.
constexpr std::uint64_t join64(std::uint32_t lo, std::uint32_t hi) noexcept {
    return std::uint64_t{hi} << 32 | lo;
}

template <class W>
concept Word = std::same_as<W, std::uint32_t> || std::same_as<W, std::uint64_t>;

template <Word W> constexpr W bit(unsigned shamt) noexcept {
    return W{1} << shamt;
}

template <Word W> constexpr W field_mask(unsigned nbit, unsigned shamt) noexcept {
    constexpr unsigned bits = std::numeric_limits<W>::digits;
    W field = nbit >= bits ? std::numeric_limits<W>::max() : (W{1} << nbit) - 1;
    return shamt >= bits ? W{0} : W(field << shamt);
}

template <Word W> constexpr W field_get(W x, unsigned shamt, W mask) noexcept {
    return x >> shamt & mask;
}

template <Word W> constexpr W field_set(W x, unsigned shamt, W mask, W v) noexcept {
    return W((x & ~W(mask << shamt)) | W((v & mask) << shamt));
}

template <Word W> constexpr std::make_signed_t<W> as_int_n(unsigned nbit, W x) noexcept {
    constexpr unsigned bits = std::numeric_limits<W>::digits;
    return std::make_signed_t<W>(W(x << (bits - nbit))) >> (bits - nbit);
}

} // namespace xpute
