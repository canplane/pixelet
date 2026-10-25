// cpp/xpute/guest/mem/heap.hpp

// Blocks out of the guest's heap, named by offsets in its memory
// (`mem::core`). The allocator is installed by the guest (`set_heap`), and
// `operator new` points at it too, so nothing allocates from memory the guest
// was not given.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <utility>

#include "xpute/kit/alloc/slab.hpp"
#include "xpute/guest/mem.hpp"

namespace xpute {

// An offset and a size rather than a pointer, because a host names a block by
// its offset, and giving a block back needs the size.
struct Block {
    std::uint32_t at;
    std::uint32_t bytes;
    bool operator==(const Block &) const = default;
};

inline constexpr std::size_t BLOCK_ALIGN = 8;

// The same size and alignment are told back on free.
struct Allocator {
    void *(*alloc)(std::size_t bytes, std::size_t align) noexcept;
    void (*free)(void *at, std::size_t bytes, std::size_t align) noexcept;
};

void set_heap(Allocator heap) noexcept;

// One reading of a heap, so compared fields are from one moment.
struct HeapStat {
    std::size_t range = 0;
    // A high-water mark, not a holding.
    std::size_t used = 0;
    std::size_t live = 0;
    std::size_t run = 0;
    std::size_t slot = 0;
    // Past the tree's current root the range is untouched rather than listed,
    // so this is not the free space.
    std::size_t largest_free = 0;
    // Moves when a page is released, which is when a request of a page or
    // more refused before might now be served; a slot returned to a run still
    // in use does not move it (`SlabMalloc::releases`).
    std::uint32_t releases = 0;
    std::uint32_t refusals = 0;
    std::uint32_t requests = 0;

    bool operator==(const HeapStat &) const = default;
};

// Reads the slab whose calls the guest installed as its heap (`set_heap`).
template <Range R> HeapStat heap_stat(const SlabMalloc<R> &a) noexcept {
    return HeapStat{
        .range = a.range_bytes(),
        .used = a.used(),
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

// None for no room, no allocator, or zero bytes.
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
