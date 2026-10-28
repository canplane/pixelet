// cpp/xpute/kit/codec/codec.test.cpp

#include <array>
#include <string>
#include <string_view>

#include "xpute/kit/test.hpp"
#include "encoding.hpp"
#include "hash.hpp"

using namespace xpute;

static std::span<const std::uint8_t> bytes_of(std::string_view s) {
    return {reinterpret_cast<const std::uint8_t *>(s.data()), s.size()};
}

// RFC 4648's vectors, which the URL alphabet shares with the standard one.
XPUTE_TEST(codec_base64url_rfc_vectors) {
    const std::pair<std::string_view, std::string_view> cases[] = {
        {"", ""}, {"f", "Zg"}, {"fo", "Zm8"}, {"foo", "Zm9v"}, {"foob", "Zm9vYg"}, {"fooba", "Zm9vYmE"}, {"foobar", "Zm9vYmFy"},
    };
    for (auto [plain, coded] : cases) {
        std::array<char, 16> text{};
        std::size_t n = base64url_encode(bytes_of(plain), text);
        XPUTE_CHECK_EQ(std::string_view(text.data(), n), coded, "base64url wrote another text");
        std::array<std::uint8_t, 16> back{};
        Result<std::size_t> m = base64url_decode(coded, back);
        XPUTE_CHECK(m.ok());
        XPUTE_CHECK_EQ(std::string_view(reinterpret_cast<const char *>(back.data()), *m), plain, "base64url read back another string");
    }
}

XPUTE_TEST(codec_base64url_alphabet) {
    const std::array<std::uint8_t, 2> high{0xfb, 0xff};
    std::array<char, 4> text{};
    std::size_t n = base64url_encode(high, text);
    XPUTE_CHECK_EQ(std::string_view(text.data(), n), std::string_view("-_8"), "the URL alphabet's 62 and 63");
    std::array<std::uint8_t, 8> out{};
    XPUTE_CHECK(!base64url_decode("Zm9vY", out).ok());
    XPUTE_CHECK(!base64url_decode("Zm9=", out).ok());
    XPUTE_CHECK(!base64url_decode("Zm+v", out).ok());
    XPUTE_CHECK(!base64url_decode("Zh", out).ok());
}

XPUTE_TEST(codec_hex) {
    const std::array<std::uint8_t, 4> in{0x00, 0x7f, 0xab, 0xff};
    std::array<char, 8> text{};
    XPUTE_CHECK_EQ(std::string_view(text.data(), hex_encode(in, text)), std::string_view("007fabff"), "hex wrote another text");
    std::array<std::uint8_t, 4> back{};
    Result<std::size_t> n = hex_decode("007FabFF", back);
    XPUTE_CHECK(n.ok() && *n == 4 && back == in);
    XPUTE_CHECK(!hex_decode("abc", back).ok());
    XPUTE_CHECK(!hex_decode("zz", back).ok());
}

// FNV-1a's published values, which the other languages compute too.
XPUTE_TEST(codec_fnv1a64) {
    XPUTE_CHECK_EQ(fnv1a64(""), FNV64_OFFSET, "the empty string");
    XPUTE_CHECK_EQ(fnv1a64("a"), std::uint64_t{0xaf63dc4c8601ec8cu}, "\"a\"");
    XPUTE_CHECK_EQ(fnv1a64("foobar"), std::uint64_t{0x85944171f73967e8u}, "\"foobar\"");
    XPUTE_CHECK_EQ(fnv1a64(bytes_of("foobar")), fnv1a64("foobar"), "bytes and a string are one hash");
}
