// cpp/xpute/kit/wire/tlv.test.cpp

#include <charconv>
#include <cmath>
#include <limits>
#include <string>
#include <vector>

#include "xpute/kit/codec/encoding.hpp"
#include "dev/test/golden.hpp"
#include "dev/test/test.hpp"
#include "tlv.hpp"

using namespace xpute;

namespace {

std::string hex(std::span<const std::uint8_t> bytes) {
    std::string out(hex_len(bytes.size()), '\0');
    hex_encode(bytes, out);
    return out;
}

// A number as JavaScript prints one, which is the record's text: the shortest
// digits that read back as the same double.
std::string js_number(double x) {
    if (std::isnan(x)) return "NaN";
    char buf[64];
    auto r = std::to_chars(buf, buf + sizeof buf, x);
    return std::string(buf, r.ptr);
}

// A value in the golden record's text, which is how the TypeScript test prints
// one: its `typeof`, a colon, the value.
std::string show(const tlv::Value &v) {
    return std::visit(
        [](const auto &x) -> std::string {
            using T = std::decay_t<decltype(x)>;
            if constexpr (std::is_same_v<T, bool>) return std::string("boolean:") + (x ? "true" : "false");
            else if constexpr (std::is_same_v<T, std::uint64_t> || std::is_same_v<T, std::int64_t>) return "bigint:" + std::to_string(x);
            else if constexpr (std::is_same_v<T, float> || std::is_same_v<T, double>) return "number:" + js_number(static_cast<double>(x));
            else if constexpr (std::is_same_v<T, tlv::Str>) return "string:" + std::string(x.text);
            else if constexpr (std::is_same_v<T, tlv::Blob>) return "bytes:" + hex(x.data);
            else return "number:" + std::to_string(x);
        },
        v);
}

} // namespace

XPUTE_TEST(tlv_matches_other_languages) {
    Golden v = Golden::load("kit/spec/golden/wire/tlv.tsv");
    std::vector<std::uint8_t> buf;
    tlv::Writer w(buf);
    const std::uint8_t three[] = {9, 8, 7};
    const std::uint8_t one[] = {1};
    const tlv::Value vals[] = {
        std::uint8_t{200},
        std::int8_t{-3},
        std::uint16_t{65535},
        std::int16_t{-2},
        std::uint32_t{4000000000u},
        std::int32_t{-7},
        std::uint64_t{1} << 63,
        std::int64_t{-(std::int64_t{1} << 40)},
        0.25f,
        -1.5,
        true,
        tlv::Str{"€€"},
        tlv::Blob{three},
        true,
        std::int32_t{5},
        std::int32_t{-2147483647 - 1},
        std::int64_t{2147483648},
        std::int64_t{9007199254740991},
        1.5,
        std::numeric_limits<double>::quiet_NaN(),
        std::numeric_limits<std::int64_t>::min(),
        tlv::Str{"s"},
        tlv::Blob{one},
    };
    XPUTE_CHECK(w.write_all(vals).ok());
    const std::vector<std::uint8_t> &pkt = w.finish();
    XPUTE_CHECK_EQ(hex(pkt), v.s("pkt"), "the packet is not the other languages' bytes");

    std::vector<tlv::Value> read;
    XPUTE_CHECK(tlv::Reader(pkt).read_all(read).ok());
    XPUTE_CHECK_EQ(read.size(), v.len("read"), "read back a different count");
    for (std::size_t j = 0; j < read.size(); j++) XPUTE_CHECK_EQ(show(read[j]), v.s("read." + std::to_string(j)), ("read." + std::to_string(j)).c_str());

    // Each case gives the values read before the sequence stopped, then how it
    // stopped: the tag `end` read, the input run out between two values, or a
    // fault.
    v.each("end", [&](const std::string &k) {
        std::vector<std::uint8_t> pkt = v.bytes(k + ".pkt");
        tlv::Reader r(pkt);
        int n = 0;
        std::string stop;
        for (;;) {
            Result<std::optional<tlv::Value>> got = r.next();
            if (!got) {
                stop = "errno:" + std::to_string(static_cast<int>(got.error().errnum));
                break;
            }
            if (!*got) {
                stop = r.end() == tlv::End::closed ? "closed" : "unclosed";
                break;
            }
            n++;
        }
        XPUTE_CHECK_EQ(std::to_string(n) + ":" + stop, v.s(k + ".read"), k.c_str());
    });

    const std::string &huge_hex = v.s("huge.pkt");
    std::vector<std::uint8_t> huge(huge_hex.size() / 2);
    XPUTE_CHECK(hex_decode(huge_hex, huge).ok());
    std::vector<tlv::Value> none;
    Result<void> r = tlv::Reader(huge).read_all(none);
    std::string got = r.ok() ? "ok" : "errno:" + std::to_string(static_cast<int>(r.error().errnum));
    XPUTE_CHECK_EQ(got, v.s("huge.read"), "a length past the buffer");
}

XPUTE_TEST(tlv_sealed_writer_refuses) {
    std::vector<std::uint8_t> buf;
    tlv::Writer w(buf);
    w.finish();
    XPUTE_CHECK(!w.write(std::uint8_t{1}).ok());
}
