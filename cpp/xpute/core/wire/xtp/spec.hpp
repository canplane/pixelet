// cpp/xpute/core/wire/xtp/spec.hpp

// XTP, a relocatable packet of one tree; the golden records hold every
// language's encoding to the same bytes.
//
//   word0  [MAGIC 32 | reserved 32]
//   word1  [root payload bytes 32 | type 8 | reserved 24]
//
// A branch is an 8-byte child count, then one word per child (offset in the
// low half, type in the next byte). An absent node keeps its type with a size
// or offset of 0.

#pragma once

#include <algorithm>
#include <array>
#include <bit>
#include <cstdint>
#include <cstring>
#include <span>
#include <type_traits>

#include "../../status/error.hpp"

namespace xpute::xtp {

using NodeType = std::uint8_t;

// A leaf's type byte, when its top bit is set:
//
//   7 | 6 | 5 | 4 3 | 2 1 0
//   L | R | S |  C  |   A
//
// L leaf, R reserved, S sequence (else scalar), C class (unsigned, signed,
// float, neither), A its argument: log2 of a number's width in bytes.
inline constexpr NodeType BIT_LEAF = 0x80;
inline constexpr NodeType BIT_SEQ = 0x20;
inline constexpr NodeType CLASS_MASK = 0x18;
inline constexpr NodeType CLASS_U = 0x00;
inline constexpr NodeType CLASS_I = 0x08;
inline constexpr NodeType CLASS_F = 0x10;
inline constexpr NodeType CLASS_R = 0x18;
inline constexpr NodeType ARG_MASK = 0x07;

constexpr NodeType int_type(std::uint32_t log2_width, bool sign) noexcept {
    return BIT_LEAF | (sign ? CLASS_I : CLASS_U) | (log2_width & ARG_MASK);
}
constexpr NodeType float_type(std::uint32_t log2_width) noexcept {
    return BIT_LEAF | CLASS_F | (log2_width & ARG_MASK);
}
constexpr NodeType nonnum_type(std::uint32_t x) noexcept {
    return BIT_LEAF | CLASS_R | (x & ARG_MASK);
}

enum class ScalarType : NodeType {
    u8 = int_type(0, false),
    i8 = int_type(0, true),
    u16 = int_type(1, false),
    i16 = int_type(1, true),
    u32 = int_type(2, false),
    i32 = int_type(2, true),
    u64 = int_type(3, false),
    i64 = int_type(3, true),
    f32 = float_type(2),
    f64 = float_type(3),
    boolean = nonnum_type(0),
};

enum class SequenceType : NodeType {
    u8_array = BIT_SEQ | int_type(0, false),
    i8_array = BIT_SEQ | int_type(0, true),
    u16_array = BIT_SEQ | int_type(1, false),
    i16_array = BIT_SEQ | int_type(1, true),
    u32_array = BIT_SEQ | int_type(2, false),
    i32_array = BIT_SEQ | int_type(2, true),
    u64_array = BIT_SEQ | int_type(3, false),
    i64_array = BIT_SEQ | int_type(3, true),
    f32_array = BIT_SEQ | float_type(2),
    f64_array = BIT_SEQ | float_type(3),
    bitset = BIT_SEQ | nonnum_type(0),
    str = BIT_SEQ | nonnum_type(1),
    // len + 1 offsets into the UTF-8 after them.
    strs = BIT_SEQ | nonnum_type(2),
};

enum class SpecialType : NodeType {
    nil = 0x00,
    branch = 0x20,
    // Never on the wire: a graft is written as its packet's root, under that
    // root's type.
    graft = 0x40,
};

template <class E> constexpr NodeType type_of(E t) noexcept {
    return static_cast<NodeType>(t);
}

constexpr bool type_is_leaf(NodeType t) noexcept {
    return (t & BIT_LEAF) != 0;
}
constexpr bool type_is_special(NodeType t) noexcept {
    return !type_is_leaf(t);
}
constexpr bool type_is_scalar(NodeType t) noexcept {
    return type_is_leaf(t) && (t & BIT_SEQ) == 0;
}
constexpr bool type_is_seq(NodeType t) noexcept {
    return type_is_leaf(t) && (t & BIT_SEQ) != 0;
}
constexpr bool type_is_nonnum(NodeType t) noexcept {
    return type_is_leaf(t) && (t & CLASS_MASK) == CLASS_R;
}
constexpr bool type_is_float(NodeType t) noexcept {
    return type_is_leaf(t) && (t & CLASS_MASK) == CLASS_F;
}
constexpr bool type_is_signed(NodeType t) noexcept {
    return type_is_leaf(t) && (t & CLASS_MASK) == CLASS_I;
}
constexpr std::uint32_t type_arg_of(NodeType t) noexcept {
    return t & ARG_MASK;
}

template <class T> inline constexpr SequenceType ARRAY_OF = [] {
    if constexpr (std::is_same_v<T, std::uint8_t>) return SequenceType::u8_array;
    else if constexpr (std::is_same_v<T, std::int8_t>) return SequenceType::i8_array;
    else if constexpr (std::is_same_v<T, std::uint16_t>) return SequenceType::u16_array;
    else if constexpr (std::is_same_v<T, std::int16_t>) return SequenceType::i16_array;
    else if constexpr (std::is_same_v<T, std::uint32_t>) return SequenceType::u32_array;
    else if constexpr (std::is_same_v<T, std::int32_t>) return SequenceType::i32_array;
    else if constexpr (std::is_same_v<T, std::uint64_t>) return SequenceType::u64_array;
    else if constexpr (std::is_same_v<T, std::int64_t>) return SequenceType::i64_array;
    else if constexpr (std::is_same_v<T, float>) return SequenceType::f32_array;
    else {
        static_assert(std::is_same_v<T, double>, "no array of this element on the wire");
        return SequenceType::f64_array;
    }
}();

// "XTP\0" read little-endian.
inline constexpr std::uint32_t MAGIC = 0x00505458u;
inline constexpr std::uint32_t WORD_SZ = 8;
inline constexpr std::uint32_t HDR_SZ = 2 * WORD_SZ;
inline constexpr std::uint32_t MAX_PKT_SZ = 0x7fffffffu;
inline constexpr std::uint32_t RESERVED = 0;
inline constexpr std::uint32_t DESC_TYPE_MASK = 0xffu;

// A little-endian `T` at `p`, which need not be aligned.
template <class T> T load(const std::uint8_t *p) noexcept {
    std::array<std::uint8_t, sizeof(T)> raw;
    std::memcpy(raw.data(), p, sizeof(T));
    if constexpr (std::endian::native == std::endian::big) std::reverse(raw.begin(), raw.end());
    return std::bit_cast<T>(raw);
}

template <class T> void store(std::uint8_t *p, T x) noexcept {
    auto raw = std::bit_cast<std::array<std::uint8_t, sizeof(T)>>(x);
    if constexpr (std::endian::native == std::endian::big) std::reverse(raw.begin(), raw.end());
    std::memcpy(p, raw.data(), sizeof(T));
}

// Low half first.
inline void set_word(std::span<std::uint8_t> buf, std::uint32_t off, std::uint32_t lo, std::uint32_t hi) noexcept {
    store(buf.data() + off, lo);
    store(buf.data() + off + 4, hi);
}
inline std::array<std::uint32_t, 2> get_word(std::span<const std::uint8_t> buf, std::uint32_t off) noexcept {
    return {load<std::uint32_t>(buf.data() + off), load<std::uint32_t>(buf.data() + off + 4)};
}

// `unit` is a power of two no wider than a word.
inline Result<std::uint32_t> align(std::uint32_t nbyte, std::uint32_t unit) noexcept {
    if (unit == 0 || (unit & (unit - 1)) != 0 || unit > WORD_SZ) return marshal_error(Errno::einval);
    std::uint32_t mask = unit - 1;
    if (nbyte > UINT32_MAX - mask) return marshal_error(Errno::eoverflow);
    return (nbyte + mask) & ~mask;
}

// A sequence or a branch starts on a word so its leading slot is one.
inline Result<std::uint32_t> align_sz(NodeType t) noexcept {
    if (t == type_of(SpecialType::branch) || type_is_seq(t)) return WORD_SZ;
    switch (static_cast<ScalarType>(t)) {
    case ScalarType::u8:
    case ScalarType::i8:
    case ScalarType::boolean:
        return 1u;
    case ScalarType::u16:
    case ScalarType::i16:
        return 2u;
    case ScalarType::u32:
    case ScalarType::i32:
    case ScalarType::f32:
        return 4u;
    case ScalarType::u64:
    case ScalarType::i64:
    case ScalarType::f64:
        return 8u;
    }
    return marshal_error(Errno::ebadmsg);
}

// A string sequence's element is its offsets' word.
constexpr std::uint32_t elem_sz(SequenceType t) noexcept {
    if (t == SequenceType::strs) return 4;
    return type_is_nonnum(type_of(t)) ? 1 : 1u << type_arg_of(type_of(t));
}

} // namespace xpute::xtp
