// cpp/xpute/kit/status/bug.test.cpp

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <source_location>

#include "bug.hpp"
#include "dev/test/test.hpp"
#include "verbose_abort.hpp"

namespace {

using namespace xpute;

// What the report was last handed.
Errno reported_errnum = Errno::ok;
std::uint64_t reported_a = 0;
const char *reported_msg = nullptr;

void install() {
    set_report([](Errno errnum, const std::source_location &, std::uint64_t a, std::uint64_t, const char *msg) {
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
    XPUTE_CHECK(reported_msg != nullptr && std::strcmp(reported_msg, "full") == 0);
    XPUTE_CHECK(caught(true, []() -> bool { bug(Errno::enospc); }));
    XPUTE_CHECK(reported_msg == nullptr);
}

XPUTE_TEST(verbose_abort_reports_libcxx_msg) {
    install();
    XPUTE_CHECK(caught(true, []() -> bool { xpute_verbose_abort("%s:%d: %s", "vector", -7, "out of range"); }));
    XPUTE_CHECK(reported_errnum == Errno::enotrecoverable);
    XPUTE_CHECK(reported_msg != nullptr && std::strcmp(reported_msg, "vector:-7: out of range") == 0);
    XPUTE_CHECK(caught(true, []() -> bool { xpute_verbose_abort("%zu of %lu, %x%%, %c", std::size_t{3}, 9ul, 255u, 'k'); }));
    XPUTE_CHECK(std::strcmp(reported_msg, "3 of 9, ff%, k") == 0);
    // From a conversion it does not write on, the format goes as it is.
    XPUTE_CHECK(caught(true, []() -> bool { xpute_verbose_abort("%d at %p", 1, nullptr); }));
    XPUTE_CHECK(std::strcmp(reported_msg, "1 at %p") == 0);
    // A message past the buffer is cut, and ends.
    static char longer[1024];
    std::memset(longer, 'x', sizeof longer - 1);
    XPUTE_CHECK(caught(true, []() -> bool { xpute_verbose_abort("%s", longer); }));
    XPUTE_CHECK(std::strlen(reported_msg) == 511);
}
