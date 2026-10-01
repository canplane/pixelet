// cpp/xpute/runtime/mem/heap.cpp

#include "heap.hpp"

namespace xpute {

namespace {
// None until the guest installs its own: the platform's allocator is never a
// guest's, and on WebAssembly reaching it links the libc that prints its
// failures through WASI.
Heap installed{nullptr, nullptr};
} // namespace

void set_heap(Heap heap) noexcept {
    installed = heap;
}

Heap heap() noexcept {
    return installed;
}

} // namespace xpute
