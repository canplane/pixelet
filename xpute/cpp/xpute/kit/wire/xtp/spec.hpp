// cpp/xpute/kit/wire/xtp/spec.hpp

// XTP, the Xpute Tree Packet, is a tree of values in one buffer, read in place.
// This header holds its layout, its type byte, and the helpers that read and
// write a packet's words. The format is described in
// kit/ts/src/wire/xtp/README.md, and the golden records under
// kit/spec/golden/wire/xtp/ hold the C++, Rust and TypeScript sides to the same
// bytes.
//
// A packet is a 16-byte header and one root node, all little-endian. A len,
// an offset or a size takes an 8-byte slot, the value in its low half; the
// high half is reserved and zero, or holds a type.
//
//   header  word 0  [MAGIC 32 | reserved 32]
//           word 1  [root payload size 32 | type 8 | reserved 24]
//   branch  [child count 32 | reserved 32], then a descriptor for each child:
//           [offset from the branch's start 32 | type 8 | reserved 24]
//
// A child's offset counts from its branch's start, so a subtree can be copied
// into another packet unchanged. An absent node keeps its type, with a size or
// an offset of 0. A present child starts past its branch's table, before the
// next present sibling, and inside its parent; a reader refuses a packet that
// breaks this.

#pragma once

#include <algorithm>
#include <array>
#include <bit>
#include <cstdint>
#include <cstring>
#include <span>
#include <type_traits>

#include "xpute/kit/status/error.hpp"

namespace xpute::xtp {

using NodeType = std::uint8_t;

// A type byte with its top bit set names a leaf:
//
//   7 | 6 | 5 | 4 3 | 2 1 0
//   L | R | S |  C  |   A
//
// L is set for a leaf, and R is reserved. S is set for a sequence and clear
// for a scalar. C is the class: unsigned, signed, float, or non-numeric. A is
// the argument: for a number, log2 of its width in bytes; for a non-numeric
// leaf, which one it is (bool or bitset, str, strs). A type byte with the top
// bit clear is a special type: nil, branch or graft.
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
    // UTF-8 bytes followed by a NUL, which `len` does not count.
    str = BIT_SEQ | nonnum_type(1),
    // Many strings in one leaf, so that a string costs no node of its own:
    // `len` + 1 u32 offsets, each no less than the one before, then the UTF-8
    // they index. String `i` lies between offsets `i` and `i + 1`.
    strs = BIT_SEQ | nonnum_type(2),
};

enum class SpecialType : NodeType {
    // The untyped null. An absent node is not nil: it keeps its own type.
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

// "XTP\0" read as a little-endian u32.
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

inline void set_word(std::span<std::uint8_t> buf, std::uint32_t off, std::uint32_t lo, std::uint32_t hi) noexcept {
    store(buf.data() + off, lo);
    store(buf.data() + off + 4, hi);
}
inline std::array<std::uint32_t, 2> get_word(std::span<const std::uint8_t> buf, std::uint32_t off) noexcept {
    return {load<std::uint32_t>(buf.data() + off), load<std::uint32_t>(buf.data() + off + 4)};
}

// Whether a type byte may appear on the wire: nil, branch, or a leaf with no
// reserved bit set and an argument its class defines. A graft may not.
constexpr bool type_is_known(NodeType t) noexcept {
    if (t == type_of(SpecialType::nil) || t == type_of(SpecialType::branch)) return true;
    if (!type_is_leaf(t) || (t & ~(BIT_LEAF | BIT_SEQ | CLASS_MASK | ARG_MASK)) != 0) return false;
    std::uint32_t arg = type_arg_of(t);
    switch (t & CLASS_MASK) {
    case CLASS_F: return arg == 2 || arg == 3;
    case CLASS_R: return arg <= (type_is_seq(t) ? 2u : 0u);
    default: return arg <= 3;
    }
}

// Reads a slot that holds a len, a size or the magic. A high half that is not
// zero makes the packet malformed (EBADMSG).
inline Result<std::uint32_t> get_len(std::span<const std::uint8_t> buf, std::uint32_t off) noexcept {
    auto [lo, hi] = get_word(buf, off);
    if (hi != RESERVED) return make_error(Errno::ebadmsg);
    return lo;
}

// Reads a descriptor, a branch's entry for a child or the header's root word:
// the offset or size, and the type. A type the wire cannot carry, or a
// reserved bit set, makes the packet malformed (EBADMSG).
struct Desc {
    std::uint32_t off;
    NodeType type;
};
inline Result<Desc> get_desc(std::span<const std::uint8_t> buf, std::uint32_t off) noexcept {
    auto [lo, desc] = get_word(buf, off);
    auto type = static_cast<NodeType>(desc & DESC_TYPE_MASK);
    if (desc != type || !type_is_known(type)) return make_error(Errno::ebadmsg);
    return Desc{lo, type};
}

// `unit` must be a power of two no larger than `WORD_SZ`, or EINVAL.
inline Result<std::uint32_t> align(std::uint32_t nbyte, std::uint32_t unit) noexcept {
    if (unit == 0 || (unit & (unit - 1)) != 0 || unit > WORD_SZ) return make_error(Errno::einval);
    std::uint32_t mask = unit - 1;
    if (nbyte > UINT32_MAX - mask) return make_error(Errno::eoverflow);
    return (nbyte + mask) & ~mask;
}

// A sequence or a branch starts on a word, so its len slot is aligned.
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
    return make_error(Errno::ebadmsg);
}

// One element's size. A strs's element is a u32 offset, and a bitset's is a
// byte, since its bits are given a byte each before they are packed.
constexpr std::uint32_t elem_sz(SequenceType t) noexcept {
    if (t == SequenceType::strs) return 4;
    return type_is_nonnum(type_of(t)) ? 1 : 1u << type_arg_of(type_of(t));
}

} // namespace xpute::xtp
