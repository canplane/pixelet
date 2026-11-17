// cpp/xpute/link/cabi.test.cpp

#include <string>
#include <vector>

#include "dev/test/golden.hpp"
#include "dev/test/test.hpp"
#include "link.hpp"

using namespace xpute;
using link::Lent;
using link::Lift;
using link::Lower;
using link::Token;

namespace {

// The record's packet: one of each number, a string, a list of numbers, a URI
// and a lent buffer, as the Canonical ABI lays a tuple of them.
constexpr std::size_t FIXED_SZ = 88;

const std::vector<Token> URI{Token{Token::Int{5}}, Token{Token::Str{"f"}}};

Result<std::size_t> lower(std::span<std::uint8_t> buf) {
    Lower w(buf, FIXED_SZ);
    w.put<bool>(0, true);
    w.put<std::int8_t>(1, -3);
    w.put<std::uint16_t>(2, 65535);
    w.put<std::int32_t>(4, -7);
    w.put<std::uint64_t>(8, std::uint64_t{1} << 63);
    w.put<float>(16, 0.25f);
    w.put<double>(24, -1.5);
    w.put<std::uint8_t>(32, 200);
    w.put<std::int16_t>(34, -2);
    w.put<std::uint32_t>(36, 4000000000u);
    w.put<std::int64_t>(40, -(std::int64_t{1} << 40));
    w.str(48, "€€");
    const std::uint16_t list[] = {1, 2, 65535};
    w.arr<std::uint16_t>(56, list);
    link::store_uri(w, 64, URI);
    Lent{.buf = 4096, .nbyte = 64}.store(w, 72);
    return w.finish();
}

} // namespace

// The packet lowers as the record holds it, and only into a buffer that holds
// it whole.
XPUTE_TEST(cabi_lower_matches_record) {
    std::vector<std::uint8_t> want = Golden::load("link/spec/golden/cabi.tsv").bytes("pkt");
    std::vector<std::uint8_t> buf(want.size(), 0xaa);
    Result<std::size_t> n = lower(buf);
    XPUTE_CHECK(n.ok() && *n == want.size());
    XPUTE_CHECK_EQ(buf, want, "the packet lowered as other bytes");
    std::vector<std::uint8_t> short_(want.size() - 1);
    Result<std::size_t> refused = lower(short_);
    XPUTE_CHECK(!refused.ok() && refused.error().errnum == Errno::emsgsize);
}

// The record's packet lifts as the values it was lowered from.
XPUTE_TEST(cabi_lift_matches_record) {
    std::vector<std::uint8_t> pkt = Golden::load("link/spec/golden/cabi.tsv").bytes("pkt");
    Lift r{std::span<const std::uint8_t>(pkt)};
    XPUTE_CHECK(r.get<bool>(0));
    XPUTE_CHECK(r.get<std::int8_t>(1) == -3);
    XPUTE_CHECK(r.get<std::uint16_t>(2) == 65535);
    XPUTE_CHECK(r.get<std::int32_t>(4) == -7);
    XPUTE_CHECK(r.get<std::uint64_t>(8) == std::uint64_t{1} << 63);
    XPUTE_CHECK(r.get<float>(16) == 0.25f);
    XPUTE_CHECK(r.get<double>(24) == -1.5);
    XPUTE_CHECK(r.get<std::uint8_t>(32) == 200);
    XPUTE_CHECK(r.get<std::int16_t>(34) == -2);
    XPUTE_CHECK(r.get<std::uint32_t>(36) == 4000000000u);
    XPUTE_CHECK(r.get<std::int64_t>(40) == -(std::int64_t{1} << 40));
    XPUTE_CHECK(r.str(48) == "€€");
    XPUTE_CHECK((r.arr<std::uint16_t>(56) == std::vector<std::uint16_t>{1, 2, 65535}));
    std::vector<Token> uri = link::load_uri(r, 64);
    auto *n = uri.size() == 2 ? std::get_if<Token::Int>(&uri[0].variant) : nullptr;
    auto *s = uri.size() == 2 ? std::get_if<Token::Str>(&uri[1].variant) : nullptr;
    XPUTE_CHECK(n && n->val == 5 && s && s->val == "f");
    Lent lent = Lent::load(r, 72);
    XPUTE_CHECK(lent.buf == 4096 && lent.nbyte == 64);
    XPUTE_CHECK(r.finish().ok());
}

// Each recorded packet that does not hold a URI is refused as one, EBADMSG.
XPUTE_TEST(cabi_refusals_match_record) {
    Golden g = Golden::load("link/spec/golden/cabi.tsv");
    g.each("refused", [&](const std::string &k) {
        std::vector<std::uint8_t> pkt = g.bytes(k);
        Lift r{std::span<const std::uint8_t>(pkt)};
        link::load_uri(r, 0);
        Result<void> f = r.finish();
        XPUTE_CHECK(!f.ok() && f.error().errnum == Errno::ebadmsg);
    });
}
