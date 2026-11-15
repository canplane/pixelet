// cpp/xpute/guest/heap.hpp

// The guest's heap, as `xpute-guest`'s `heap.rs` is a Rust guest's: kit's
// slab of size classes over its buddy allocator of pages (`slab.hpp`), in
// the guest's memory as its runtime sized it (`xpute/link/mem.hpp`
// `region`), past the program's share. A module's share begins on the page
// past its data; a native guest's at its region's base. The heap runs from
// the page past the share to the memory's end, and is never grown.
//
// A C++ guest takes what it lends its host from `heap()`, as a `std::pmr`
// container given it, in a module and natively alike. A library never
// replaces the global `operator new` of a process it shares, so a native
// guest's global `new` stays the system's. A module is an image of its own,
// and its global `new` and `delete` are this heap's, which nothing relies on
// for what crosses. The heap is the guest's thread's alone.

#pragma once

#include <cstdint>
#include <memory_resource>

namespace xpute::guest {

// Where the program's share begins: as many bytes as it gave its runtime,
// zeroed, its own for the guest's life.
std::uintptr_t share_at() noexcept;

// The heap, for what crosses the link. A request it refuses is a bug: the
// memory is spent.
std::pmr::memory_resource &heap() noexcept;

} // namespace xpute::guest
