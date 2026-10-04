// cpp/xpute/core/wire/tlv.hpp

// TLV, a forward-only typed value sequence: [tag, payload] … [END]. A reader
// keeps nothing it has passed. Reading stops closed (END), unclosed (input
// ran out at a boundary), or EBADMSG (inside an element or an unknown tag).

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

// Invalid UTF-8 decodes to U+FFFD, as the host decodes it.
struct Str {
    std::string_view text;
    bool operator==(const Str &) const = default;
    template <class Put> void decode(Put &&put) const { utf8_lossy(text, std::forward<Put>(put)); }
};

struct Bytes {
    std::span<const std::uint8_t> data;
    bool operator==(const Bytes &o) const { return std::equal(data.begin(), data.end(), o.data.begin(), o.data.end()); }
};

enum class End : std::uint8_t {
    closed,
    unclosed,
};

// In the tags' order.
using Value = std::variant<bool, std::uint8_t, std::int8_t, std::uint16_t, std::int16_t, std::uint32_t, std::int32_t, std::uint64_t, std::int64_t, float, double, Str, Bytes>;

template <class Buf> class Writer {
  public:
    explicit Writer(Buf &buf) noexcept : buf_(buf) {}

    const Buf &data() const noexcept { return buf_; }

    // The writer is not written to after.
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

    // None at END or at the input's end, which `end` tells apart.
    Result<std::optional<Value>> next() noexcept;

    // None while values remain, and none after a fault.
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
    // In 64 bits: a length past 2^32 off the wire would otherwise wrap.
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
