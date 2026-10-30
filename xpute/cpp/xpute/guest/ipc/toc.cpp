// cpp/xpute/guest/ipc/toc.cpp

#include "toc.hpp"

#include "xpute/kit/status/bug.hpp"

namespace xpute {

void write_toc(Memory mem, std::uint32_t at, std::uint32_t words, std::span<const Entry> entries) noexcept {
    ensure(toc_words(static_cast<std::uint32_t>(entries.size())) <= words, Errno::enospc, entries.size());
    for (std::size_t i = 0; i < entries.size(); i++) {
        for (std::size_t j = 0; j < i; j++) {
            ensure(entries[j].key != entries[i].key, Errno::einval, entries[i].key);
        }
    }
    mem.set_word(at, toc::MAGIC);
    mem.set_word(at + 4, toc::VERSION);
    mem.set_word(at + 8, static_cast<std::uint32_t>(entries.size()));
    mem.set_word(at + 12, 0);
    std::uint32_t off = at + 4 * toc::HEAD;
    for (const Entry &e : entries) {
        mem.set_word(off, e.key);
        mem.set_word(off + 4, e.val);
        off += 8;
    }
}

} // namespace xpute
