// cpp/guest/ipc/directory.cpp

#include "directory.hpp"

#include "../../kit/math/scalar.hpp"
#include "../../kit/status/bug.hpp"

namespace xpute {

void write_directory(Memory mem, std::uint32_t at, std::uint32_t words, std::span<const Entry> entries) noexcept {
    ensure(directory_words(static_cast<std::uint32_t>(entries.size())) <= words, Errno::enospc, entries.size());
    for (std::size_t i = 0; i < entries.size(); i++) {
        for (std::size_t j = 0; j < i; j++) {
            ensure(entries[j].key != entries[i].key, Errno::einval, entries[i].key);
        }
    }
    mem.set_word(at, directory::MAGIC);
    mem.set_word(at + 4, directory::VERSION);
    mem.set_word(at + 8, static_cast<std::uint32_t>(entries.size()));
    mem.set_word(at + 12, 0);
    std::uint32_t off = at + 4 * directory::HEAD;
    for (const Entry &e : entries) {
        mem.set_word(off, e.key);
        mem.set_word(off + 4, e.value);
        off += 8;
    }
}

std::array<Entry, 3> quantum_entries(const QuantumPolicy &q) noexcept {
    auto scaled = [](double v) { return static_cast<std::uint32_t>(round(v * directory::QUANTUM_SCALE)); };
    return {{
        {directory::key::MARGIN_SHARE, scaled(q.margin_share)},
        {directory::key::BATCH_FRAMES, scaled(q.batch_frames)},
        {directory::key::SETTLE_MS, scaled(q.settle_ms)},
    }};
}

} // namespace xpute
