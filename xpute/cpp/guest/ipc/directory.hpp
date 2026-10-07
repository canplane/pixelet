// cpp/guest/ipc/directory.hpp

// The directory a guest's first turn writes, where its host finds what it laid
// out: a head, then key-value entries, no key twice.

#pragma once

#include <array>
#include <cstdint>
#include <span>

#include "../mem/memory.hpp"
#include "../sched/quantum.hpp"
#include "directory.spec.hpp"

namespace xpute {

struct Entry {
    std::uint32_t key;
    std::uint32_t value;
};

constexpr std::uint32_t directory_words(std::uint32_t entries) noexcept {
    return directory::HEAD + 2 * entries;
}

// The directory at `at`, `words` long. A key may appear once, so no two
// readers can pick different values.
void write_directory(Memory mem, std::uint32_t at, std::uint32_t words, std::span<const Entry> entries) noexcept;

// Each number in `directory::QUANTUM_SCALE`.
std::array<Entry, 3> quantum_entries(const QuantumPolicy &q) noexcept;

} // namespace xpute
