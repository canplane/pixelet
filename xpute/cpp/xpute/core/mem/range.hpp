// cpp/xpute/core/mem/range.hpp

// The core's fixed ranges are ranges of its own memory below its heap, sized
// at compile time, placed once at boot and never freed. Whoever lays out the
// memory hands each owner its range once (`OwnedRange`). A range is read only
// through `arr` and `Section::at`, which check its end.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>

#include "xpute/kit/status/bug.hpp"
#include "xpute/core/mem.hpp"
#include "section.hpp"

namespace xpute {

// This is one owner's range, handed in once at boot. Asking for it before then
// is a bug.
class OwnedRange {
  public:
    void set(Section s) noexcept { section_ = s; }

    Section get() const noexcept { return or_bug(section_, Errno::enotrecoverable); }

  private:
    std::optional<Section> section_;
};

// A cache line is this many bytes, and arrays are laid out in whole lines.
inline constexpr std::size_t LINE_BYTES = 64;

// This is the bytes an array of `cnt` items of `size` bytes takes, rounded up
// to whole cache lines.
constexpr std::uint32_t arr_bytes(std::size_t cnt, std::size_t size) noexcept {
    return static_cast<std::uint32_t>((cnt * size + LINE_BYTES - 1) / LINE_BYTES * LINE_BYTES);
}

// `arr` answers an array of `n` items of `T`, at `off` into `s` in the core's
// own memory. Sizes are fixed at compile time, so an array out of bounds or
// misaligned is a bug, not a refusal. The caller must hold those bytes alone
// for as long as it keeps the array, and they must be valid `T`s (zeroed, for
// plain numbers).
template <class T> std::span<T> arr(Section s, std::uint32_t off, std::size_t n) noexcept {
    std::uint64_t bytes = std::uint64_t{n} * sizeof(T);
    ensure(bytes <= UINT32_MAX, Errno::enotrecoverable, bytes);
    std::uint32_t at = or_bug(s.at(off, static_cast<std::uint32_t>(bytes)), Errno::enotrecoverable);
    ensure(at % alignof(T) == 0, Errno::enotrecoverable, at);
    return {reinterpret_cast<T *>(mem::at(at)), n};
}

} // namespace xpute
