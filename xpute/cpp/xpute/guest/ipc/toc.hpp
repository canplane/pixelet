// cpp/xpute/guest/ipc/toc.hpp

// The toc a guest's first turn writes, where its host finds what it laid
// out: a head, then key-value entries, no key twice.

#pragma once

#include <array>
#include <cstdint>
#include <span>

#include "xpute/guest/mem.hpp"
#include "toc.spec.hpp"

namespace xpute {

struct Entry {
    std::uint32_t key;
    std::uint32_t val;
};

constexpr std::uint32_t toc_words(std::uint32_t entries) noexcept {
    return toc::HEAD + 2 * entries;
}

// The toc at `at`, `words` long. A key may appear once, so no two
// readers can pick different values.
void write_toc(Memory mem, std::uint32_t at, std::uint32_t words, std::span<const Entry> entries) noexcept;

} // namespace xpute
