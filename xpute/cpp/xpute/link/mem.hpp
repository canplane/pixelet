// cpp/xpute/link/mem.hpp

// The guest's memory, which every offset on the link names: offset `at` is
// the byte `at` past the memory's base, as xpute-link's `mem.rs` reads it. A
// module's memory begins at 0. A native guest's is the one region it
// reserves as it starts (`set`), which a host linked into the same process
// reads as its own.

#pragma once

#include <cstdint>
#include <span>
#include <utility>

namespace xpute::link::mem {

// Where a native guest's memory lies.
struct Region {
    std::uint8_t *base;
    std::uint32_t len;
};

#ifndef __wasm__
// Sets the native guest's region, once, before anything reaches it: the
// guest reserves it and its allocator hands out only memory inside it, so
// every offset fits a u32. A second set is a bug.
void set(Region r) noexcept;

// The native guest's region.
Region region() noexcept;
#endif

// The `len` bytes at `at`. A native guest's are held to its region: a range
// past it is a bug, as a module's memory traps past its end.
std::span<std::uint8_t> bytes(std::uint32_t at, std::uint32_t len) noexcept;

namespace detail {
// The offset of `p` in the guest's memory; an address whose offset does not
// fit a u32 is a bug, never truncated.
std::uint32_t off(const void *p) noexcept;
} // namespace detail

// The offset of the value `v` in the guest's memory, such as a word the guest
// lends its host to store into.
template <class T> std::uint32_t off(const T &v) noexcept {
    return detail::off(&v);
}

// Where the range `s` lies in the guest's memory and how many bytes it takes,
// as a lent span (link/wit/types.wit `lent`). An empty range has no address,
// and lends (0, 0), in a module and natively alike; any other lies whole in
// the memory, a native guest's in its region (`bytes`).
template <class T> std::pair<std::uint32_t, std::uint32_t> span(std::span<const T> s) noexcept {
    if (s.empty()) return {0, 0};
    std::uint32_t at = off(s.front()), len = static_cast<std::uint32_t>(s.size_bytes());
    bytes(at, len);
    return {at, len};
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
