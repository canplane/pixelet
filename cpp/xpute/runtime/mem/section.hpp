// cpp/xpute/runtime/mem/section.hpp

// Ranges laid end to end in a linear memory: the mechanism under a guest's
// memory map, and a linker script's shape. `ld` places output sections in
// the order the script gives, each at a `. = ALIGN(n)` boundary, and answers
// where each one landed; which sections there are, what they hold and what
// `n` is are the script's. Nothing here knows what a range holds either — it
// is told a base, a unit and a list of sizes.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>

#include "../../core/status/bug.hpp"

namespace xpute {

// `n` at the next multiple of `unit`, which is where the range holding `n`
// bytes ends and the one after it may begin.
constexpr std::uint32_t align_up(std::uint32_t n, std::uint32_t unit) noexcept {
    return (n + unit - 1) / unit * unit;
}

// What `sizes` span when laid end to end, each beginning on `unit`. The last
// one's own rounding is in here too, which is what makes this the offset of
// the boundary whatever follows them starts on.
constexpr std::uint32_t span_of(std::span<const std::uint32_t> sizes, std::uint32_t unit) noexcept {
    std::uint32_t span = 0;
    for (std::uint32_t s : sizes) span += align_up(s, unit);
    return span;
}

// Where the `n`th of `sizes` begins, laid from `base` on `unit`.
constexpr std::uint32_t nth_at(std::uint32_t base, std::span<const std::uint32_t> sizes, std::uint32_t unit, std::size_t n) noexcept {
    return base + span_of(sizes.first(n), unit);
}

struct Section {
    std::uint32_t offset = 0;
    std::uint32_t bytes = 0;
    // The boundary the section begins on, and so the one the next begins on
    // past where this one's content ends.
    std::uint32_t align = 0;

    bool operator==(const Section &) const = default;

    // Where the section ends: the first byte past it.
    constexpr std::uint32_t end() const noexcept { return offset + bytes; }

    // Where `n` bytes from `off` into the section lie, or none where they
    // would run past its end. The one way into a section: a range is reached
    // through this and never by adding to its offset, so nothing written
    // through it can land in its neighbor.
    constexpr std::optional<std::uint32_t> at(std::uint32_t off, std::uint32_t n) const noexcept {
        std::uint64_t past = std::uint64_t{off} + n;
        if (past > bytes) return std::nullopt;
        return offset + off;
    }

    // The `n`th of `sizes` laid end to end inside the section, each on
    // `unit`: a section of its own, which ends `sizes[n]` past where it
    // begins. What is laid must fit the section, or a range inside it would
    // reach past it.
    constexpr Section nth(std::span<const std::uint32_t> sizes, std::uint32_t unit, std::size_t n) const noexcept {
        if (span_of(sizes, unit) > bytes) bug(Errno::einval, span_of(sizes, unit), bytes);
        return Section{nth_at(offset, sizes, unit, n), sizes[n], unit};
    }
};

} // namespace xpute
