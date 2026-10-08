// cpp/guest/mem/heap.test.cpp

#include "heap.hpp"

#include <cstdlib>

#include "../../kit/alloc/slab.hpp"
#include "../../kit/test.hpp"

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

XPUTE_TEST(heap_a_block_is_named_by_its_offset_and_a_guard_gives_back_what_it_did_not_keep) {
    XPUTE_CHECK_EQ(alloc(Memory(0), 8), std::nullopt, "no allocator installed, no block");
    set_heap(Heap{[](std::size_t bytes, std::size_t align) noexcept { return slab.alloc(bytes, align); },
                  [](void *at, std::size_t bytes, std::size_t align) noexcept { slab.dealloc(at, bytes, align); }});
    Memory mem(HeapRange::base(), HeapRange::end() - HeapRange::base());

    XPUTE_CHECK_EQ(alloc(mem, 0), std::nullopt, "an allocator is never asked for nothing");
    Block b = *alloc(mem, 24);
    XPUTE_CHECK(b.bytes == 24 && b.at % BLOCK_ALIGN == 0 && b.at < (1u << MAX));
    XPUTE_CHECK(mem.off_of(mem.at(b.at)) == b.at);
    free(mem, b);

    Block kept = BlockGuard::alloc(mem, 16)->keep();
    XPUTE_CHECK(kept.bytes == 16);
    std::size_t live = slab.live();
    { std::optional<BlockGuard> g = BlockGuard::alloc(mem, 5000); }
    XPUTE_CHECK_EQ(slab.live(), live, "what a guard did not keep goes back");
    free(mem, kept);
    XPUTE_CHECK_EQ(alloc(mem, 1u << (MAX + 1)), std::nullopt, "and a block past the range is none, not a bug");
    set_heap(Heap{nullptr, nullptr});
}
