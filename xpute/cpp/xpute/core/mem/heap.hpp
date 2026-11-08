// cpp/xpute/core/mem/heap.hpp

// The core's heap hands out blocks named by their offsets in the core memory
// (`mem::core`). The core installs the allocator (`set_heap`), and
// `operator new` uses it too, so nothing allocates from memory the core was not
// given.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <utility>

#include "xpute/kit/alloc/slab.hpp"
#include "xpute/core/mem.hpp"

namespace xpute {

// An offset and a size: freeing needs both.
struct Block {
    std::uint32_t at;
    std::uint32_t bytes;
    bool operator==(const Block &) const = default;
};

inline constexpr std::size_t BLOCK_ALIGN = 8;

// `free` is given back the same size and alignment that `alloc` was given.
struct Allocator {
    void *(*alloc)(std::size_t bytes, std::size_t align) noexcept;
    void (*free)(void *at, std::size_t bytes, std::size_t align) noexcept;
};

void set_heap(Allocator heap) noexcept;

// One reading of a heap, so that fields compared with each other come from
// the same moment.
struct HeapStat {
    std::size_t range = 0;
    // The most ever live at once, not what is held now.
    std::size_t peak = 0;
    std::size_t live = 0;
    std::size_t run = 0;
    std::size_t slot = 0;
    // The largest free block the tree lists. The range past the tree's
    // current root is untouched rather than listed, so this is not the total
    // free space.
    std::size_t largest_free = 0;
    // Advances each time a page is released, which is when a request of a
    // page or more that was refused before might now be served. A slot
    // returned to a run still in use does not advance it
    // (`SlabMalloc::releases`).
    std::uint32_t releases = 0;
    std::uint32_t refusals = 0;
    std::uint32_t requests = 0;

    bool operator==(const HeapStat &) const = default;
};

// `heap_stat` reads the slab whose functions the core installed as its heap
// (`set_heap`).
template <Range R> HeapStat heap_stat(const SlabMalloc<R> &a) noexcept {
    return HeapStat{
        .range = a.range_bytes(),
        .peak = a.peak(),
        .live = a.live(),
        .run = a.run_live(),
        .slot = a.slot_live(),
        .largest_free = a.largest_free(),
        .releases = a.releases(),
        .refusals = a.refusals(),
        .requests = static_cast<std::uint32_t>(a.pages().allocs()),
    };
}

Allocator heap() noexcept;

// Answers none when there is no room, no allocator, or zero bytes.
inline std::optional<Block> alloc(std::uint32_t bytes) noexcept {
    Allocator h = heap();
    if (bytes == 0 || h.alloc == nullptr) return std::nullopt;
    void *p = h.alloc(bytes, BLOCK_ALIGN);
    if (p == nullptr) return std::nullopt;
    return Block{mem::off_of(p), bytes};
}

inline void free(Block block) noexcept {
    heap().free(mem::at(block.at), block.bytes, BLOCK_ALIGN);
}

} // namespace xpute
