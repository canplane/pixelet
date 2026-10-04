// cpp/xpute/runtime/ipc/directory.hpp

// The directory a guest's first turn writes, where its host finds what it laid
// out: a head, then key-value entries, no key twice.

#pragma once

#include <cstdint>
#include <span>

#include "../mem/memory.hpp"
#include "directory.spec.hpp"

namespace xpute {

struct Entry {
    std::uint32_t key;
    std::uint32_t value;
};

constexpr std::uint32_t directory_words(std::uint32_t entries) noexcept {
    return directory::HEAD + 2 * entries;
}

// A key given twice is a bug.
void write_directory(Memory mem, std::uint32_t at, std::span<const Entry> entries) noexcept;

} // namespace xpute
