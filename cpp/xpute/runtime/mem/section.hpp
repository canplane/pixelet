// cpp/xpute/runtime/mem/section.hpp

// Ranges laid end to end in a linear memory, as a linker script places
// sections at `. = ALIGN(n)` boundaries.

#pragma once

#include <cstddef>
#include <cstdint>
#include <optional>
#include <span>

#include "../../core/status/bug.hpp"

namespace xpute {

constexpr std::uint32_t align_up(std::uint32_t n, std::uint32_t unit) noexcept {
    return (n + unit - 1) / unit * unit;
}

// Includes the last one's own rounding, so this is where whatever follows starts.
constexpr std::uint32_t span_of(std::span<const std::uint32_t> sizes, std::uint32_t unit) noexcept {
    std::uint32_t span = 0;
    for (std::uint32_t s : sizes) span += align_up(s, unit);
    return span;
}

constexpr std::uint32_t nth_at(std::uint32_t base, std::span<const std::uint32_t> sizes, std::uint32_t unit, std::size_t n) noexcept {
    return base + span_of(sizes.first(n), unit);
}

struct Section {
    std::uint32_t offset = 0;
    std::uint32_t bytes = 0;
    // Rounded up, so the next section begins past where this one's content ends.
    std::uint32_t align = 0;

    bool operator==(const Section &) const = default;

    constexpr std::uint32_t end() const noexcept { return offset + bytes; }

    // None where they would run past the end. The one way into a section, so
    // nothing written through it can land in its neighbor.
    constexpr std::optional<std::uint32_t> at(std::uint32_t off, std::uint32_t n) const noexcept {
        std::uint64_t past = std::uint64_t{off} + n;
        if (past > bytes) return std::nullopt;
        return offset + off;
    }

    // What is laid must fit the section.
    constexpr Section nth(std::span<const std::uint32_t> sizes, std::uint32_t unit, std::size_t n) const noexcept {
        if (span_of(sizes, unit) > bytes) bug(Errno::einval, span_of(sizes, unit), bytes);
        return Section{nth_at(offset, sizes, unit, n), sizes[n], unit};
    }
};

} // namespace xpute
