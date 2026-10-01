// cpp/xpute/runtime/mem/memory.hpp

// The memory both sides see, as the guest holds it: where it begins, how many
// bytes it spans, and words read and written at offsets into it. Every
// address that crosses is an offset (ARCHITECTURE.xpute.md, the contract's
// rule 1), so this is the one place an offset becomes a pointer and a pointer
// an offset. A word is copied rather than dereferenced in place: an offset
// the other side names carries no alignment a compiler may assume.
//
// The base is an address, not a pointer. A WebAssembly module's memory begins
// at address 0, which as a pointer is null, and arithmetic on a null pointer
// is undefined: a compiler may drop what follows it. So an offset is added to
// the base as a number, and the sum becomes a pointer only to be read or
// written through, which no offset past the start of the memory makes null.
//
// The memory is a value rather than one per module, as Rust's base is: a
// native host may run several guests in one process, each over a region of
// its own, and a call names the one it is about.

#pragma once

#include <cstdint>
#include <cstring>
#include <span>

#include "../../core/status/bug.hpp"

namespace xpute {

class Memory {
  public:
    // The memory `bytes` long from `base`. A module's linear memory is the
    // memory from 0 to whatever it has grown to, which nothing it names lies
    // past.
    explicit Memory(std::uintptr_t base, std::uintptr_t bytes = UINTPTR_MAX) noexcept : base_(base), bytes_(bytes) {}

    std::uintptr_t base() const noexcept { return base_; }

    std::uint8_t *at(std::uint32_t off) const noexcept { return reinterpret_cast<std::uint8_t *>(base_ + off); }

    std::uint32_t word(std::uint32_t off) const noexcept {
        std::uint32_t w;
        std::memcpy(&w, at(off), sizeof w);
        return w;
    }

    void set_word(std::uint32_t off, std::uint32_t w) const noexcept { std::memcpy(at(off), &w, sizeof w); }

    // Where `p` lies, as an offset from the base. A pointer outside the
    // memory is a bug: what the guest names has to be in it, and on a native
    // build a static or the binary's read-only data is not.
    std::uint32_t off_of(const void *p, std::source_location loc = std::source_location::current()) const noexcept {
        std::uintptr_t a = reinterpret_cast<std::uintptr_t>(p);
        ensure(a >= base_ && a - base_ < bytes_ && a - base_ <= UINT32_MAX, Errno::efault, a, base_, loc);
        return static_cast<std::uint32_t>(a - base_);
    }

    // Where `s` lies, as `off_of` says, for a reader told its length too. An
    // empty span owns no bytes and its pointer is only its alignment, so it
    // names 0, where nothing is read.
    template <class T> std::uint32_t off_of(std::span<T> s, std::source_location loc = std::source_location::current()) const noexcept {
        return s.empty() ? 0 : off_of(static_cast<const void *>(s.data()), loc);
    }

  private:
    std::uintptr_t base_;
    std::uintptr_t bytes_;
};

} // namespace xpute
