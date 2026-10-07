// cpp/guest/mem/heap.cpp

#include "heap.hpp"

namespace xpute {

namespace {
// The platform's allocator is never a guest's: on wasm it links the libc that
// prints its failures through WASI.
Heap installed{nullptr, nullptr};
} // namespace

void set_heap(Heap heap) noexcept {
    installed = heap;
}

Heap heap() noexcept {
    return installed;
}

} // namespace xpute
