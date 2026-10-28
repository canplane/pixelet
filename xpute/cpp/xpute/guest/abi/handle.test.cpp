// cpp/xpute/guest/abi/handle.test.cpp

#include "handle.hpp"

#include <variant>

#include "xpute/kit/test.hpp"

using namespace xpute;
using namespace xpute::abi;

XPUTE_TEST(handle_full_table_refuses) {
    Table<std::monostate, 3> t;
    std::uint32_t h0 = *t.insert({});
    XPUTE_CHECK(t.insert({}).has_value());
    XPUTE_CHECK(!t.insert({}));
    XPUTE_CHECK(t.count() == 2);
    t.remove(h0);
    XPUTE_CHECK(t.insert({}).has_value());
}

XPUTE_TEST(handle_removed_stale) {
    Table<int, 4> t;
    std::uint32_t a = *t.insert(1);
    XPUTE_CHECK(a != 0);
    XPUTE_CHECK(t.get(a) != nullptr && *t.get(a) == 1);
    XPUTE_CHECK(t.remove(a) == 1);
    XPUTE_CHECK(!t.remove(a));
    std::uint32_t b = *t.insert(2);
    XPUTE_CHECK(handle_slot(b) == handle_slot(a));
    XPUTE_CHECK(t.get(a) == nullptr && t.get(b) != nullptr && *t.get(b) == 2);
    std::uint32_t seen = 0, count = 0;
    t.each([&](std::uint32_t h, int &) {
        seen = h;
        count++;
    });
    XPUTE_CHECK(count == 1 && seen == b);
    XPUTE_CHECK(t.get(0) == nullptr);
}

XPUTE_TEST(handle_rollback_epoch) {
    Table<std::uint8_t, 8> t;
    std::uint32_t kept = *t.insert(1);
    t.begin();
    std::uint32_t made = *t.insert(2);
    std::uint32_t gone = 0, value = 0;
    t.rollback([&](std::uint32_t h, std::uint8_t v) {
        gone = h;
        value = v;
    });
    XPUTE_CHECK(gone == made && value == 2);
    XPUTE_CHECK(t.live(kept) && !t.live(made));
}
