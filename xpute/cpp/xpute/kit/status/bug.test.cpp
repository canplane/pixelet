// cpp/xpute/kit/status/bug.test.cpp

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <source_location>
#include <string_view>
#include <vector>

#include "bug.hpp"
#include "dev/test/golden.hpp"
#include "dev/test/test.hpp"
#include "verbose_abort.hpp"

namespace {

using namespace xpute;

// What the report was last handed.
Errno reported_errnum = Errno::ok;
std::uint64_t reported_a = 0;
std::string_view reported_msg;

void install() {
    set_report([](Errno errnum, const std::source_location &, std::uint64_t a, std::uint64_t, std::string_view msg) {
        reported_errnum = errnum;
        reported_a = a;
        reported_msg = msg;
    });
}

} // namespace

XPUTE_TEST(bug_reports_its_msg) {
    install();
    XPUTE_CHECK(caught(true, []() -> bool { bug(Errno::enospc, 7, 0, "full"); }));
    XPUTE_CHECK(reported_errnum == Errno::enospc && reported_a == 7);
    XPUTE_CHECK(reported_msg == "full");
    XPUTE_CHECK(caught(true, []() -> bool { bug(Errno::enospc); }));
    XPUTE_CHECK(reported_msg.empty());
    // A message past the cap is cut as the record of the cut holds it
    // (kit/spec/golden/status/msg.tsv), a view of the message itself.
    Golden g = Golden::load("kit/spec/golden/status/msg.tsv");
    static std::vector<std::uint8_t> in = g.bytes("straddle.in");
    std::vector<std::uint8_t> out = g.bytes("straddle.out");
    static std::string_view longer(reinterpret_cast<const char *>(in.data()), in.size());
    XPUTE_CHECK(caught(true, []() -> bool { bug(Errno::enospc, 0, 0, longer); }));
    XPUTE_CHECK(reported_msg == std::string_view(reinterpret_cast<const char *>(out.data()), out.size()));
    XPUTE_CHECK(reported_msg.data() == longer.data());
}

XPUTE_TEST(verbose_abort_reports_libcxx_msg) {
    install();
    XPUTE_CHECK(caught(true, []() -> bool { xpute_verbose_abort("%s:%d: %s", "vector", -7, "out of range"); }));
    XPUTE_CHECK(reported_errnum == Errno::enotrecoverable);
    XPUTE_CHECK(reported_msg == "vector:-7: out of range");
    XPUTE_CHECK(caught(true, []() -> bool { xpute_verbose_abort("%zu of %lu, %x%%, %c", std::size_t{3}, 9ul, 255u, 'k'); }));
    XPUTE_CHECK(reported_msg == "3 of 9, ff%, k");
    // From a conversion it does not write on, the format goes as it is.
    XPUTE_CHECK(caught(true, []() -> bool { xpute_verbose_abort("%d at %p", 1, nullptr); }));
    XPUTE_CHECK(reported_msg == "1 at %p");
    // A message past the cap is cut.
    static char longer[1024];
    std::memset(longer, 'x', sizeof longer - 1);
    XPUTE_CHECK(caught(true, []() -> bool { xpute_verbose_abort("%s", longer); }));
    XPUTE_CHECK(reported_msg.size() == MSG_CAP);
}
