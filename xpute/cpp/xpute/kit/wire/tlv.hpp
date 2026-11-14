// cpp/xpute/kit/wire/tlv.hpp

// TLV, a sequence of values read once, front to back, with no table and no
// offset. Each value is a tag byte and its little-endian payload, and the tag
// `end` closes the sequence. A tag fixes its payload's size, except `str` and
// `bytes`, whose payload starts with its length as a u32; a reader cannot skip
// a tag it does not know, so an unknown tag is EBADMSG.
//
// The reader tells how the sequence ended (`Reader::end`): closed by the tag
// `end`, or unclosed, where the input ran out between two values, which the
// caller may or may not accept. Input that runs out inside a value is
// EBADMSG, and the values read before it stand.
//
// Equal values give equal bytes only under a schema that fixes their types: 1
// may be written as a u8, a u32 or an f64.

#pragma once

#include <algorithm>
#include <array>
#include <bit>
#include <cstdint>
#include <cstring>
#include <optional>
#include <span>
#include <string_view>
#include <variant>

#include "xpute/kit/codec/encoding.hpp"
#include "xpute/kit/status/error.hpp"

namespace xpute::tlv {

enum class Tag : std::uint8_t {
    end = 0x00,
    boolean = 0x01,
    u8 = 0x02,
    i8 = 0x03,
    u16 = 0x04,
    i16 = 0x05,
    u32 = 0x06,
    i32 = 0x07,
    u64 = 0x08,
    i64 = 0x09,
    f32 = 0x0a,
    f64 = 0x0b,
    str = 0x10,
    bytes = 0x20,
};

// `decode` puts U+FFFD in place of each sequence that is not UTF-8, as the
// Rust and TypeScript readers do.
struct Str {
    std::string_view text;
    bool operator==(const Str &) const = default;
    template <class Put> void decode(Put &&put) const { utf8_lossy(text, std::forward<Put>(put)); }
};

struct Blob {
    std::span<const std::uint8_t> data;
    bool operator==(const Blob &o) const { return std::equal(data.begin(), data.end(), o.data.begin(), o.data.end()); }
};

enum class End : std::uint8_t {
    closed,
    unclosed,
};

using Value = std::variant<bool, std::uint8_t, std::int8_t, std::uint16_t, std::int16_t, std::uint32_t, std::int32_t, std::uint64_t, std::int64_t, float, double, Str, Blob>;

template <class Buf> class Writer {
  public:
    explicit Writer(Buf &buf) noexcept : buf_(buf) {}

    const Buf &data() const noexcept { return buf_; }

    // Appends the tag `end`, once. A write after it is EBADMSG.
    const Buf &finish() {
        if (!sealed_) {
            buf_.push_back(static_cast<std::uint8_t>(Tag::end));
            sealed_ = true;
        }
        return buf_;
    }

    Result<void> write(const Value &v) {
        if (sealed_) return make_error(Errno::ebadmsg);
        std::visit([this](const auto &x) { this->put(x); }, v);
        return {};
    }

    Result<void> write_all(std::span<const Value> vals) {
        for (const Value &v : vals) {
            if (Result<void> r = write(v); !r) return r;
        }
        return {};
    }

  private:
    void tag(Tag t) { buf_.push_back(static_cast<std::uint8_t>(t)); }

    template <class T> void le(T x) {
        auto raw = std::bit_cast<std::array<std::uint8_t, sizeof(T)>>(x);
        if constexpr (std::endian::native == std::endian::big) std::reverse(raw.begin(), raw.end());
        buf_.insert(buf_.end(), raw.begin(), raw.end());
    }

    void put(bool x) {
        tag(Tag::boolean);
        buf_.push_back(x ? 1 : 0);
    }
    void put(std::uint8_t x) { tag(Tag::u8), le(x); }
    void put(std::int8_t x) { tag(Tag::i8), le(x); }
    void put(std::uint16_t x) { tag(Tag::u16), le(x); }
    void put(std::int16_t x) { tag(Tag::i16), le(x); }
    void put(std::uint32_t x) { tag(Tag::u32), le(x); }
    void put(std::int32_t x) { tag(Tag::i32), le(x); }
    void put(std::uint64_t x) { tag(Tag::u64), le(x); }
    void put(std::int64_t x) { tag(Tag::i64), le(x); }
    void put(float x) { tag(Tag::f32), le(x); }
    void put(double x) { tag(Tag::f64), le(x); }
    void put(const Str &s) {
        tag(Tag::str);
        le(static_cast<std::uint32_t>(s.text.size()));
        buf_.insert(buf_.end(), s.text.begin(), s.text.end());
    }
    void put(const Blob &b) {
        tag(Tag::bytes);
        le(static_cast<std::uint32_t>(b.data.size()));
        buf_.insert(buf_.end(), b.data.begin(), b.data.end());
    }

    Buf &buf_;
    bool sealed_ = false;
};

class Reader {
  public:
    explicit Reader(std::span<const std::uint8_t> buf) noexcept : buf_(buf) {}

    // None at the tag `end` or at the input's end, which `end()` tells apart.
    Result<std::optional<Value>> next() noexcept;

    // How the sequence ended, once the reader has reached its end; none while
    // values remain, and after a fault.
    std::optional<End> end() const noexcept { return end_; }

    template <class Vec> Result<void> read_all(Vec &out) {
        for (;;) {
            Result<std::optional<Value>> v = next();
            if (!v) return v.error();
            if (!*v) return {};
            out.push_back(**v);
        }
    }

  private:
    // `n` is 64-bit so that a length off the wire near 2^32 cannot wrap the
    // sum.
    bool has(std::uint64_t n) const noexcept { return off_ + n <= buf_.size(); }

    template <class T> T le() noexcept {
        std::array<std::uint8_t, sizeof(T)> raw;
        std::memcpy(raw.data(), buf_.data() + off_, sizeof(T));
        if constexpr (std::endian::native == std::endian::big) std::reverse(raw.begin(), raw.end());
        off_ += sizeof(T);
        return std::bit_cast<T>(raw);
    }

    std::span<const std::uint8_t> buf_;
    std::size_t off_ = 0;
    std::optional<End> end_;
};

} // namespace xpute::tlv
