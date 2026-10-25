// cpp/xpute/guest/mem/heap.cpp

#include "heap.hpp"

namespace xpute {

namespace {
// The platform's allocator is never a guest's: on wasm it links the libc that
// prints its failures through WASI.
Allocator installed_heap{nullptr, nullptr};
} // namespace

void set_heap(Allocator heap) noexcept {
    installed_heap = heap;
}

Allocator heap() noexcept {
    return installed_heap;
}

} // namespace xpute
