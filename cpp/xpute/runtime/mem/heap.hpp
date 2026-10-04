// cpp/xpute/runtime/mem/heap.hpp

// Blocks out of the guest's heap. The allocator is installed by the guest
// (`set_heap`), and `operator new` points at it too, so nothing allocates from
// memory the guest was not given.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <utility>

#include "memory.hpp"

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
struct Heap {
    void *(*alloc)(std::size_t bytes, std::size_t align) noexcept;
    void (*free)(void *at, std::size_t bytes, std::size_t align) noexcept;
};

void set_heap(Heap heap) noexcept;

Heap heap() noexcept;

// None for no room, no allocator, or zero bytes.
inline std::optional<Block> alloc(Memory mem, std::uint32_t bytes) noexcept {
    Heap h = heap();
    if (bytes == 0 || h.alloc == nullptr) return std::nullopt;
    void *p = h.alloc(bytes, BLOCK_ALIGN);
    if (p == nullptr) return std::nullopt;
    return Block{mem.off_of(p), bytes};
}

inline void free(Memory mem, Block block) noexcept {
    heap().free(mem.at(block.at), block.bytes, BLOCK_ALIGN);
}

// For work that can still fail after taking the block; `keep` hands it on.
class BlockGuard {
  public:
    static std::optional<BlockGuard> alloc(Memory mem, std::uint32_t bytes) noexcept {
        std::optional<Block> b = xpute::alloc(mem, bytes);
        if (!b) return std::nullopt;
        return BlockGuard(mem, *b);
    }

    BlockGuard(BlockGuard &&o) noexcept : mem_(o.mem_), block_(std::exchange(o.block_, std::nullopt)) {}
    BlockGuard(const BlockGuard &) = delete;
    BlockGuard &operator=(const BlockGuard &) = delete;
    BlockGuard &operator=(BlockGuard &&) = delete;
    ~BlockGuard() {
        if (block_) xpute::free(mem_, *block_);
    }

    const Block &block() const noexcept { return *block_; }

    Block keep() noexcept { return *std::exchange(block_, std::nullopt); }

  private:
    BlockGuard(Memory mem, Block block) noexcept : mem_(mem), block_(block) {}

    Memory mem_;
    std::optional<Block> block_;
};

} // namespace xpute
