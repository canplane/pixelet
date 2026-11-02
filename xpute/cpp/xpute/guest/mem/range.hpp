// cpp/xpute/guest/mem/range.hpp

// A range of the guest's own ahead of its heap: placed once at boot, never
// freed, sized at compile time. Whoever lays the memory out hands each owner
// its range once (`OwnedRange`); a range is read only through `arr` and
// `Section::at`, which check its end.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>

#include "xpute/kit/status/bug.hpp"
#include "xpute/guest/mem.hpp"
#include "section.hpp"

namespace xpute {

// One owner's range, handed in once at boot; asking before is a bug.
class OwnedRange {
  public:
    void set(Section s) noexcept { held_ = s; }

    Section get() const noexcept { return or_bug(held_, Errno::enotrecoverable); }

  private:
    std::optional<Section> held_;
};

// A cache line, which an array is laid on.
inline constexpr std::size_t LINE_BYTES = 64;

// `cnt` of `size` bytes, rounded up to a whole cache line, as an array is laid.
constexpr std::uint32_t arr_bytes(std::size_t cnt, std::size_t size) noexcept {
    return static_cast<std::uint32_t>((cnt * size + LINE_BYTES - 1) / LINE_BYTES * LINE_BYTES);
}

// `n` of `T` from `off` into `s`, in the guest's own memory. Sizes are fixed at
// compile time, so out of bounds or misaligned is a bug, not a refusal. The
// caller holds those bytes alone while it keeps the array, and they are a valid
// `T` (zeroed, for plain numbers).
template <class T> std::span<T> arr(Section s, std::uint32_t off, std::size_t n) noexcept {
    std::uint64_t bytes = std::uint64_t{n} * sizeof(T);
    ensure(bytes <= UINT32_MAX, Errno::enotrecoverable, bytes);
    std::uint32_t at = or_bug(s.at(off, static_cast<std::uint32_t>(bytes)), Errno::enotrecoverable);
    ensure(at % alignof(T) == 0, Errno::enotrecoverable, at);
    return {reinterpret_cast<T *>(mem::at(at)), n};
}

} // namespace xpute
