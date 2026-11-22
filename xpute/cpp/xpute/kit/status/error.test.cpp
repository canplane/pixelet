// cpp/xpute/kit/status/error.test.cpp

#include <cstdint>
#include <string>
#include <string_view>

#include "dev/test/test.hpp"
#include "error.hpp"

using namespace xpute;

// An error is placed where it is made; its message past the cap is cut at a
// character boundary, and an empty one is none.
XPUTE_TEST(error_is_placed_and_its_msg_cut) {
    Error e = make_error(Errno::eio, "no café");
    XPUTE_CHECK(e.file.ends_with("error.test.cpp") && e.line != 0 && e.msg == "no café");
    std::string long_msg = "x";
    for (std::uint32_t k = 0; k < MSG_CAP / 2; k++) long_msg += "é";
    XPUTE_CHECK(make_error(Errno::eio, long_msg).msg == std::string_view(long_msg).substr(0, MSG_CAP - 1));
    XPUTE_CHECK(make_error(Errno::eio, "").msg.empty());
    Error came = make_error(Errno::eio, "a/b.rs", 7, "");
    XPUTE_CHECK(came.file == "a/b.rs" && came.line == 7 && came.msg.empty());
}
