// cpp/xpute/kit/alloc/alloc.test.cpp

#include <cstdlib>
#include <cstring>
#include <tuple>
#include <vector>

#include "xpute/kit/test.hpp"
#include "buddy_tree.hpp"
#include "slab.hpp"

using namespace xpute;

namespace {

constexpr std::size_t MIN = 12;
constexpr std::size_t MAX = 20;
constexpr std::size_t PAGE = std::size_t{1} << MIN;

// Memory the test owns, so a block can be written and read back, which is how
// an overlap shows.
template <int Tag> struct TestRange {
    static constexpr std::size_t MIN_LOG2 = MIN;
    static constexpr std::size_t MAX_LOG2 = MAX;
    static std::uintptr_t base() noexcept {
        static void *at = std::aligned_alloc(std::size_t{1} << MAX, std::size_t{1} << MAX);
        if (at == nullptr) test::fail(__FILE__, __LINE__, "the test could not reserve its own range");
        return reinterpret_cast<std::uintptr_t>(at);
    }
    static std::uintptr_t end() noexcept { return base() + (std::size_t{1} << MAX); }
};

bool all(const void *p, std::size_t n, std::uint8_t byte) {
    const auto *b = static_cast<const std::uint8_t *>(p);
    for (std::size_t i = 0; i < n; i++) {
        if (b[i] != byte) return false;
    }
    return true;
}

} // namespace

XPUTE_TEST(buddy_block_contents_survive) {
    using R = TestRange<0>;
    static BuddyMalloc<R> heap;
    const std::size_t sizes[] = {1, 4096, 4097, 8192, 100, 16384, 40, 65536};
    std::vector<std::tuple<void *, std::size_t, std::uint8_t>> live;
    for (std::size_t i = 0; i < 24; i++) {
        std::size_t sz = sizes[i % 8];
        void *p = heap.malloc(sz);
        if (p == nullptr) continue;
        auto at = reinterpret_cast<std::uintptr_t>(p);
        XPUTE_CHECK(at >= R::base() && at + sz <= R::end());
        std::memset(p, static_cast<int>(i), sz);
        live.emplace_back(p, sz, static_cast<std::uint8_t>(i));
    }
    XPUTE_CHECK(live.size() > 8);
    std::vector<std::tuple<void *, std::size_t, std::uint8_t>> kept;
    for (std::size_t j = 0; j < live.size(); j++) {
        if (j % 2 == 0) heap.free(std::get<0>(live[j]), std::get<1>(live[j]));
        else kept.push_back(live[j]);
    }
    for (std::size_t sz : sizes) {
        if (void *p = heap.malloc(sz)) std::memset(p, 0xff, sz);
    }
    for (auto &[p, sz, byte] : kept) XPUTE_CHECK_EQ(all(p, sz, byte), true, "a block was written over");
}

XPUTE_TEST(buddy_buddies_merge) {
    static BuddyMalloc<TestRange<1>> heap;
    void *a = heap.malloc(PAGE);
    void *b = heap.malloc(PAGE);
    XPUTE_CHECK(a != nullptr && b != nullptr);
    void *low = a < b ? a : b;
    heap.free(a, PAGE);
    heap.free(b, PAGE);
    XPUTE_CHECK_EQ(heap.malloc(PAGE * 2), low, "the pair did not join");
}

XPUTE_TEST(buddy_oversize_refused) {
    static BuddyMalloc<TestRange<2>> heap;
    void *first = heap.malloc(PAGE);
    XPUTE_CHECK(first != nullptr);
    std::memset(first, 0x5a, PAGE);
    std::size_t served = 1;
    while (heap.malloc(PAGE) != nullptr) {
        served++;
        XPUTE_CHECK(served <= (std::size_t{1} << (MAX - MIN)) + 1);
    }
    XPUTE_CHECK(heap.live() <= heap.range_bytes());
    XPUTE_CHECK_EQ(all(first, PAGE, 0x5a), true, "a refusal wrote over a live block");
}

XPUTE_TEST(buddy_freed_reused) {
    static BuddyMalloc<TestRange<3>> heap;
    std::size_t sz = std::size_t{1} << (MIN + 2);
    void *p = heap.malloc(sz);
    XPUTE_CHECK(p != nullptr);
    std::size_t reached = heap.used();
    for (int i = 0; i < 8; i++) {
        heap.free(p, sz);
        XPUTE_CHECK_EQ(heap.malloc(sz), p, "the same request after the same free moved");
        XPUTE_CHECK_EQ(heap.used(), reached, "a reuse reached further into the range");
    }
    XPUTE_CHECK(heap.releases() >= 8);
}

XPUTE_TEST(slab_slot_contents_survive) {
    static SlabMalloc<TestRange<4>> heap;
    const std::size_t sizes[] = {1, 16, 17, 24, 64, 200, 1000, 4096, 9000};
    std::vector<std::tuple<void *, std::size_t, std::uint8_t>> live;
    for (std::size_t i = 0; i < 90; i++) {
        std::size_t sz = sizes[i % 9];
        void *p = heap.alloc(sz, 8);
        if (p == nullptr) continue;
        std::memset(p, static_cast<int>(i), sz);
        live.emplace_back(p, sz, static_cast<std::uint8_t>(i));
    }
    XPUTE_CHECK(live.size() > 40);
    std::vector<std::tuple<void *, std::size_t, std::uint8_t>> kept;
    for (std::size_t j = 0; j < live.size(); j++) {
        if (j % 2 == 0) heap.dealloc(std::get<0>(live[j]), std::get<1>(live[j]), 8);
        else kept.push_back(live[j]);
    }
    for (std::size_t sz : sizes) {
        if (void *p = heap.alloc(sz, 8)) std::memset(p, 0xff, sz);
    }
    for (auto &[p, sz, byte] : kept) XPUTE_CHECK_EQ(all(p, sz, byte), true, "a slot was written over");
}

XPUTE_TEST(slab_empty_run_returned) {
    static SlabMalloc<TestRange<5>> heap;
    void *first = heap.alloc(24, 8);
    XPUTE_CHECK(first != nullptr);
    std::size_t one_run = heap.pages().live();
    XPUTE_CHECK(one_run >= PAGE);
    std::vector<void *> live{first};
    while (heap.pages().live() == one_run) {
        void *p = heap.alloc(24, 8);
        XPUTE_CHECK(p != nullptr);
        live.push_back(p);
    }
    // The one that made a second run goes back first.
    heap.dealloc(live.back(), 24, 8);
    live.pop_back();
    XPUTE_CHECK_EQ(heap.pages().live(), one_run, "the second run did not go back");
    for (void *p : live) heap.dealloc(p, 24, 8);
    XPUTE_CHECK_EQ(heap.pages().live(), std::size_t{0}, "the last slot of a run did not give the run back");
}

XPUTE_TEST(slab_page_requests_delegated) {
    static SlabMalloc<TestRange<6>> heap;
    for (std::size_t sz : {PAGE, PAGE + 1, PAGE * 4}) {
        std::size_t before = heap.pages().live(), run = heap.run_live(), slot = heap.slot_live();
        void *p = heap.alloc(sz, 8);
        XPUTE_CHECK(p != nullptr);
        XPUTE_CHECK_EQ(heap.run_live() == run && heap.slot_live() == slot, true, "a whole-page request took a run or a slot");
        XPUTE_CHECK(heap.pages().live() - before >= sz);
        XPUTE_CHECK_EQ(reinterpret_cast<std::uintptr_t>(p) % PAGE, std::uintptr_t{0}, "a whole-page request was not page-aligned");
        heap.dealloc(p, sz, 8);
        XPUTE_CHECK_EQ(heap.pages().live(), before, "it did not go back whole");
    }
}

XPUTE_TEST(slab_slots_inside_runs) {
    static SlabMalloc<TestRange<7>> heap;
    const std::size_t sizes[] = {16, 24, 64, 300, 1000, PAGE, PAGE * 3};
    std::vector<std::pair<void *, std::size_t>> live;
    auto check = [] {
        XPUTE_CHECK(heap.slot_live() <= heap.run_live());
        XPUTE_CHECK(heap.run_live() <= heap.pages().live());
        XPUTE_CHECK(heap.pages().live() <= heap.range_bytes());
    };
    for (int round = 0; round < 4; round++) {
        for (std::size_t k = 0; k < 7; k++) {
            void *p = heap.alloc(sizes[k], 8);
            XPUTE_CHECK(p != nullptr);
            live.emplace_back(p, sizes[k]);
            check();
            if (k % 2 == 1) {
                heap.dealloc(live.front().first, live.front().second, 8);
                live.erase(live.begin());
                check();
            }
        }
    }
    for (auto &[p, sz] : live) {
        heap.dealloc(p, sz, 8);
        check();
    }
    XPUTE_CHECK(heap.slot_live() == 0 && heap.run_live() == 0 && heap.pages().live() == 0);
}
