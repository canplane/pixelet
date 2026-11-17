// cpp/xpute/link/mem.hpp

// The guest's memory, which every address on the link names, a u64 on every
// platform (link/wit/types.wit `lent`), as xpute-link's `mem.rs` reads it: a
// module's offset into its own memory, which begins at 0, so its offset is its
// pointer, or a native guest's pointer, which a host linked into the same
// process reads as its own. An address past what the platform's pointer holds
// is a bug.

#pragma once

#include <cstddef>
#include <cstdint>
#include <span>
#include <utility>

#include "xpute/kit/status/bug.hpp"
#include "xpute/link/abi/types.hpp"

namespace xpute::link::mem {

using Lent = ::xpute::abi::link::types::Lent;

// The `len` bytes at `at`.
std::span<std::uint8_t> bytes(std::uint64_t at, std::uint32_t len) noexcept;

namespace detail {
// The address of `p`.
std::uint64_t addr(const void *p) noexcept;
} // namespace detail

// The address of the value `v`, such as a word the guest lends its host to
// store into.
template <class T> std::uint64_t addr(const T &v) noexcept {
    return detail::addr(&v);
}

// The range `s` as a lent buffer, where it lies and how many bytes it takes
// (link/wit/types.wit `lent`). An empty range has no address, and lends
// (0, 0), in a module and natively alike; one past 4 GiB is a bug.
template <class T> Lent lent(std::span<const T> s) noexcept {
    if (s.empty()) return Lent{.buf = 0, .nbyte = 0};
    ensure(s.size_bytes() <= UINT32_MAX, Errno::efault);
    return Lent{.buf = addr(s.front()), .nbyte = static_cast<std::uint32_t>(s.size_bytes())};
}

// The little-endian word at `at`, aligned or not.
std::uint32_t word(std::uint64_t at) noexcept;

// Writes the little-endian word `v` at `at`, aligned or not.
void set_word(std::uint64_t at, std::uint32_t v) noexcept;

// Loads the aligned word at `at` with acquire: a word that hands memory
// between the guest and its host, such as a fence the guest reads while the
// host answers it, which both sides touch only atomically, so whatever the
// other side wrote before it stored the word is seen.
std::uint32_t load_acquire(std::uint64_t at) noexcept;

// Stores `v` into such a word with release, so whatever this side wrote
// before is seen by the side that loads it.
void store_release(std::uint64_t at, std::uint32_t v) noexcept;

} // namespace xpute::link::mem
