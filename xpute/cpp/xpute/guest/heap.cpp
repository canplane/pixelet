// cpp/xpute/guest/heap.cpp

#include "heap.hpp"

#include <algorithm>
#include <cstddef>
#include <cstring>
#include <new>

#include "xpute/kit/alloc/slab.hpp"
#include "xpute/kit/status/bug.hpp"
#include "xpute/link/mem.hpp"

namespace xpute::guest {

namespace {

// The heap's range: from the page past the share to the memory's end, in a
// tree of 2 GiB, the largest power of two short of the whole address space.
struct Span {
    static constexpr std::size_t MIN_LOG2 = 12;
    static constexpr std::size_t MAX_LOG2 = ADDR_BITS - 1;
    static std::uintptr_t base() noexcept { return link::mem::parts().heap; }
    static std::uintptr_t end() noexcept { return link::mem::parts().end; }
};
static_assert(std::size_t{1} << Span::MIN_LOG2 == link::mem::PAGE_SZ);

constinit SlabMalloc<Span> slab;

void *take(std::size_t n, std::size_t align) noexcept {
    void *at = slab.alloc(n, align);
    ensure(at != nullptr, Errno::enomem, n, align);
    return at;
}

class Heap final : public std::pmr::memory_resource {
    void *do_allocate(std::size_t n, std::size_t align) override { return take(n, align); }
    void do_deallocate(void *at, std::size_t n, std::size_t align) override { slab.dealloc(at, n, align); }
    bool do_is_equal(const std::pmr::memory_resource &other) const noexcept override { return this == &other; }
};

// Made by the first `heap()` and lasting as long as the guest, so no
// destructor of a static runs for it.
Heap *the_heap = nullptr;

#ifdef __wasm__
// A block of the global `new` carries its size in the bytes before it, 16
// or its alignment if more, since `operator delete` may be told no size and
// the slab takes a block back by its size.
constexpr std::size_t NEW_HDR_SZ = 16;

// What a `new` that names no alignment is aligned to.
constexpr std::size_t NEW_ALIGN = __STDCPP_DEFAULT_NEW_ALIGNMENT__;

void *new_block(std::size_t n, std::size_t align) noexcept {
    std::size_t hdr = std::max(align, NEW_HDR_SZ), sz = hdr + n;
    auto *at = static_cast<std::uint8_t *>(take(sz, hdr));
    std::memcpy(at + hdr - sizeof sz, &sz, sizeof sz);
    return at + hdr;
}

void delete_block(void *p, std::size_t align) noexcept {
    if (p == nullptr) return;
    std::size_t hdr = std::max(align, NEW_HDR_SZ), sz;
    auto *at = static_cast<std::uint8_t *>(p) - hdr;
    std::memcpy(&sz, at + hdr - sizeof sz, sizeof sz);
    slab.dealloc(at, sz, hdr);
}
#endif

} // namespace

std::uintptr_t share_at() noexcept {
    return link::mem::parts().share;
}

std::pmr::memory_resource &heap() noexcept {
    if (the_heap == nullptr) the_heap = new Heap;
    return *the_heap;
}

} // namespace xpute::guest

#ifdef __wasm__
void *operator new(std::size_t n) {
    return xpute::guest::new_block(n, xpute::guest::NEW_ALIGN);
}
void *operator new[](std::size_t n) {
    return xpute::guest::new_block(n, xpute::guest::NEW_ALIGN);
}
void *operator new(std::size_t n, std::align_val_t a) {
    return xpute::guest::new_block(n, static_cast<std::size_t>(a));
}
void *operator new[](std::size_t n, std::align_val_t a) {
    return xpute::guest::new_block(n, static_cast<std::size_t>(a));
}
void operator delete(void *p) noexcept {
    xpute::guest::delete_block(p, xpute::guest::NEW_ALIGN);
}
void operator delete[](void *p) noexcept {
    xpute::guest::delete_block(p, xpute::guest::NEW_ALIGN);
}
void operator delete(void *p, std::size_t) noexcept {
    xpute::guest::delete_block(p, xpute::guest::NEW_ALIGN);
}
void operator delete[](void *p, std::size_t) noexcept {
    xpute::guest::delete_block(p, xpute::guest::NEW_ALIGN);
}
void operator delete(void *p, std::align_val_t a) noexcept {
    xpute::guest::delete_block(p, static_cast<std::size_t>(a));
}
void operator delete[](void *p, std::align_val_t a) noexcept {
    xpute::guest::delete_block(p, static_cast<std::size_t>(a));
}
void operator delete(void *p, std::size_t, std::align_val_t a) noexcept {
    xpute::guest::delete_block(p, static_cast<std::size_t>(a));
}
void operator delete[](void *p, std::size_t, std::align_val_t a) noexcept {
    xpute::guest::delete_block(p, static_cast<std::size_t>(a));
}
#endif
