// cpp/xpute/kit/status/msg.test.cpp

#include <cstdint>
#include <string>
#include <string_view>
#include <vector>

#include "dev/test/golden.hpp"
#include "dev/test/test.hpp"
#include "msg.hpp"

using namespace xpute;

// Each recorded message is cut as the record holds it: one past the cap by a
// character of four bytes that straddles it, and by the three bytes a lone
// surrogate is as UTF-8.
XPUTE_TEST(msg_cut_matches_record) {
    Golden g = Golden::load("kit/spec/golden/status/msg.tsv");
    for (std::string k : {"straddle", "surrogate"}) {
        std::vector<std::uint8_t> in = g.bytes(k + ".in");
        std::vector<std::uint8_t> want = g.bytes(k + ".out");
        std::string_view cut = cut_msg(std::string_view(reinterpret_cast<const char *>(in.data()), in.size()));
        XPUTE_CHECK_EQ(std::vector<std::uint8_t>(cut.begin(), cut.end()), want, "the message cut as other bytes");
    }
}
