// cpp/xpute/core/abi/word.hpp

// The bit field, in either word width the formats use: a contiguous mask,
// the get and the set through one, one bit, and a field's low bits read as a
// signed number. What the language gives — the widths, their limits and the
// casts — is its own. A width of the whole word is the whole word, which the
// shift alone cannot say; `nbit` and `shamt` are otherwise the caller's to
// keep inside the word.

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

// A contiguous mask of `nbit` bits starting at `shamt`.
template <Word W> constexpr W field_mask(unsigned nbit, unsigned shamt) noexcept {
    constexpr unsigned bits = std::numeric_limits<W>::digits;
    W field = nbit >= bits ? std::numeric_limits<W>::max() : (W{1} << nbit) - 1;
    return shamt >= bits ? W{0} : W(field << shamt);
}

// The field `mask` selects at `shamt`, shifted down.
template <Word W> constexpr W field_get(W x, unsigned shamt, W mask) noexcept {
    return x >> shamt & mask;
}

// `x` with the field `mask` selects at `shamt` replaced by `v`'s low bits.
template <Word W> constexpr W field_set(W x, unsigned shamt, W mask, W v) noexcept {
    return W((x & ~W(mask << shamt)) | W((v & mask) << shamt));
}

// The low `nbit` bits of `x`, sign-extended: a signed field read out of a
// word.
template <Word W> constexpr std::make_signed_t<W> as_int_n(unsigned nbit, W x) noexcept {
    constexpr unsigned bits = std::numeric_limits<W>::digits;
    return std::make_signed_t<W>(W(x << (bits - nbit))) >> (bits - nbit);
}

} // namespace xpute
