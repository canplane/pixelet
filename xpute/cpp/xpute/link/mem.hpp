// cpp/xpute/link/mem.hpp

// The guest's memory, which every offset on the link names: offset `at` is
// the byte `at` past the memory's base, as xpute-link's `mem.rs` reads it. A
// module's memory begins at 0. A native guest's is the region its runtime
// reserves (`region`), which a host linked into the same process reads as
// its own.

#pragma once

#include <cstddef>
#include <cstdint>
#include <span>
#include <utility>

namespace xpute::link::mem {

// Where the guest's memory lies, as its runtime sized it before the guest's
// first crossing: its first byte, its size, and the bytes the program keeps
// for itself, which the guest's heap leaves before its own.
struct Region {
    std::uint8_t *base;
    std::uint32_t len;
    std::uint32_t share_sz;
};

// The guest's memory, as its runtime answers it (`xpute.h` `xpute_mem`). A
// native guest's region is reserved by the first call, from any thread.
Region region() noexcept;

// The buddy's smallest block (`xpute/guest/heap.hpp`). The share begins on
// one, and so does the heap past it.
inline constexpr std::size_t PAGE_SZ = 4096;

// Where the program's share and the guest's heap lie, as addresses: the
// share from `share`, the heap from `heap`, the page past the share, to the
// memory's end `end`. A module's share begins on the page past its data; a
// native guest's at its region's base.
struct Parts {
    std::uintptr_t share;
    std::uintptr_t heap;
    std::uintptr_t end;
};

// The parts of the memory, read from `region` by the first call, on the
// guest's thread; a share past the memory's end is a bug.
const Parts &parts() noexcept;

// The `len` bytes at `at`. A native guest's are held to its region: a range
// past it is a bug, as a module's memory traps past its end.
std::span<std::uint8_t> bytes(std::uint32_t at, std::uint32_t len) noexcept;

namespace detail {
// The offset of `p` in the guest's memory; an address whose offset does not
// fit a u32 is a bug, never truncated.
std::uint32_t off(const void *p) noexcept;

// Holds the `len` bytes at `p` to the guest's heap: a range outside it is a
// bug.
void held(const void *p, std::size_t len) noexcept;
} // namespace detail

// The offset of the value `v` in the guest's memory, such as a word the guest
// lends its host to store into.
template <class T> std::uint32_t off(const T &v) noexcept {
    return detail::off(&v);
}

// Where the range `s` lies in the guest's memory and how many bytes it takes,
// as a lent span (link/wit/types.wit `lent`). An empty range has no address,
// and lends (0, 0); any other lies whole in the guest's heap (`held`), in a
// module and natively alike, so a static or a stack's word lent stops a
// module as it stops a native guest.
template <class T> std::pair<std::uint32_t, std::uint32_t> span(std::span<const T> s) noexcept {
    if (s.empty()) return {0, 0};
    detail::held(s.data(), s.size_bytes());
    return {off(s.front()), static_cast<std::uint32_t>(s.size_bytes())};
}

// The little-endian word at `at`, aligned or not.
std::uint32_t word(std::uint32_t at) noexcept;

// Writes the little-endian word `v` at `at`, aligned or not.
void set_word(std::uint32_t at, std::uint32_t v) noexcept;

// Loads the aligned word at `at` with acquire: a word that hands memory
// between the guest and its host, such as a fence the guest reads while the
// host answers it, which both sides touch only atomically, so whatever the
// other side wrote before it stored the word is seen.
std::uint32_t load_acquire(std::uint32_t at) noexcept;

// Stores `v` into such a word with release, so whatever this side wrote
// before is seen by the side that loads it.
void store_release(std::uint32_t at, std::uint32_t v) noexcept;

} // namespace xpute::link::mem
