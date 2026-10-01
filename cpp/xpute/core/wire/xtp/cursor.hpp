// cpp/xpute/core/wire/xtp/cursor.hpp

// The read end: cursors over a packet, and nothing read until a `get`. A
// cursor is a node's place and type; one physically absent keeps its type
// and reads as null. What a read hands back lies in the packet — a string,
// an array, a bitset are views of its bytes, and a branch is its cursor — so
// nothing is allocated and the packet outlives what is read from it. The
// other two can materialize a subtree whole (`get_deep`); here the packet
// is already that tree, and a caller walks it.
//
// The packet need not be aligned in memory: an element is loaded where it
// lies rather than viewed in place, which the other two need a word-aligned
// packet for. Its nodes must be aligned within it, as the format says.

#pragma once

#include <cstdint>
#include <optional>
#include <span>
#include <cstring>
#include <string_view>
#include <variant>

#include "spec.hpp"

namespace xpute::xtp {

struct Null {
    bool operator==(const Null &) const = default;
};

// A number array's elements, loaded one at a time.
template <class T> class Elements {
  public:
    Elements(std::span<const std::uint8_t> bytes) noexcept : bytes_(bytes) {}
    std::uint32_t size() const noexcept { return static_cast<std::uint32_t>(bytes_.size() / sizeof(T)); }
    T operator[](std::uint32_t i) const noexcept { return load<T>(bytes_.data() + i * sizeof(T)); }

  private:
    std::span<const std::uint8_t> bytes_;
};

// A sequence leaf of numbers: its type and its elements' bytes.
struct Array {
    SequenceType type;
    std::span<const std::uint8_t> bytes;

    template <class T> Elements<T> elements() const noexcept { return bytes; }
};

// A bitset's bits, packed low bit first.
struct Bitset {
    std::span<const std::uint8_t> packed;
    std::uint32_t len;

    bool operator[](std::uint32_t i) const noexcept { return (packed[i >> 3] >> (i & 7)) & 1; }
};

// A string sequence's strings, as they lie: `len` of them, each the span
// between two offsets into the UTF-8 after the offsets.
struct Strs {
    std::span<const std::uint8_t> offs;
    std::span<const std::uint8_t> blob;
    std::uint32_t len;

    std::string_view operator[](std::uint32_t i) const noexcept {
        std::uint32_t a, b;
        std::memcpy(&a, offs.data() + 4 * i, 4);
        std::memcpy(&b, offs.data() + 4 * (i + 1), 4);
        return std::string_view(reinterpret_cast<const char *>(blob.data() + a), b - a);
    }
};

class BranchCursor;

// What a read hands back: a leaf's value at the width it was written, null,
// or a branch's cursor.
using Value = std::variant<Null, bool, std::uint8_t, std::int8_t, std::uint16_t, std::int16_t, std::uint32_t, std::int32_t, std::uint64_t, std::int64_t, float,
                           double, std::string_view, Array, Bitset, Strs, BranchCursor>;

class Cursor {
  public:
    NodeType type() const noexcept { return type_; }
    bool is_null() const noexcept { return base_ == 0; }
    bool is_branch() const noexcept { return !is_null() && type_ == type_of(SpecialType::branch); }

    // None for a node physically absent.
    std::optional<Cursor> opt() const noexcept { return is_null() ? std::nullopt : std::optional(*this); }

    // A branch's cursor: EFAULT for an absent node, EBADMSG for a leaf.
    Result<BranchCursor> as_branch() const noexcept;

    Result<Value> get() const noexcept;

    // A number array's elements as the type it was written as: EBADMSG for
    // any other node, rather than the bytes read as another width.
    template <class T> Result<Elements<T>> get_array() const noexcept {
        if (is_null() || type_ != type_of(ARRAY_OF<T>)) return marshal_error(Errno::ebadmsg);
        Result<Array> a = array(sizeof(T));
        if (!a) return a.error();
        return Elements<T>(a->bytes);
    }

    Result<std::uint64_t> get_u64() const noexcept;
    Result<std::int64_t> get_i64() const noexcept;
    Result<bool> get_bool() const noexcept;
    // The bytes as they lie, without the NUL; whether they are UTF-8 is the
    // caller's to ask.
    Result<std::string_view> get_str() const noexcept;
    // A string sequence, its offsets checked to climb within its payload.
    Result<Strs> get_strs() const noexcept;

    // A scalar of 32 bits or fewer, or a float, as a double, which holds
    // every one of them exactly. A 64-bit integer is not one: get_u64/get_i64.
    Result<double> get_number() const noexcept;

  private:
    friend class TreeReader;
    friend class BranchCursor;

    Cursor(std::span<const std::uint8_t> pkt, std::uint32_t base, NodeType type, std::uint32_t len = 0) noexcept : pkt_(pkt), base_(base), type_(type), len_(len) {}

    // The node of `type` at `base`, checked for its place and, a branch, for
    // its table.
    static Result<Cursor> at(std::span<const std::uint8_t> pkt, std::uint32_t base, NodeType type) noexcept;

    // Defined where they are used, once Value is whole.
    template <class T> Result<T> get_as() const noexcept;
    template <class T> Result<Value> scalar() const noexcept;

    // A sequence's payload, checked to lie in the packet.
    Result<Array> array(std::uint32_t elem) const noexcept;
    Result<Bitset> bitset() const noexcept;
    Result<std::string_view> str() const noexcept;
    Result<Strs> strs() const noexcept;

    std::span<const std::uint8_t> pkt_;
    // 0 for a node physically absent, where no node can start.
    std::uint32_t base_;
    NodeType type_;
    // A branch's child count.
    std::uint32_t len_;
};

class BranchCursor {
  public:
    std::uint32_t base() const noexcept { return node_.base_; }
    std::uint32_t len() const noexcept { return node_.len_; }

    // The child at `idx`: EFAULT past the end, EBADMSG for an offset that
    // does not point past the table and into the packet.
    Result<Cursor> at(std::uint32_t idx) const noexcept;
    Result<BranchCursor> at_branch(std::uint32_t idx) const noexcept;

    bool operator==(const BranchCursor &o) const noexcept { return node_.pkt_.data() == o.node_.pkt_.data() && base() == o.base(); }

  private:
    friend class Cursor;
    explicit BranchCursor(Cursor node) noexcept : node_(node) {}

    Cursor node_;
};

// A packet's header, checked, and its root.
class TreeReader {
  public:
    static Result<TreeReader> make(std::span<const std::uint8_t> pkt) noexcept;

    Cursor read() const noexcept { return root_; }
    Result<BranchCursor> read_branch() const noexcept { return root_.as_branch(); }

  private:
    explicit TreeReader(Cursor root) noexcept : root_(root) {}

    Cursor root_;
};

} // namespace xpute::xtp
