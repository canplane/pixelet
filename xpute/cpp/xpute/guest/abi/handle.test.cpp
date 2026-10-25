// cpp/xpute/guest/abi/handle.test.cpp

#include "handle.hpp"

#include "xpute/kit/test.hpp"

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

XPUTE_TEST(slots_a_removed_handle_names_nothing_even_once_its_slot_is_taken_again) {
    Slots<int, 4> s;
    std::uint32_t a = s.insert(1);
    XPUTE_CHECK(a != 0);
    XPUTE_CHECK(s.get(a) != nullptr && *s.get(a) == 1);
    XPUTE_CHECK(s.remove(a) == 1);
    XPUTE_CHECK(!s.remove(a));
    std::uint32_t b = s.insert(2);
    XPUTE_CHECK(handle_slot(b) == handle_slot(a));
    XPUTE_CHECK(s.get(a) == nullptr && s.get(b) != nullptr && *s.get(b) == 2);
    std::uint32_t seen = 0, count = 0;
    s.each([&](std::uint32_t h, int &) {
        seen = h;
        count++;
    });
    XPUTE_CHECK(count == 1 && seen == b);
    XPUTE_CHECK(s.get(0) == nullptr);
}
