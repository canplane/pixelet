// cpp/xpute/runtime/ipc/directory.cpp

#include "directory.hpp"

#include <cstdlib>

namespace xpute {

void write_directory(Memory mem, std::uint32_t at, std::span<const Entry> entries) noexcept {
    for (std::size_t i = 0; i < entries.size(); i++) {
        for (std::size_t j = 0; j < i; j++) {
            if (entries[j].key == entries[i].key) std::abort();
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

} // namespace xpute
