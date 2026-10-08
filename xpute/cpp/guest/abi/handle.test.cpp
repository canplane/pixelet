// cpp/guest/abi/handle.test.cpp

#include "handle.hpp"

#include <string>
#include <vector>

#include "../../kit/test.hpp"

using namespace xpute;

XPUTE_TEST(handle_a_released_handle_no_longer_names_its_slot_and_the_slot_s_next_handle_differs) {
    HandleTable<4> t;
    std::uint32_t a = *t.acquire();
    XPUTE_CHECK(t.live(a));
    XPUTE_CHECK(t.release(a));
    XPUTE_CHECK(!t.live(a));
    XPUTE_CHECK_EQ(t.release(a), false, "a handle is released once");
    std::uint32_t b = *t.acquire();
    XPUTE_CHECK_EQ(handle_slot(b), handle_slot(a), "the slot is reused");
    XPUTE_CHECK(b != a);
    XPUTE_CHECK(t.live(b) && !t.live(a));
}

XPUTE_TEST(handle_0_is_never_a_handle_and_a_full_table_refuses) {
    HandleTable<3> t;
    XPUTE_CHECK(!t.live(0));
    std::uint32_t h0 = *t.acquire(), h1 = *t.acquire();
    XPUTE_CHECK(h0 != 0 && handle_slot(h0) != 0 && h1 != 0 && handle_slot(h1) != 0);
    XPUTE_CHECK(!t.acquire());
    XPUTE_CHECK(t.count() == 2);
    t.release(h0);
    XPUTE_CHECK(t.acquire().has_value());
}

XPUTE_TEST(handle_a_freed_slot_is_the_next_one_taken) {
    HandleTable<8> t;
    std::uint32_t a = *t.acquire(), b = *t.acquire(), c = *t.acquire();
    t.release(a);
    t.release(c);
    XPUTE_CHECK_EQ(handle_slot(*t.acquire()), handle_slot(c), "the last freed first");
    XPUTE_CHECK_EQ(handle_slot(*t.acquire()), handle_slot(a), "then the one before it");
    XPUTE_CHECK(t.live(b));
}

XPUTE_TEST(slots_a_removed_handle_names_nothing_even_once_its_slot_is_taken_again) {
    Slots<std::string> s;
    std::uint32_t a = s.insert("a");
    XPUTE_CHECK(a != 0);
    XPUTE_CHECK(*s.get(a) == "a");
    XPUTE_CHECK(*s.remove(a) == "a");
    XPUTE_CHECK_EQ(s.remove(a), std::nullopt, "a handle is removed once");
    std::uint32_t b = s.insert("b");
    XPUTE_CHECK_EQ(handle_slot(b), handle_slot(a), "the slot is reused");
    XPUTE_CHECK(s.get(a) == nullptr && *s.get(b) == "b");
    std::vector<std::uint32_t> held;
    s.each([&](std::uint32_t h, std::string &) { held.push_back(h); });
    XPUTE_CHECK((held == std::vector{b}));
    XPUTE_CHECK_EQ(s.get(0), nullptr, "0 is never a handle");
}
