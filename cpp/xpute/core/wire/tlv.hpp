// cpp/xpute/core/wire/tlv.hpp

// TLV: xpute's forward-only typed value sequence — the format xpute-core's
// wire/tlv.rs and @xpute/core wire/tlv.ts write, and golden/wire/tlv.tsv
// holds all three to, byte for byte. Rust's module comment is the contract in
// full; in short:
//
//   [tag | payload] … [END]
//
// A producer appends values; a consumer reads them once, in order, and keeps
// nothing it has passed — no table, offset or tree, which is what sets it
// apart from XTP, a retained structured value. A tag is its payload's length
// as well as its type: a scalar is its little-endian bytes, STR and BYTES a
// u32 length and that many bytes.
//
// Where reading stops is one of three facts, and the reader says which: END
// read (closed); the input run out at an element's boundary (unclosed); or
// the input run out inside an element, or a tag unknown — EBADMSG, the values
// before it standing. Which end a sequence must have is its user's.
//
// The writer appends to a byte container the caller owns — any with the
// vector's `push_back` and `insert`, so one over the guest's heap — and a
// value read back names the width it was written at. A string or bytes read
// back is a view into the buffer, which outlives it.

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

#include "../codec/encoding.hpp"
#include "../status/error.hpp"

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

// A string as it lies. What a reader decodes of it — what is not UTF-8 as
// U+FFFD, as Rust's reader and the host's decode it — is `decode`'s.
struct Str {
    std::string_view text;
    bool operator==(const Str &) const = default;
    template <class Put> void decode(Put &&put) const { utf8_lossy(text, std::forward<Put>(put)); }
};

struct Bytes {
    std::span<const std::uint8_t> data;
    bool operator==(const Bytes &o) const { return std::equal(data.begin(), data.end(), o.data.begin(), o.data.end()); }
};

// Where a sequence stopped without a fault.
enum class End : std::uint8_t {
    // END was read.
    closed,
    // The input ran out at an element's boundary, without END.
    unclosed,
};

// One variant per tag, in the tags' order.
using Value = std::variant<bool, std::uint8_t, std::int8_t, std::uint16_t, std::int16_t, std::uint32_t, std::int32_t, std::uint64_t, std::int64_t, float, double, Str, Bytes>;

template <class Buf> class Writer {
  public:
    explicit Writer(Buf &buf) noexcept : buf_(buf) {}

    const Buf &data() const noexcept { return buf_; }

    // Seals the stream with END, once. The writer is not written to after.
    const Buf &finish() {
        if (!sealed_) {
            buf_.push_back(static_cast<std::uint8_t>(Tag::end));
            sealed_ = true;
        }
        return buf_;
    }

    Result<void> write(const Value &v) {
        if (sealed_) return marshal_error(Errno::ebadmsg);
        std::visit([this](const auto &x) { this->put(x); }, v);
        return {};
    }

    // Writes every value in turn; the first refusal stops it.
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
    void put(const Bytes &b) {
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

    // The next value, or none at END or at the input's end, which `end`
    // then tells apart.
    Result<std::optional<Value>> next() noexcept;

    // How the sequence ended, once reading has reached its end: none while
    // values remain, and none after a fault.
    std::optional<End> end() const noexcept { return end_; }

    // Every value until END, appended to `out`.
    template <class Vec> Result<void> read_all(Vec &out) {
        for (;;) {
            Result<std::optional<Value>> v = next();
            if (!v) return v.error();
            if (!*v) return {};
            out.push_back(**v);
        }
    }

  private:
    // Whether `n` more bytes are there, in 64 bits: a length read off the wire
    // past 2^32 would otherwise wrap under the end rather than pass it.
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
