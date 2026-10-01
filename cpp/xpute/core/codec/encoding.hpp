// cpp/xpute/core/codec/encoding.hpp

// Bytes as text, the two ways the kit writes them: hex, lowercase, two digits
// a byte; and base64url without padding, the form the other two languages
// write a blob in (@xpute/core codec/encoding.ts; Rust's `base64` crate at
// URL_SAFE_NO_PAD). The standard library has neither, so they are here, as
// TypeScript's are.
//
// Each writes into the caller's span and allocates nothing; a span too short
// is the caller's mistake and a bug, and text that does not decode is a fault
// from outside and returned.

#pragma once

#include <cstddef>
#include <cstdint>
#include <span>
#include <string_view>

#include "../status/error.hpp"

namespace xpute {

constexpr std::size_t hex_len(std::size_t bytes) noexcept {
    return bytes * 2;
}

// Writes `in` as hex into `out`, which is at least hex_len(in.size()); the
// characters written.
std::size_t hex_encode(std::span<const std::uint8_t> in, std::span<char> out) noexcept;

// Reads hex into `out`: the bytes read, or EBADMSG for an odd length or a
// character that is not a hex digit. Either case is accepted.
Result<std::size_t> hex_decode(std::string_view in, std::span<std::uint8_t> out) noexcept;

constexpr std::size_t base64url_len(std::size_t bytes) noexcept {
    return bytes / 3 * 4 + (bytes % 3 == 0 ? 0 : bytes % 3 + 1);
}

// Writes `in` as unpadded base64url into `out`, which is at least
// base64url_len(in.size()); the characters written.
std::size_t base64url_encode(std::span<const std::uint8_t> in, std::span<char> out) noexcept;

// Reads unpadded base64url into `out`: the bytes read, or EBADMSG for a
// length no encoding has, a padding character, a character outside the
// alphabet, or bits set past the last byte.
Result<std::size_t> base64url_decode(std::string_view in, std::span<std::uint8_t> out) noexcept;

// `s` handed to `put` with every byte that is not part of UTF-8 as
// U+FFFD, a maximal bad prefix one replacement, as a host decodes and
// Rust's `from_utf8_lossy` does.
template <class Put> void utf8_lossy(std::string_view s, Put &&put) {
    static constexpr std::string_view FFFD = "\xef\xbf\xbd";
    std::size_t plain = 0, i = 0;
    auto byte = [&](std::size_t k) { return static_cast<unsigned char>(s[k]); };
    while (i < s.size()) {
        unsigned char b = byte(i);
        std::size_t len = b < 0x80 ? 1 : (b >= 0xc2 && b < 0xe0) ? 2 : (b >= 0xe0 && b < 0xf0) ? 3 : (b >= 0xf0 && b < 0xf5) ? 4 : 0;
        // How far the sequence holds: the second byte's range depends on the
        // first, which is what keeps overlong forms and surrogates out.
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
        if (len != 0 && good == len) {
            i += len;
            continue;
        }
        put(s.substr(plain, i - plain));
        put(FFFD);
        i += good == 0 ? 1 : good;
        plain = i;
    }
    put(s.substr(plain));
}

} // namespace xpute
