// cpp/xpute/core/mem.hpp

// `Memory` is the memory both sides see, and the one place where an offset
// becomes a pointer. Every address a core names is an offset from a memory's
// base, never a pointer, so that it means the same on every runtime and fits in
// 32 bits. A part laid over a region of memory is given the `Memory` it lies in
// and an offset. The core's own memory is `mem::core`.

#pragma once

#include <cstdint>
#include <cstring>
#include <span>

#include "xpute/kit/status/bug.hpp"

namespace xpute {

class Memory {
  public:
    explicit Memory(std::uintptr_t base, std::uintptr_t bytes = UINTPTR_MAX) noexcept : base_(base), bytes_(bytes) {}

    std::uintptr_t base() const noexcept { return base_; }

    std::uint8_t *at(std::uint32_t off) const noexcept { return reinterpret_cast<std::uint8_t *>(base_ + off); }

    // Reads the word by copy, since an offset from the other side carries no
    // alignment a compiler may assume. A word past the memory's end is a bug.
    std::uint32_t word(std::uint32_t off, std::source_location loc = std::source_location::current()) const noexcept {
        within(off, sizeof(std::uint32_t), loc);
        std::uint32_t w;
        std::memcpy(&w, at(off), sizeof w);
        return w;
    }

    void set_word(std::uint32_t off, std::uint32_t w, std::source_location loc = std::source_location::current()) const noexcept {
        within(off, sizeof w, loc);
        std::memcpy(at(off), &w, sizeof w);
    }

    // A pointer outside the memory is a bug. On a native build, statics and
    // read-only data lie outside the memory.
    std::uint32_t off_of(const void *p, std::source_location loc = std::source_location::current()) const noexcept {
        std::uintptr_t a = reinterpret_cast<std::uintptr_t>(p);
        ensure(a >= base_ && a - base_ < bytes_ && a - base_ <= UINT32_MAX, Errno::efault, a, base_, loc);
        return static_cast<std::uint32_t>(a - base_);
    }

    // An empty span's pointer is only its alignment and may lie outside the
    // region, so an empty span names 0.
    template <class T> std::uint32_t off_of(std::span<T> s, std::source_location loc = std::source_location::current()) const noexcept {
        return s.empty() ? 0 : off_of(static_cast<const void *>(s.data()), loc);
    }

    // Whether the `len` bytes at `off` lie inside the memory.
    bool holds(std::uint32_t off, std::uint32_t len) const noexcept {
        return static_cast<std::uint64_t>(off) + len <= bytes_;
    }

  private:
    void within(std::uint32_t off, std::uint32_t len, std::source_location loc) const noexcept {
        ensure(holds(off, len), Errno::efault, off, bytes_, loc);
    }

    std::uintptr_t base_;
    std::uintptr_t bytes_;
};

namespace mem {

// Sets the core's own memory, its start and size, once before anything is
// allocated. Until then it starts at 0 and spans everything, which is what a
// module and a native test need.
void set(std::uintptr_t base, std::uintptr_t bytes = UINTPTR_MAX) noexcept;

Memory core() noexcept;

inline std::uint8_t *at(std::uint32_t off) noexcept {
    return core().at(off);
}

inline std::uint32_t off_of(const void *p, std::source_location loc = std::source_location::current()) noexcept {
    return core().off_of(p, loc);
}

template <class T> std::uint32_t off_of(std::span<T> s, std::source_location loc = std::source_location::current()) noexcept {
    return core().off_of(s, loc);
}

} // namespace mem

} // namespace xpute
