// cpp/xpute/kit/alloc/heap.hpp

// A heap over the range of memory a program gives it (`Range`), as
// `xpute-kit`'s `alloc/heap.rs` is Rust's: the slab of size classes over the
// buddy allocator of pages (slab.hpp), as a `std::pmr::memory_resource`,
// which a program picks for what it allocates, as its default resource or a
// container's. A request the range cannot hold is a bug: a memory resource
// answers none with nothing. A program's default resource is every thread's,
// and the slab is one thread's, so where the target runs threads each call
// takes it under one lock (`Lock`): one flag, spun on and then yielded on, as
// Rust's heap takes it. A WebAssembly target without the `atomics` feature
// runs no threads, and takes none.

#pragma once

#include <cstddef>
#include <memory_resource>

#include "slab.hpp"
#include "xpute/kit/status/bug.hpp"

#if !defined(__wasm__) || defined(__wasm_atomics__)
#include <atomic>
#include <thread>
#endif

namespace xpute {

template <Range R> class HeapMalloc final : public std::pmr::memory_resource {
  public:
    HeapMalloc() noexcept = default;
    HeapMalloc(const HeapMalloc &) = delete;
    HeapMalloc &operator=(const HeapMalloc &) = delete;

    // The slab, whose counters the program reads.
    const SlabMalloc<R> &slab() const noexcept { return slab_; }

  private:
#if !defined(__wasm__) || defined(__wasm_atomics__)
    // The slab's lock, on a target that runs threads: whether a thread holds
    // the slab.
    class Lock {
      public:
        void take() noexcept {
            // How many times a thread spins on the lock before it gives its
            // time back to the scheduler on each try instead.
            constexpr unsigned SPIN_CNT = 100;
            for (unsigned spins = 0; locked_.test_and_set(std::memory_order_acquire);) {
                if (spins < SPIN_CNT) spins++;
                else std::this_thread::yield();
            }
        }

        void give() noexcept { locked_.clear(std::memory_order_release); }

      private:
        std::atomic_flag locked_;
    };
#else
    // The slab's lock, on a target that runs no threads: none, the slab held
    // by the one thread there is.
    class Lock {
      public:
        void take() noexcept {}
        void give() noexcept {}
    };
#endif

    void *do_allocate(std::size_t n, std::size_t align) override {
        lock_.take();
        void *at = slab_.alloc(n, align);
        lock_.give();
        ensure(at != nullptr, Errno::enomem, n, align);
        return at;
    }

    void do_deallocate(void *at, std::size_t n, std::size_t align) override {
        lock_.take();
        slab_.dealloc(at, n, align);
        lock_.give();
    }

    bool do_is_equal(const std::pmr::memory_resource &other) const noexcept override { return this == &other; }

    SlabMalloc<R> slab_;
    Lock lock_;
};

} // namespace xpute
