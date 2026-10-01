// cpp/xpute/runtime/ipc/directory.hpp

// The directory a guest's first turn writes: where its host finds what it
// laid out (ARCHITECTURE.xpute.md, the contract's rule 0). Its format is
// directory.spec.hpp's: a head, then entries of a key and a value, no key
// twice.

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

// Words a directory of `entries` entries takes.
constexpr std::uint32_t directory_words(std::uint32_t entries) noexcept {
    return directory::HEAD + 2 * entries;
}

// Writes `entries` at `at`. A key given twice is a guest that could not say
// which value it means, and stops.
void write_directory(Memory mem, std::uint32_t at, std::span<const Entry> entries) noexcept;

} // namespace xpute
