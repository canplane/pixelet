// cpp/xpute/runtime/mem/heap.hpp

// Blocks out of the heap: bytes asked for and given back.
//
// The allocator is not this file's. Which one serves is the guest's own
// choice, installed where it is assembled (`set_heap`) as `#[global_allocator]`
// installs Rust's, over its heap section — xpute's `alloc/` has two that
// fit. The same assembly points the global `operator new` and `operator
// delete` at it, so a standard container a guest uses allocates from the
// guest's heap as a block does, and never from memory the guest was not
// given. What is here is the door, and the shape a block travels in.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <utility>

#include "memory.hpp"

namespace xpute {

// A block of the heap: where it lies in the memory, as an offset, and its
// bytes. Two numbers rather than a container, because what holds a block
// names it by number — a list that uploads it, a host that writes into the
// memory at an offset — and the size travels with the address, since giving
// a block back needs both.
struct Block {
    std::uint32_t at;
    std::uint32_t bytes;
    bool operator==(const Block &) const = default;
};

// Every lane element a block is cut into — bytes up to a double, a u64 and
// records of them — aligns within this.
inline constexpr std::size_t BLOCK_ALIGN = 8;

// The guest's allocator: bytes at an alignment, none for no room, and the
// same size and alignment told back when they are given back.
struct Heap {
    void *(*alloc)(std::size_t bytes, std::size_t align) noexcept;
    void (*free)(void *at, std::size_t bytes, std::size_t align) noexcept;
};

// Installs the guest's allocator, before anything is allocated.
void set_heap(Heap heap) noexcept;

// The installed allocator, both of its calls null until one is.
Heap heap() noexcept;

// A block of `bytes` from the guest's allocator. None when it has no room or
// none is installed, and for zero bytes, which an allocator is never asked
// for.
inline std::optional<Block> alloc(Memory mem, std::uint32_t bytes) noexcept {
    Heap h = heap();
    if (bytes == 0 || h.alloc == nullptr) return std::nullopt;
    void *p = h.alloc(bytes, BLOCK_ALIGN);
    if (p == nullptr) return std::nullopt;
    return Block{mem.off_of(p), bytes};
}

// Gives a block back. `block` came from `alloc` over the same memory and has
// not been given back.
inline void free(Memory mem, Block block) noexcept {
    heap().free(mem.at(block.at), block.bytes, BLOCK_ALIGN);
}

// A block given back when it goes out of scope, as `ArenaGuard` rewinds an
// arena: for work that can still fail after taking it. `keep` hands the
// block on.
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
