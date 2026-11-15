// cpp/xpute/kit/alloc/heap.hpp

// A heap over the range of memory a program gives it (`Range`), as
// `xpute-kit`'s `alloc/heap.rs` is Rust's: the slab of size classes over the
// buddy allocator of pages (slab.hpp), as a `std::pmr::memory_resource`,
// which a program picks for what it allocates, as its default resource or a
// container's. A request the range cannot hold is a bug: a memory resource
// answers none with nothing. It is one thread's, as a memory resource with
// no synchronization of its own is.

#pragma once

#include <cstddef>
#include <memory_resource>

#include "slab.hpp"
#include "xpute/kit/status/bug.hpp"

namespace xpute {

template <Range R> class HeapMalloc final : public std::pmr::memory_resource {
  public:
    HeapMalloc() noexcept = default;
    HeapMalloc(const HeapMalloc &) = delete;
    HeapMalloc &operator=(const HeapMalloc &) = delete;

    // The slab, whose counters the program reads.
    const SlabMalloc<R> &slab() const noexcept { return slab_; }

  private:
    void *do_allocate(std::size_t n, std::size_t align) override {
        void *at = slab_.alloc(n, align);
        ensure(at != nullptr, Errno::enomem, n, align);
        return at;
    }

    void do_deallocate(void *at, std::size_t n, std::size_t align) override { slab_.dealloc(at, n, align); }

    bool do_is_equal(const std::pmr::memory_resource &other) const noexcept override { return this == &other; }

    SlabMalloc<R> slab_;
};

} // namespace xpute
