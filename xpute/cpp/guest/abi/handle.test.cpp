// cpp/guest/abi/handle.test.cpp

#include "handle.hpp"

#include "../../kit/test.hpp"

using namespace xpute;

XPUTE_TEST(handle_a_full_table_refuses) {
    HandleTable<3> t;
    std::uint32_t h0 = *t.acquire();
    XPUTE_CHECK(t.acquire().has_value());
    XPUTE_CHECK(!t.acquire());
    XPUTE_CHECK(t.count() == 2);
    t.release(h0);
    XPUTE_CHECK(t.acquire().has_value());
}
