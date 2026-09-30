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

} // namespace xpute
