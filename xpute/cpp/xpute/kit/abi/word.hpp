// cpp/xpute/kit/abi/word.hpp

// Bit fields in 32- and 64-bit words. A field is `nbit` bits wide and starts
// at bit `shamt`; the caller keeps it inside the word. `field_mask` also takes
// a field as wide as the whole word, which a plain shift cannot express.

#pragma once

#include <concepts>
#include <cstdint>
#include <limits>
#include <type_traits>

namespace xpute {

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
