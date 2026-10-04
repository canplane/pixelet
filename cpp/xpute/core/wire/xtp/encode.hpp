// cpp/xpute/core/wire/xtp/encode.hpp

// Writes a packet in place into bytes the caller holds, with no tree and no
// allocation; the bytes match the tree encoder's for the same tree. A string
// is judged against the cap by its UTF-16 units first, four bytes each, as the
// TypeScript measures, so the same strings are refused on every side.

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
    // The buffer's length is the cap, at most MAX_PKT_SZ.
    explicit PacketWriter(std::span<std::uint8_t> buf) noexcept;

    PacketWriter &nil() noexcept { return entry(std::nullopt, type_of(SpecialType::nil)); }

    // None writes a null of that type.
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

    template <class T, class F> PacketWriter &array_with(std::uint32_t len, F &&value) noexcept {
        std::optional<std::uint32_t> at = seq(len, std::uint64_t{len} * sizeof(T));
        if (at) {
            for (std::uint32_t i = 0; i < len; i++) store<T>(buf_.data() + *at + WORD_SZ + i * sizeof(T), value(i));
        }
        return entry(at, type_of(ARRAY_OF<T>));
    }

    // Packed low bit first.
    PacketWriter &bitset(std::optional<std::span<const std::uint8_t>> bits) noexcept;

    // Followed by a NUL that the length leaves out.
    PacketWriter &str(std::optional<std::string_view> s) noexcept;

    // len + 1 offsets into the UTF-8 after them.
    PacketWriter &strs(std::optional<std::span<const std::string_view>> items) noexcept;

    // `write` gets the room after the length slot and answers the bytes written,
    // or none when they did not fit.
    template <class F> PacketWriter &str_with(F &&write) noexcept {
        std::optional<std::uint32_t> at;
        if (std::optional<std::uint32_t> room = str_room()) {
            std::uint32_t payload = str_payload();
            std::optional<std::size_t> n = write(std::span<char>(reinterpret_cast<char *>(buf_.data() + payload), *room));
            at = n && *n <= *room ? str_commit(static_cast<std::uint32_t>(*n)) : spoil(Errno::eoverflow);
        }
        return entry(at, type_of(SequenceType::str));
    }

    // Writing a different number of children than `count` is a bug.
    template <class F> PacketWriter &branch(std::uint32_t count, F &&fill) noexcept {
        std::optional<std::uint32_t> at = branch_open(count);
        Frame outer = frame_;
        frame_ = Frame{.base = at.value_or(0), .count = count, .written = 0, .root = false};
        fill(*this);
        ensure(frame_.written == frame_.count, Errno::enotrecoverable, frame_.count, frame_.written);
        frame_ = outer;
        return entry(branch_close(at), type_of(SpecialType::branch));
    }

    PacketWriter &no_branch() noexcept { return entry(std::nullopt, type_of(SpecialType::branch)); }

    // Its root's payload copied, placed as its root's type places it.
    PacketWriter &graft(std::span<const std::uint8_t> packet) noexcept;

    // The length, header included and to the word, or why it was refused.
    Result<std::uint32_t> finish() noexcept;

  private:
    // The root is a branch of one whose entry is the header.
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
    NodeType root_type_ = type_of(SpecialType::nil);
    std::uint32_t lim_ = HDR_SZ;
    // The first reason only.
    std::optional<Error> error_;
};

} // namespace xpute::xtp
