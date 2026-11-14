// cpp/xpute/kit/wire/xtp/encode.hpp

// Writes a packet in place, into bytes the caller holds, without building a
// tree and without allocating. It is the one writer of the layout: `encode`
// (view.hpp) walks its tree onto it, so both give the same bytes. A node that
// does not fit does not fail its call: the writer goes on counting entries,
// and `finish` refuses the packet (EOVERFLOW). In every language, a string
// fits exactly when its UTF-8 and the NUL after it do.

#pragma once

#include <cstdint>
#include <optional>
#include <span>
#include <string_view>

#include "xpute/kit/status/bug.hpp"
#include "spec.hpp"

namespace xpute::xtp {

class PacketWriter {
  public:
    // A writer for a packet whose root is any one node. The packet may take
    // the whole buffer, up to `MAX_PKT_SZ` bytes.
    explicit PacketWriter(std::span<std::uint8_t> buf) noexcept;

    PacketWriter &nil() noexcept { return entry(std::nullopt, type_of(SpecialType::nil)); }

    // An empty optional writes a typed null, an absent node that keeps its
    // type.
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

    // A packed array of `T`, an empty optional a typed null.
    template <class T> PacketWriter &array(std::optional<std::span<const T>> a) noexcept {
        if (!a) return entry(std::nullopt, type_of(ARRAY_OF<T>));
        return array_with<T>(static_cast<std::uint32_t>(a->size()), [&a](std::uint32_t i) { return (*a)[i]; });
    }

    template <class T, class F> PacketWriter &array_with(std::uint32_t len, F &&val) noexcept {
        std::optional<std::uint32_t> at = seq(len, std::uint64_t{len} * sizeof(T));
        if (at) {
            for (std::uint32_t i = 0; i < len; i++) store<T>(buf_.data() + *at + WORD_SZ + i * sizeof(T), val(i));
        }
        return entry(at, type_of(ARRAY_OF<T>));
    }

    // `bits` holds one byte for each bit, nonzero for set. They are packed
    // eight to a byte, the first in the lowest bit.
    PacketWriter &bitset(std::optional<std::span<const std::uint8_t>> bits) noexcept;

    PacketWriter &str(std::optional<std::string_view> s) noexcept;

    PacketWriter &strs(std::optional<std::span<const std::string_view>> items) noexcept;

    // Writing a number of children other than `cnt` is a bug.
    template <class F> PacketWriter &branch(std::uint32_t cnt, F &&fill) noexcept {
        std::optional<std::uint32_t> at = branch_open(cnt);
        Table outer = table_;
        table_ = Table{.base = at.value_or(0), .cnt = cnt, .written = 0, .root = false};
        fill(*this);
        ensure(table_.written == table_.cnt, Errno::enotrecoverable, table_.cnt, table_.written);
        table_ = outer;
        return entry(branch_close(at), type_of(SpecialType::branch));
    }

    PacketWriter &no_branch() noexcept { return entry(std::nullopt, type_of(SpecialType::branch)); }

    // Copies the root of the encoded packet `pkt` in as this entry, aligned as
    // its root's type requires. A malformed `pkt` makes `finish` refuse the
    // packet (EBADMSG).
    PacketWriter &graft(std::span<const std::uint8_t> pkt) noexcept;

    // Writes the header and returns the packet's length, header included and
    // padded to a word, or the first error that spoiled the packet.
    Result<std::uint32_t> finish() noexcept;

  private:
    // Writes a str whose bytes `write` puts into the room after the len slot.
    // `write` returns how many bytes it wrote, or nothing when they did not
    // fit, which spoils the packet.
    template <class F> PacketWriter &str_with(F &&write) noexcept {
        std::optional<std::uint32_t> at;
        if (std::optional<std::uint32_t> room = str_room()) {
            std::uint32_t payload = str_payload();
            std::optional<std::size_t> n = write(std::span<char>(reinterpret_cast<char *>(buf_.data() + payload), *room));
            at = n && *n <= *room ? str_commit(static_cast<std::uint32_t>(*n)) : spoil(Errno::eoverflow);
        }
        return entry(at, type_of(SequenceType::str));
    }

    // The child table the writer's next entries go in. The root is treated as
    // a branch of one child whose entry is the header's root word: `root`
    // marks it, and its entry sets the root's type instead of writing a
    // descriptor.
    struct Table {
        std::uint32_t base;
        std::uint32_t cnt;
        std::uint32_t written;
        bool root;
    };

    std::optional<std::uint32_t> spoil(Errno errnum) noexcept;
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

    std::uint32_t str_payload() const noexcept;
    std::optional<std::uint32_t> str_room() noexcept;
    std::optional<std::uint32_t> str_commit(std::uint32_t len) noexcept;

    std::optional<std::uint32_t> branch_open(std::uint32_t cnt) noexcept;
    std::optional<std::uint32_t> branch_close(std::optional<std::uint32_t> at) noexcept;

    std::span<std::uint8_t> buf_;
    Table table_{.base = 0, .cnt = 1, .written = 0, .root = true};
    NodeType root_type_ = type_of(SpecialType::nil);
    std::uint32_t lim_ = HDR_SZ;
    // This is the first error met; later ones are not kept.
    std::optional<Error> err_;
};

} // namespace xpute::xtp
