// cpp/xpute/core/mem/heap.cpp

#include "heap.hpp"

namespace xpute {

namespace {
// The platform's allocator is never the core's: on wasm, using it would link
// the libc, which prints its failures through WASI.
Allocator installed_heap{nullptr, nullptr};
} // namespace

void set_heap(Allocator heap) noexcept {
    installed_heap = heap;
}

Allocator heap() noexcept {
    return installed_heap;
}

} // namespace xpute
