// cpp/xpute/kit/codec/encoding.hpp

// Hex and unpadded base64url, in the forms the other languages write. An
// output span that is too short is a bug; a text that does not decode is an
// EBADMSG error.

#pragma once

#include <cstddef>
#include <cstdint>
#include <span>
#include <string_view>

#include "xpute/kit/status/error.hpp"

namespace xpute {

constexpr std::size_t hex_len(std::size_t bytes) noexcept {
    return bytes * 2;
}

// `out` must hold at least `hex_len(in.size())` characters.
std::size_t hex_encode(std::span<const std::uint8_t> in, std::span<char> out) noexcept;

// Accepts hex digits in either case.
Result<std::size_t> hex_decode(std::string_view in, std::span<std::uint8_t> out) noexcept;

constexpr std::size_t base64url_len(std::size_t bytes) noexcept {
    return bytes / 3 * 4 + (bytes % 3 == 0 ? 0 : bytes % 3 + 1);
}

// `out` must hold at least `base64url_len(in.size())` characters.
std::size_t base64url_encode(std::span<const std::uint8_t> in, std::span<char> out) noexcept;

// Refuses with EBADMSG a character outside the alphabet, padding, and any bit
// set past the last byte, so that each byte string has one encoding.
Result<std::size_t> base64url_decode(std::string_view in, std::span<std::uint8_t> out) noexcept;

// Reads the UTF-8 character at `i`. `len` is its length in bytes when it is
// well formed. Otherwise `len` is 0 and `bad` is the length of the maximal
// ill-formed prefix there, at least 1.
struct Utf8Step {
    std::size_t len, bad;
};
constexpr Utf8Step utf8_step(std::string_view s, std::size_t i) noexcept {
    auto byte = [&](std::size_t k) { return static_cast<unsigned char>(s[k]); };
    unsigned char b = byte(i);
    std::size_t len = b < 0x80 ? 1 : (b >= 0xc2 && b < 0xe0) ? 2 : (b >= 0xe0 && b < 0xf0) ? 3 : (b >= 0xf0 && b < 0xf5) ? 4 : 0;
    // The allowed range of the second byte depends on the first, which keeps
    // overlong forms and surrogates out.
    std::size_t good = len == 0 ? 0 : 1;
    for (std::size_t k = 1; k < len && i + k < s.size(); k++) {
        unsigned char c = byte(i + k);
        unsigned char lo = 0x80, hi = 0xbf;
        if (k == 1 && b == 0xe0) lo = 0xa0;
        if (k == 1 && b == 0xed) hi = 0x9f;
        if (k == 1 && b == 0xf0) lo = 0x90;
        if (k == 1 && b == 0xf4) hi = 0x8f;
        if (c < lo || c > hi) break;
        good++;
    }
    if (len != 0 && good == len) return {len, 0};
    return {0, good == 0 ? 1 : good};
}

constexpr bool is_utf8(std::string_view s) noexcept {
    for (std::size_t i = 0; i < s.size();) {
        Utf8Step c = utf8_step(s, i);
        if (c.len == 0) return false;
        i += c.len;
    }
    return true;
}

// Hands `put` the text with each maximal ill-formed prefix replaced by one
// U+FFFD, as Rust's `String::from_utf8_lossy` does.
template <class Put> void utf8_lossy(std::string_view s, Put &&put) {
    static constexpr std::string_view FFFD = "\xef\xbf\xbd";
    std::size_t plain = 0, i = 0;
    while (i < s.size()) {
        Utf8Step c = utf8_step(s, i);
        if (c.len != 0) {
            i += c.len;
            continue;
        }
        put(s.substr(plain, i - plain));
        put(FFFD);
        i += c.bad;
        plain = i;
    }
    put(s.substr(plain));
}

} // namespace xpute
