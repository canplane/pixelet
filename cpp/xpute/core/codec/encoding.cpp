// cpp/xpute/core/codec/encoding.cpp

#include "encoding.hpp"

#include "../status/bug.hpp"

namespace xpute {

namespace {

constexpr char HEX[] = "0123456789abcdef";
constexpr char B64URL[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

int hex_value(char c) noexcept {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

int b64url_value(char c) noexcept {
    if (c >= 'A' && c <= 'Z') return c - 'A';
    if (c >= 'a' && c <= 'z') return c - 'a' + 26;
    if (c >= '0' && c <= '9') return c - '0' + 52;
    if (c == '-') return 62;
    if (c == '_') return 63;
    return -1;
}

} // namespace

std::size_t hex_encode(std::span<const std::uint8_t> in, std::span<char> out) noexcept {
    ensure(out.size() >= hex_len(in.size()), Errno::enospc, out.size(), in.size());
    std::size_t at = 0;
    for (std::uint8_t b : in) {
        out[at++] = HEX[b >> 4];
        out[at++] = HEX[b & 0xf];
    }
    return at;
}

Result<std::size_t> hex_decode(std::string_view in, std::span<std::uint8_t> out) noexcept {
    if (in.size() % 2 != 0) return marshal_error(Errno::ebadmsg);
    ensure(out.size() >= in.size() / 2, Errno::enospc, out.size(), in.size());
    for (std::size_t i = 0; i < in.size(); i += 2) {
        int hi = hex_value(in[i]);
        int lo = hex_value(in[i + 1]);
        if (hi < 0 || lo < 0) return marshal_error(Errno::ebadmsg);
        out[i / 2] = static_cast<std::uint8_t>(hi << 4 | lo);
    }
    return in.size() / 2;
}

std::size_t base64url_encode(std::span<const std::uint8_t> in, std::span<char> out) noexcept {
    ensure(out.size() >= base64url_len(in.size()), Errno::enospc, out.size(), in.size());
    std::size_t at = 0;
    std::size_t i = 0;
    for (; i + 3 <= in.size(); i += 3) {
        std::uint32_t w = std::uint32_t{in[i]} << 16 | std::uint32_t{in[i + 1]} << 8 | in[i + 2];
        out[at++] = B64URL[w >> 18 & 63];
        out[at++] = B64URL[w >> 12 & 63];
        out[at++] = B64URL[w >> 6 & 63];
        out[at++] = B64URL[w & 63];
    }
    std::size_t rest = in.size() - i;
    if (rest > 0) {
        std::uint32_t w = std::uint32_t{in[i]} << 16 | (rest == 2 ? std::uint32_t{in[i + 1]} << 8 : 0);
        out[at++] = B64URL[w >> 18 & 63];
        out[at++] = B64URL[w >> 12 & 63];
        if (rest == 2) out[at++] = B64URL[w >> 6 & 63];
    }
    return at;
}

Result<std::size_t> base64url_decode(std::string_view in, std::span<std::uint8_t> out) noexcept {
    // A final group of one character carries six bits, which is no whole
    // byte: no encoding ends that way.
    if (in.size() % 4 == 1) return marshal_error(Errno::ebadmsg);
    std::size_t bytes = in.size() / 4 * 3 + (in.size() % 4 == 0 ? 0 : in.size() % 4 - 1);
    ensure(out.size() >= bytes, Errno::enospc, out.size(), bytes);
    std::size_t at = 0;
    for (std::size_t i = 0; i < in.size(); i += 4) {
        std::size_t n = in.size() - i < 4 ? in.size() - i : 4;
        std::uint32_t w = 0;
        for (std::size_t k = 0; k < 4; k++) {
            int v = k < n ? b64url_value(in[i + k]) : 0;
            if (v < 0) return marshal_error(Errno::ebadmsg);
            w = w << 6 | static_cast<std::uint32_t>(v);
        }
        // Bits past the last byte are zero in any canonical encoding.
        if ((n == 2 && (w & 0xffff) != 0) || (n == 3 && (w & 0xff) != 0)) return marshal_error(Errno::ebadmsg);
        out[at++] = static_cast<std::uint8_t>(w >> 16);
        if (n > 2) out[at++] = static_cast<std::uint8_t>(w >> 8);
        if (n > 3) out[at++] = static_cast<std::uint8_t>(w);
    }
    return at;
}

} // namespace xpute
