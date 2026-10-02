// cpp/xpute/core/wire/xtp/view.hpp

// The write end as a tree. A node borrows what it carries (an array, a string,
// a graft's packet), so the caller keeps them alive until the tree is encoded.

#pragma once

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <memory>
#include <optional>
#include <span>
#include <string_view>
#include <variant>
#include <vector>

#include "../../status/error.hpp"
#include "encode.hpp"
#include "spec.hpp"

namespace xpute::xtp {

enum class NodeKind : std::uint8_t { nil, scalar, array, bitset, str, strs, branch, graft };

template <class Alloc = std::allocator<std::byte>> struct Node {
    using Children = std::vector<Node, typename std::allocator_traits<Alloc>::template rebind_alloc<Node>>;

    NodeKind kind = NodeKind::nil;
    NodeType type = type_of(SpecialType::nil);
    // A typed null keeps its type and is absent.
    bool present = true;
    // Little end first.
    std::uint64_t bits = 0;
    std::span<const std::uint8_t> bytes;
    std::string_view str;
    std::span<const std::string_view> strs;
    Children children;

    explicit Node(const Alloc &alloc = Alloc()) : children(alloc) {}
};

namespace view_detail {

template <class Alloc, class T> Node<Alloc> scalar(ScalarType type, std::optional<T> v, const Alloc &alloc) {
    Node<Alloc> n(alloc);
    n.kind = NodeKind::scalar;
    n.type = type_of(type);
    n.present = v.has_value();
    if (v) std::memcpy(&n.bits, &*v, sizeof(T));
    return n;
}

template <class Alloc, class T> Node<Alloc> array(std::optional<std::span<const T>> a, const Alloc &alloc) {
    Node<Alloc> n(alloc);
    n.kind = NodeKind::array;
    n.type = type_of(ARRAY_OF<T>);
    n.present = a.has_value();
    if (a) n.bytes = std::as_bytes(*a).empty() ? std::span<const std::uint8_t>() : std::span<const std::uint8_t>(reinterpret_cast<const std::uint8_t *>(a->data()), a->size_bytes());
    return n;
}

} // namespace view_detail

template <class Alloc> class BranchView;

template <class Alloc = std::allocator<std::byte>> struct ViewValue {
    struct Null {};
    struct Array {
        SequenceType type;
        std::span<const std::uint8_t> bytes;
    };
    std::variant<Null, bool, std::uint8_t, std::int8_t, std::uint16_t, std::int16_t, std::uint32_t, std::int32_t, std::uint64_t, std::int64_t, float, double,
                 std::string_view, Array, std::span<const ViewValue>, const Node<Alloc> *>
        v;
};

// `Self::write` replaces a tree's root or appends a branch's child.
template <class Self, class Alloc> class NodeView {
  public:
    Self &nil() { return self().write(Node<Alloc>(alloc())); }

    Self &u8(std::optional<std::uint8_t> v) { return self().write(view_detail::scalar(ScalarType::u8, v, alloc())); }
    Self &i8(std::optional<std::int8_t> v) { return self().write(view_detail::scalar(ScalarType::i8, v, alloc())); }
    Self &u16(std::optional<std::uint16_t> v) { return self().write(view_detail::scalar(ScalarType::u16, v, alloc())); }
    Self &i16(std::optional<std::int16_t> v) { return self().write(view_detail::scalar(ScalarType::i16, v, alloc())); }
    Self &u32(std::optional<std::uint32_t> v) { return self().write(view_detail::scalar(ScalarType::u32, v, alloc())); }
    Self &i32(std::optional<std::int32_t> v) { return self().write(view_detail::scalar(ScalarType::i32, v, alloc())); }
    Self &u64(std::optional<std::uint64_t> v) { return self().write(view_detail::scalar(ScalarType::u64, v, alloc())); }
    Self &i64(std::optional<std::int64_t> v) { return self().write(view_detail::scalar(ScalarType::i64, v, alloc())); }
    Self &f32(std::optional<float> v) { return self().write(view_detail::scalar(ScalarType::f32, v, alloc())); }
    Self &f64(std::optional<double> v) { return self().write(view_detail::scalar(ScalarType::f64, v, alloc())); }
    Self &boolean(std::optional<bool> v) {
        return self().write(view_detail::scalar(ScalarType::boolean, v ? std::optional<std::uint8_t>(*v ? 1 : 0) : std::nullopt, alloc()));
    }

    Self &u8_array(std::optional<std::span<const std::uint8_t>> a) { return self().write(view_detail::array(a, alloc())); }
    Self &i8_array(std::optional<std::span<const std::int8_t>> a) { return self().write(view_detail::array(a, alloc())); }
    Self &u16_array(std::optional<std::span<const std::uint16_t>> a) { return self().write(view_detail::array(a, alloc())); }
    Self &i16_array(std::optional<std::span<const std::int16_t>> a) { return self().write(view_detail::array(a, alloc())); }
    Self &u32_array(std::optional<std::span<const std::uint32_t>> a) { return self().write(view_detail::array(a, alloc())); }
    Self &i32_array(std::optional<std::span<const std::int32_t>> a) { return self().write(view_detail::array(a, alloc())); }
    Self &u64_array(std::optional<std::span<const std::uint64_t>> a) { return self().write(view_detail::array(a, alloc())); }
    Self &i64_array(std::optional<std::span<const std::int64_t>> a) { return self().write(view_detail::array(a, alloc())); }
    Self &f32_array(std::optional<std::span<const float>> a) { return self().write(view_detail::array(a, alloc())); }
    Self &f64_array(std::optional<std::span<const double>> a) { return self().write(view_detail::array(a, alloc())); }

    // Bits given a byte each, nonzero for set.
    Self &bitset(std::optional<std::span<const std::uint8_t>> bits) {
        Node<Alloc> n(alloc());
        n.kind = NodeKind::bitset;
        n.type = type_of(SequenceType::bitset);
        n.present = bits.has_value();
        if (bits) n.bytes = *bits;
        return self().write(std::move(n));
    }

    Self &str(std::optional<std::string_view> s) {
        Node<Alloc> n(alloc());
        n.kind = NodeKind::str;
        n.type = type_of(SequenceType::str);
        n.present = s.has_value();
        if (s) n.str = *s;
        return self().write(std::move(n));
    }

    Self &strs(std::optional<std::span<const std::string_view>> items) {
        Node<Alloc> n(alloc());
        n.kind = NodeKind::strs;
        n.type = type_of(SequenceType::strs);
        n.present = items.has_value();
        if (items) n.strs = *items;
        return self().write(std::move(n));
    }

    // With no `fill`, an absent branch that keeps its type.
    template <class F> Self &branch(F &&fill) {
        BranchView<Alloc> b(alloc());
        fill(b);
        return self().write(std::move(b.node()));
    }
    Self &no_branch() {
        Node<Alloc> n(alloc());
        n.kind = NodeKind::branch;
        n.type = type_of(SpecialType::branch);
        n.present = false;
        return self().write(std::move(n));
    }

    // Only the construction is checked here; the header is read at encode.
    Self &graft(std::span<const std::uint8_t> packet) {
        Node<Alloc> n(alloc());
        n.kind = NodeKind::graft;
        n.type = type_of(SpecialType::graft);
        n.bytes = packet;
        return self().write(std::move(n));
    }

  protected:
    Result<Node<Alloc>> lower(const ViewValue<Alloc> &val) {
        using V = ViewValue<Alloc>;
        const Alloc &a = alloc();
        return std::visit(
            [&](const auto &x) -> Result<Node<Alloc>> {
                using T = std::decay_t<decltype(x)>;
                if constexpr (std::is_same_v<T, typename V::Null>) return Node<Alloc>(a);
                else if constexpr (std::is_same_v<T, bool>) return view_detail::scalar(ScalarType::boolean, std::optional<std::uint8_t>(x ? 1 : 0), a);
                else if constexpr (std::is_same_v<T, std::uint8_t>) return view_detail::scalar(ScalarType::u8, std::optional(x), a);
                else if constexpr (std::is_same_v<T, std::int8_t>) return view_detail::scalar(ScalarType::i8, std::optional(x), a);
                else if constexpr (std::is_same_v<T, std::uint16_t>) return view_detail::scalar(ScalarType::u16, std::optional(x), a);
                else if constexpr (std::is_same_v<T, std::int16_t>) return view_detail::scalar(ScalarType::i16, std::optional(x), a);
                else if constexpr (std::is_same_v<T, std::uint32_t>) return view_detail::scalar(ScalarType::u32, std::optional(x), a);
                else if constexpr (std::is_same_v<T, std::int32_t>) return view_detail::scalar(ScalarType::i32, std::optional(x), a);
                else if constexpr (std::is_same_v<T, std::uint64_t>) return view_detail::scalar(ScalarType::u64, std::optional(x), a);
                else if constexpr (std::is_same_v<T, std::int64_t>) return view_detail::scalar(ScalarType::i64, std::optional(x), a);
                else if constexpr (std::is_same_v<T, float>) return view_detail::scalar(ScalarType::f32, std::optional(x), a);
                else if constexpr (std::is_same_v<T, double>) return view_detail::scalar(ScalarType::f64, std::optional(x), a);
                else if constexpr (std::is_same_v<T, std::string_view>) {
                    Node<Alloc> n(a);
                    n.kind = NodeKind::str;
                    n.type = type_of(SequenceType::str);
                    n.str = x;
                    return n;
                } else if constexpr (std::is_same_v<T, typename V::Array>) {
                    Node<Alloc> n(a);
                    n.kind = x.type == SequenceType::bitset ? NodeKind::bitset : NodeKind::array;
                    n.type = type_of(x.type);
                    n.bytes = x.bytes;
                    return n;
                } else if constexpr (std::is_same_v<T, std::span<const V>>) {
                    BranchView<Alloc> b(a);
                    for (const V &child : x) {
                        Result<Node<Alloc>> c = b.lower(child);
                        if (!c) return c.error();
                        b.write(std::move(*c));
                    }
                    return std::move(b.node());
                } else {
                    return *x;
                }
            },
            val.v);
    }

  private:
    Self &self() { return static_cast<Self &>(*this); }
    const Alloc &alloc() { return static_cast<Self &>(*this).allocator(); }
};

template <class Alloc = std::allocator<std::byte>> class TreeView : public NodeView<TreeView<Alloc>, Alloc> {
  public:
    explicit TreeView(const Alloc &alloc = Alloc()) : alloc_(alloc), node_(alloc) {}

    TreeView &write(Node<Alloc> node) {
        node_ = std::move(node);
        return *this;
    }

    Result<TreeView *> set(const ViewValue<Alloc> &val) {
        Result<Node<Alloc>> n = this->lower(val);
        if (!n) return n.error();
        write(std::move(*n));
        return this;
    }

    const Node<Alloc> &node() const noexcept { return node_; }
    const Alloc &allocator() const noexcept { return alloc_; }

  private:
    Alloc alloc_;
    Node<Alloc> node_;
};

template <class Alloc = std::allocator<std::byte>> class BranchView : public NodeView<BranchView<Alloc>, Alloc> {
  public:
    explicit BranchView(const Alloc &alloc = Alloc()) : alloc_(alloc), node_(alloc) {
        node_.kind = NodeKind::branch;
        node_.type = type_of(SpecialType::branch);
    }

    BranchView &write(Node<Alloc> node) {
        node_.children.push_back(std::move(node));
        return *this;
    }

    Result<BranchView *> set(std::span<const ViewValue<Alloc>> vals) {
        node_.children.clear();
        for (const ViewValue<Alloc> &val : vals) {
            Result<BranchView *> r = put(val);
            if (!r) return r;
        }
        return this;
    }

    Result<BranchView *> put(const ViewValue<Alloc> &val) {
        Result<Node<Alloc>> n = this->lower(val);
        if (!n) return n.error();
        write(std::move(*n));
        return this;
    }

    template <class V> BranchView &subtree(const V &view) { return write(view.node()); }

    Node<Alloc> &node() noexcept { return node_; }
    const Node<Alloc> &node() const noexcept { return node_; }
    const Alloc &allocator() const noexcept { return alloc_; }

  private:
    template <class, class> friend class NodeView;
    Alloc alloc_;
    Node<Alloc> node_;
};

struct TreeEncoderOptions {
    std::optional<std::uint32_t> init_cap;
    std::optional<std::uint32_t> max_cap;
};

namespace view_detail {

template <class T> T load(const std::uint8_t *p) noexcept {
    T v;
    std::memcpy(&v, p, sizeof v);
    return v;
}

template <class T, class Alloc> void array_into(PacketWriter &w, const Node<Alloc> &n) {
    if (!n.present) {
        w.template array_with<T>(0, [](std::uint32_t) { return T{}; });
        return;
    }
    auto len = static_cast<std::uint32_t>(n.bytes.size() / sizeof(T));
    w.template array_with<T>(len, [&n](std::uint32_t i) { return load<T>(n.bytes.data() + i * sizeof(T)); });
}

template <class Alloc> void walk(PacketWriter &w, const Node<Alloc> &n);

template <class Alloc> void scalar_into(PacketWriter &w, const Node<Alloc> &n) {
    auto bits = [&]<class T>() { return n.present ? std::optional<T>(load<T>(reinterpret_cast<const std::uint8_t *>(&n.bits))) : std::nullopt; };
    switch (static_cast<ScalarType>(n.type)) {
    case ScalarType::u8: w.u8(bits.template operator()<std::uint8_t>()); break;
    case ScalarType::i8: w.i8(bits.template operator()<std::int8_t>()); break;
    case ScalarType::u16: w.u16(bits.template operator()<std::uint16_t>()); break;
    case ScalarType::i16: w.i16(bits.template operator()<std::int16_t>()); break;
    case ScalarType::u32: w.u32(bits.template operator()<std::uint32_t>()); break;
    case ScalarType::i32: w.i32(bits.template operator()<std::int32_t>()); break;
    case ScalarType::u64: w.u64(bits.template operator()<std::uint64_t>()); break;
    case ScalarType::i64: w.i64(bits.template operator()<std::int64_t>()); break;
    case ScalarType::f32: w.f32(bits.template operator()<float>()); break;
    case ScalarType::f64: w.f64(bits.template operator()<double>()); break;
    case ScalarType::boolean: {
        std::optional<std::uint8_t> b = bits.template operator()<std::uint8_t>();
        w.boolean(b ? std::optional<bool>(*b != 0) : std::nullopt);
        break;
    }
    }
}

template <class Alloc> void walk(PacketWriter &w, const Node<Alloc> &n) {
    switch (n.kind) {
    case NodeKind::nil: w.nil(); return;
    case NodeKind::scalar: scalar_into(w, n); return;
    case NodeKind::array:
        if (!n.present) {
            switch (static_cast<SequenceType>(n.type)) {
            case SequenceType::u8_array: w.u8_array(std::nullopt); return;
            case SequenceType::i8_array: w.i8_array(std::nullopt); return;
            case SequenceType::u16_array: w.u16_array(std::nullopt); return;
            case SequenceType::i16_array: w.i16_array(std::nullopt); return;
            case SequenceType::u32_array: w.u32_array(std::nullopt); return;
            case SequenceType::i32_array: w.i32_array(std::nullopt); return;
            case SequenceType::u64_array: w.u64_array(std::nullopt); return;
            case SequenceType::i64_array: w.i64_array(std::nullopt); return;
            case SequenceType::f32_array: w.f32_array(std::nullopt); return;
            default: w.f64_array(std::nullopt); return;
            }
        }
        switch (static_cast<SequenceType>(n.type)) {
        case SequenceType::u8_array: array_into<std::uint8_t>(w, n); return;
        case SequenceType::i8_array: array_into<std::int8_t>(w, n); return;
        case SequenceType::u16_array: array_into<std::uint16_t>(w, n); return;
        case SequenceType::i16_array: array_into<std::int16_t>(w, n); return;
        case SequenceType::u32_array: array_into<std::uint32_t>(w, n); return;
        case SequenceType::i32_array: array_into<std::int32_t>(w, n); return;
        case SequenceType::u64_array: array_into<std::uint64_t>(w, n); return;
        case SequenceType::i64_array: array_into<std::int64_t>(w, n); return;
        case SequenceType::f32_array: array_into<float>(w, n); return;
        default: array_into<double>(w, n); return;
        }
    case NodeKind::bitset: w.bitset(n.present ? std::optional(n.bytes) : std::nullopt); return;
    case NodeKind::str: w.str(n.present ? std::optional(n.str) : std::nullopt); return;
    case NodeKind::strs: w.strs(n.present ? std::optional(n.strs) : std::nullopt); return;
    case NodeKind::branch:
        if (!n.present) {
            w.no_branch();
            return;
        }
        w.branch(static_cast<std::uint32_t>(n.children.size()), [&n](PacketWriter &b) {
            for (const Node<Alloc> &c : n.children) walk(b, c);
        });
        return;
    case NodeKind::graft: w.graft(n.bytes); return;
    }
}

} // namespace view_detail

// EINVAL for a cap outside a packet's sizes, EOVERFLOW for a tree past the cap.
template <class Alloc, class View>
Result<std::vector<std::uint8_t, typename std::allocator_traits<Alloc>::template rebind_alloc<std::uint8_t>>> encode(const View &view, TreeEncoderOptions opts = {},
                                                                                                                     const Alloc &alloc = Alloc()) {
    using Bytes = std::vector<std::uint8_t, typename std::allocator_traits<Alloc>::template rebind_alloc<std::uint8_t>>;
    std::uint32_t max_cap = opts.max_cap.value_or(MAX_PKT_SZ);
    if (max_cap < HDR_SZ || max_cap > MAX_PKT_SZ) return marshal_error(Errno::einval);
    std::uint32_t cap = std::max(HDR_SZ, std::min(opts.init_cap.value_or(1u << 10), max_cap));
    Bytes buf(alloc);
    for (;;) {
        buf.assign(cap, 0);
        PacketWriter w(std::span<std::uint8_t>(buf.data(), buf.size()));
        view_detail::walk(w, view.node());
        Result<std::uint32_t> n = w.finish();
        if (n) {
            buf.resize(*n);
            return buf;
        }
        if (n.error().code != Errno::eoverflow || cap == max_cap) return n.error();
        cap = cap > max_cap / 2 ? max_cap : cap * 2;
    }
}

} // namespace xpute::xtp
