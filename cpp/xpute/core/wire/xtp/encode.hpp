// cpp/xpute/core/wire/xtp/encode.hpp

// The write end: a packet written in place, into bytes the caller holds — a
// ring slot's payload, or a buffer sized for it — with no tree built first
// and nothing allocated. The other two build a tree of nodes and encode it
// after (Rust's TreeView and TreeEncoder, TypeScript's the same); Rust adds
// a writer like this one for the root branch of a ring's packet. Here that
// writer is the only one: it writes any root, every node type and a graft,
// and gives the bytes the tree encoder gives for the same tree.
//
// A packet is one root node. A branch declares its child count up front,
// since its table precedes its children, and then writes them as the packet
// writes its root; declaring one count and writing another is the caller's
// bug. What does not fit spoils the packet, and `finish` says why.
//
// The room is the buffer. What it refuses is what the encoder refuses at the
// same cap, including a string judged by its UTF-16 units, four bytes each,
// before its bytes — the rule the TypeScript measures by, which makes which
// strings a cap refuses part of what a packet is.

#pragma once

#include <cstdint>
#include <optional>
#include <span>
#include <string_view>

#include "../../status/bug.hpp"
#include "spec.hpp"

namespace xpute::xtp {

class PacketWriter {
  public:
    // A writer over `buf`, whose length is the cap: none past MAX_PKT_SZ,
    // and one shorter than a header refuses the packet.
    explicit PacketWriter(std::span<std::uint8_t> buf) noexcept;

    // An untyped null.
    PacketWriter &nil() noexcept { return entry(std::nullopt, type_of(SpecialType::nil)); }

    // A leaf of its type, or none for a null of that type.
    PacketWriter &u8(std::optional<std::uint8_t> v) noexcept { return scalar(ScalarType::u8, v); }
    PacketWriter &i8(std::optional<std::int8_t> v) noexcept { return scalar(ScalarType::i8, v); }
    PacketWriter &u16(std::optional<std::uint16_t> v) noexcept { return scalar(ScalarType::u16, v); }
    PacketWriter &i16(std::optional<std::int16_t> v) noexcept { return scalar(ScalarType::i16, v); }
    PacketWriter &u32(std::optional<std::uint32_t> v) noexcept { return scalar(ScalarType::u32, v); }
    PacketWriter &i32(std::optional<std::int32_t> v) noexcept { return scalar(ScalarType::i32, v); }
    PacketWriter &u64(std::optional<std::uint64_t> v) noexcept { return scalar(ScalarType::u64, v); }
    PacketWriter &i64(std::optional<std::int64_t> v) noexcept { return scalar(ScalarType::i64, v); }
    PacketWriter &f32(std::optional<float> v) noexcept { return scalar(ScalarType::f32, v); }
    PacketWriter &f64(std::optional<double> v) noexcept { return scalar(ScalarType::f64, v); }
    PacketWriter &boolean(std::optional<bool> v) noexcept {
        return scalar(ScalarType::boolean, v ? std::optional<std::uint8_t>(*v ? 1 : 0) : std::nullopt);
    }

    PacketWriter &u8_array(std::optional<std::span<const std::uint8_t>> a) noexcept { return array(a); }
    PacketWriter &i8_array(std::optional<std::span<const std::int8_t>> a) noexcept { return array(a); }
    PacketWriter &u16_array(std::optional<std::span<const std::uint16_t>> a) noexcept { return array(a); }
    PacketWriter &i16_array(std::optional<std::span<const std::int16_t>> a) noexcept { return array(a); }
    PacketWriter &u32_array(std::optional<std::span<const std::uint32_t>> a) noexcept { return array(a); }
    PacketWriter &i32_array(std::optional<std::span<const std::int32_t>> a) noexcept { return array(a); }
    PacketWriter &u64_array(std::optional<std::span<const std::uint64_t>> a) noexcept { return array(a); }
    PacketWriter &i64_array(std::optional<std::span<const std::int64_t>> a) noexcept { return array(a); }
    PacketWriter &f32_array(std::optional<std::span<const float>> a) noexcept { return array(a); }
    PacketWriter &f64_array(std::optional<std::span<const double>> a) noexcept { return array(a); }

    // An array of `len` elements, element `i` being `value(i)`: for numbers
    // that lie in no array to copy from.
    template <class T, class F> PacketWriter &array_with(std::uint32_t len, F &&value) noexcept {
        std::optional<std::uint32_t> at = seq(len, std::uint64_t{len} * sizeof(T));
        if (at) {
            for (std::uint32_t i = 0; i < len; i++) store<T>(buf_.data() + *at + WORD_SZ + i * sizeof(T), value(i));
        }
        return entry(at, type_of(ARRAY_OF<T>));
    }

    // Bits given a byte each, nonzero for set, packed low bit first.
    PacketWriter &bitset(std::optional<std::span<const std::uint8_t>> bits) noexcept;

    // UTF-8, with the NUL the wire carries after it and its length leaves out.
    PacketWriter &str(std::optional<std::string_view> s) noexcept;

    // Many strings as one leaf: len + 1 offsets into the UTF-8 after them.
    PacketWriter &strs(std::optional<std::span<const std::string_view>> items) noexcept;

    // A string written in place by `write`, which is given the room after
    // the length slot and answers how many bytes it wrote, or none when they
    // did not fit: for text that lies in no string to copy from.
    template <class F> PacketWriter &str_with(F &&write) noexcept {
        std::optional<std::uint32_t> at;
        if (std::optional<std::uint32_t> room = str_room()) {
            std::uint32_t payload = str_payload();
            std::optional<std::size_t> n = write(std::span<char>(reinterpret_cast<char *>(buf_.data() + payload), *room));
            at = n && *n <= *room ? str_commit(static_cast<std::uint32_t>(*n)) : spoil(Errno::eoverflow);
        }
        return entry(at, type_of(SequenceType::str));
    }

    // A branch of `count` children, which `fill` writes with this writer.
    template <class F> PacketWriter &branch(std::uint32_t count, F &&fill) noexcept {
        std::optional<std::uint32_t> at = branch_open(count);
        Frame outer = frame_;
        frame_ = Frame{.base = at.value_or(0), .count = count, .written = 0, .root = false};
        fill(*this);
        ensure(frame_.written == frame_.count, Errno::enotrecoverable, frame_.count, frame_.written);
        frame_ = outer;
        return entry(branch_close(at), type_of(SpecialType::branch));
    }

    // A branch that is not there, its type kept.
    PacketWriter &no_branch() noexcept { return entry(std::nullopt, type_of(SpecialType::branch)); }

    // A packet already written, spliced in as this node: its root's payload
    // copied, placed as its root's type places it.
    PacketWriter &graft(std::span<const std::uint8_t> packet) noexcept;

    // The packet's length, header included and to the word, once its root
    // is written; or why it was refused.
    Result<std::uint32_t> finish() noexcept;

  private:
    // The branch being written, or the root, which is a branch of one whose
    // entry is the header.
    struct Frame {
        std::uint32_t base;
        std::uint32_t count;
        std::uint32_t written;
        bool root;
    };

    std::optional<std::uint32_t> spoil(Errno code) noexcept;
    std::optional<std::uint32_t> reserve(std::uint32_t unit, std::uint64_t bytes) noexcept;
    PacketWriter &entry(std::optional<std::uint32_t> at, NodeType type) noexcept;

    template <class T> PacketWriter &scalar(ScalarType type, std::optional<T> v) noexcept {
        std::optional<std::uint32_t> at;
        if (v) {
            at = reserve(sizeof(T), sizeof(T));
            if (at) store<T>(buf_.data() + *at, *v);
        }
        return entry(at, type_of(type));
    }

    std::optional<std::uint32_t> seq(std::uint32_t len, std::uint64_t bytes) noexcept;

    template <class T> PacketWriter &array(std::optional<std::span<const T>> a) noexcept {
        if (!a) return entry(std::nullopt, type_of(ARRAY_OF<T>));
        return array_with<T>(static_cast<std::uint32_t>(a->size()), [&a](std::uint32_t i) { return (*a)[i]; });
    }

    std::uint32_t str_payload() const noexcept;
    std::optional<std::uint32_t> str_room() noexcept;
    std::optional<std::uint32_t> str_commit(std::uint32_t len) noexcept;

    std::optional<std::uint32_t> branch_open(std::uint32_t count) noexcept;
    std::optional<std::uint32_t> branch_close(std::optional<std::uint32_t> at) noexcept;

    std::span<std::uint8_t> buf_;
    Frame frame_{.base = 0, .count = 1, .written = 0, .root = true};
    // The root's type, for the header.
    NodeType root_type_ = type_of(SpecialType::nil);
    // The end of what is written, from the packet's start.
    std::uint32_t lim_ = HDR_SZ;
    // Why the packet was spoiled, the first time it was.
    std::optional<Error> error_;
};

} // namespace xpute::xtp
