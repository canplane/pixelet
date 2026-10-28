// cpp/xpute/guest/mem/heap.test.cpp

#include "heap.hpp"

#include <cstdlib>

#include "xpute/kit/alloc/slab.hpp"
#include "xpute/kit/test.hpp"

using namespace xpute;

namespace {

constexpr std::size_t MAX = 20;

struct HeapRange {
    static constexpr std::size_t MIN_LOG2 = 12;
    static constexpr std::size_t MAX_LOG2 = MAX;
    static std::uintptr_t base() noexcept {
        static void *at = std::aligned_alloc(std::size_t{1} << MAX, std::size_t{1} << MAX);
        if (at == nullptr) test::fail(__FILE__, __LINE__, "the test could not reserve its own range");
        return reinterpret_cast<std::uintptr_t>(at);
    }
    static std::uintptr_t end() noexcept { return base() + (std::size_t{1} << MAX); }
};

SlabMalloc<HeapRange> slab;

} // namespace

XPUTE_TEST(heap_block_offset_and_guard) {
    XPUTE_CHECK_EQ(alloc(8), std::nullopt, "no allocator installed, no block");
    set_heap(Allocator{[](std::size_t bytes, std::size_t align) noexcept { return slab.alloc(bytes, align); },
                  [](void *at, std::size_t bytes, std::size_t align) noexcept { slab.dealloc(at, bytes, align); }});
    mem::set(HeapRange::base(), HeapRange::end() - HeapRange::base());

    XPUTE_CHECK_EQ(alloc(0), std::nullopt, "an allocator is never asked for nothing");
    Block b = *alloc(24);
    XPUTE_CHECK(b.bytes == 24 && b.at % BLOCK_ALIGN == 0 && b.at < (1u << MAX));
    XPUTE_CHECK(mem::off_of(mem::at(b.at)) == b.at);
    free(b);

    XPUTE_CHECK_EQ(alloc(1u << (MAX + 1)), std::nullopt, "and a block past the range is none, not a bug");

    HeapStat stat = heap_stat(slab);
    XPUTE_CHECK(stat.range == slab.range_bytes() && stat.used == slab.used() && stat.live == slab.live());
    XPUTE_CHECK(stat.run == slab.run_live() && stat.slot == slab.slot_live() && stat.largest_free == slab.largest_free());
    XPUTE_CHECK(stat.releases == slab.releases() && stat.refusals == slab.refusals() && stat.requests == slab.pages().allocs());
    XPUTE_CHECK_EQ(stat.refusals, 1u, "the block past the range was the one refusal");
    XPUTE_CHECK_EQ(stat.slot, 0u, "every slot went back");
    set_heap(Allocator{nullptr, nullptr});
    mem::set(0);
}
