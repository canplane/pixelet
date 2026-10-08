// cpp/kit/wire/xtp/cursor.hpp

// Cursors over a packet; what a read hands back is a view of the packet's
// bytes, so the packet must outlive it. The packet need not be aligned in
// memory, but its nodes must be aligned within it.

#pragma once

#include <cstdint>
#include <optional>
#include <span>
#include <cstring>
#include <string_view>
#include <memory>
#include <optional>
#include <string>
#include <variant>
#include <vector>

#include "../../codec/encoding.hpp"
#include "../../status/bug.hpp"
#include "spec.hpp"

namespace xpute::xtp {

struct Null {
    bool operator==(const Null &) const = default;
};

template <class T> class Elements {
  public:
    Elements(std::span<const std::uint8_t> bytes) noexcept : bytes_(bytes) {}
    std::uint32_t size() const noexcept { return static_cast<std::uint32_t>(bytes_.size() / sizeof(T)); }
    T operator[](std::uint32_t i) const noexcept { return load<T>(bytes_.data() + i * sizeof(T)); }

  private:
    std::span<const std::uint8_t> bytes_;
};

struct Array {
    SequenceType type;
    std::span<const std::uint8_t> bytes;

    template <class T> Elements<T> elements() const noexcept { return bytes; }
};

// Low bit first.
struct Bitset {
    std::span<const std::uint8_t> packed;
    std::uint32_t len;

    bool operator[](std::uint32_t i) const noexcept { return (packed[i >> 3] >> (i & 7)) & 1; }
};

bool is_utf8(std::span<const std::uint8_t> bytes) noexcept;

struct Strs {
    std::span<const std::uint8_t> offs;
    std::span<const std::uint8_t> blob;
    std::uint32_t len;

    std::uint32_t size() const noexcept { return len; }
    bool empty() const noexcept { return len == 0; }

    std::optional<std::string_view> get(std::uint32_t i) const noexcept {
        if (i >= len) return std::nullopt;
        std::uint32_t a = load<std::uint32_t>(offs.data() + 4 * i), b = load<std::uint32_t>(offs.data() + 4 * (i + 1));
        return std::string_view(reinterpret_cast<const char *>(blob.data() + a), b - a);
    }
    std::string_view operator[](std::uint32_t i) const noexcept {
        std::optional<std::string_view> s = get(i);
        ensure(s.has_value(), Errno::enotrecoverable, i, len);
        return *s;
    }

    class iterator {
      public:
        std::string_view operator*() const noexcept { return (*s_)[i_]; }
        iterator &operator++() noexcept { return ++i_, *this; }
        bool operator==(const iterator &o) const noexcept { return i_ == o.i_; }

      private:
        friend struct Strs;
        iterator(const Strs *s, std::uint32_t i) noexcept : s_(s), i_(i) {}
        const Strs *s_;
        std::uint32_t i_;
    };
    iterator begin() const noexcept { return iterator(this, 0); }
    iterator end() const noexcept { return iterator(this, len); }
};

class BranchCursor;
template <class Alloc> struct DeepValue;

using Value = std::variant<Null, bool, std::uint8_t, std::int8_t, std::uint16_t, std::int16_t, std::uint32_t, std::int32_t, std::uint64_t, std::int64_t, float,
                           double, std::string_view, Array, Bitset, Strs, BranchCursor>;

class Cursor {
  public:
    NodeType type() const noexcept { return type_; }
    bool is_null() const noexcept { return base_ == 0; }
    bool is_branch() const noexcept { return !is_null() && type_ == type_of(SpecialType::branch); }

    std::optional<Cursor> opt() const noexcept { return is_null() ? std::nullopt : std::optional(*this); }

    Result<BranchCursor> as_branch() const noexcept;

    Result<Value> get() const noexcept;

    template <class T> Result<Elements<T>> get_array() const noexcept {
        if (is_null() || type_ != type_of(ARRAY_OF<T>)) return marshal_error(Errno::ebadmsg);
        Result<Array> a = array(sizeof(T));
        if (!a) return a.error();
        return Elements<T>(a->bytes);
    }

    Result<std::uint64_t> get_u64() const noexcept;
    Result<std::int64_t> get_i64() const noexcept;
    Result<bool> get_bool() const noexcept;
    Result<std::string_view> get_str_ref() const noexcept;
    // Invalid UTF-8 becomes U+FFFD, as the host decodes it.
    template <class Alloc = std::allocator<char>> Result<std::basic_string<char, std::char_traits<char>, Alloc>> get_str(const Alloc &alloc = Alloc()) const {
        Result<std::string_view> raw = str_of_type();
        if (!raw) return raw.error();
        std::basic_string<char, std::char_traits<char>, Alloc> out(alloc);
        utf8_lossy(*raw, [&out](std::string_view piece) { out.append(piece); });
        return out;
    }
    Result<Strs> get_strs() const noexcept;
    template <class Alloc = std::allocator<std::byte>> Result<DeepValue<Alloc>> get_deep(const Alloc &alloc = Alloc()) const;

    template <class Put> static void utf8_lossy(std::string_view s, Put &&put);

    // Not for 64-bit integers, which a double cannot hold exactly.
    Result<double> get_number() const noexcept;

  private:
    friend class TreeReader;
    friend class BranchCursor;

    Cursor(std::span<const std::uint8_t> pkt, std::uint32_t base, NodeType type, std::uint32_t len = 0) noexcept : pkt_(pkt), base_(base), type_(type), len_(len) {}

    static Result<Cursor> at(std::span<const std::uint8_t> pkt, std::uint32_t base, NodeType type) noexcept;

    template <class T> Result<T> get_as() const noexcept;
    template <class T> Result<Value> scalar() const noexcept;

    Result<Array> array(std::uint32_t elem) const noexcept;
    Result<Bitset> bitset() const noexcept;
    Result<std::string_view> str() const noexcept;
    Result<std::string_view> str_of_type() const noexcept;
    Result<Strs> strs() const noexcept;

    std::span<const std::uint8_t> pkt_;
    // 0 for an absent node; no node can start there.
    std::uint32_t base_;
    NodeType type_;
    std::uint32_t len_;
};

class BranchCursor {
  public:
    std::uint32_t base() const noexcept { return node_.base_; }
    std::uint32_t len() const noexcept { return node_.len_; }
    bool is_branch() const noexcept { return true; }
    Result<BranchCursor> as_branch() const noexcept { return *this; }
    std::optional<BranchCursor> opt() const noexcept { return *this; }

    Result<Value> get_at(std::uint32_t idx) const noexcept;
    template <class Vec> Result<void> get(Vec &out) const;

    class iterator {
      public:
        Result<Cursor> operator*() const noexcept { return b_->at(i_); }
        iterator &operator++() noexcept { return ++i_, *this; }
        bool operator==(const iterator &o) const noexcept { return i_ == o.i_; }

      private:
        friend class BranchCursor;
        iterator(const BranchCursor *b, std::uint32_t i) noexcept : b_(b), i_(i) {}
        const BranchCursor *b_;
        std::uint32_t i_;
    };
    iterator begin() const noexcept { return iterator(this, 0); }
    iterator end() const noexcept { return iterator(this, len()); }

    Result<Cursor> at(std::uint32_t idx) const noexcept;
    Result<BranchCursor> at_branch(std::uint32_t idx) const noexcept;

    bool operator==(const BranchCursor &o) const noexcept { return node_.pkt_.data() == o.node_.pkt_.data() && base() == o.base(); }

  private:
    friend class Cursor;
    explicit BranchCursor(Cursor node) noexcept : node_(node) {}

    Cursor node_;
};

class TreeReader {
  public:
    static Result<TreeReader> make(std::span<const std::uint8_t> pkt) noexcept;

    Cursor read() const noexcept { return root_; }
    Result<BranchCursor> read_branch() const noexcept { return root_.as_branch(); }

  private:
    explicit TreeReader(Cursor root) noexcept : root_(root) {}

    Cursor root_;
};

template <class Alloc> struct DeepValue {
    using List = std::vector<DeepValue, typename std::allocator_traits<Alloc>::template rebind_alloc<DeepValue>>;
    std::variant<Value, List> v;
};

template <class Alloc> Result<DeepValue<Alloc>> Cursor::get_deep(const Alloc &alloc) const {
    if (!is_branch()) {
        Result<Value> v = get();
        if (!v) return v.error();
        return DeepValue<Alloc>{*v};
    }
    Result<BranchCursor> b = as_branch();
    if (!b) return b.error();
    typename DeepValue<Alloc>::List list(alloc);
    list.reserve(b->len());
    for (Result<Cursor> c : *b) {
        if (!c) return c.error();
        Result<DeepValue<Alloc>> d = c->get_deep(alloc);
        if (!d) return d.error();
        list.push_back(std::move(*d));
    }
    return DeepValue<Alloc>{std::move(list)};
}

template <class Vec> Result<void> BranchCursor::get(Vec &out) const {
    for (Result<Cursor> c : *this) {
        if (!c) return c.error();
        Result<Value> v = c->get();
        if (!v) return v.error();
        out.push_back(*v);
    }
    return {};
}

template <class Put> void Cursor::utf8_lossy(std::string_view s, Put &&put) {
    ::xpute::utf8_lossy(s, std::forward<Put>(put));
}

} // namespace xpute::xtp
