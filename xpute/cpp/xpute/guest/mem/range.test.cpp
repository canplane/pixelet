// cpp/xpute/guest/mem/range.test.cpp

#include "range.hpp"

#include "xpute/kit/test.hpp"

using namespace xpute;

XPUTE_TEST(mem_arr_laid_on_lines) {
    XPUTE_CHECK(arr_bytes(0, 4) == 0);
    XPUTE_CHECK(arr_bytes(1, 4) == 64);
    XPUTE_CHECK(arr_bytes(16, 4) == 64);
    XPUTE_CHECK(arr_bytes(17, 4) == 128);
}

XPUTE_TEST(mem_owned_range_handed_once) {
    OwnedRange range;
    Section s{.off = 4096, .bytes = 64};
    range.set(s);
    XPUTE_CHECK(range.get() == s);
}
