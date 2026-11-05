// cpp/xpute/guest/ipc/head.hpp

// The front's header (spec/ipc/head.json): the words a host writes on the page
// past the stack before the first turn, naming the rings and the GPU stream it
// laid. A guest that lays its own front, as the conformance guest does,
// writes it (`write_head`).

#pragma once

#include <array>
#include <cstdint>

#include "xpute/guest/mem.hpp"
#include "head.spec.hpp"

namespace xpute {

// What the header says past MAGIC and VERSION.
struct Head {
    std::uint32_t sq_cap;
    std::uint32_t cq_cap;
    std::uint32_t slot_bytes;
    std::uint32_t sq_at;
    std::uint32_t cq_at;
    // 0 without the GPU socket.
    std::uint32_t gpu_at;

    // Every word, in the header's order.
    constexpr std::array<std::uint32_t, head::WORDS> words() const noexcept {
        std::array<std::uint32_t, head::WORDS> w{};
        w[head::word::MAGIC] = head::MAGIC;
        w[head::word::VERSION] = head::VERSION;
        w[head::word::SQ_CAP] = sq_cap;
        w[head::word::CQ_CAP] = cq_cap;
        w[head::word::SLOT_BYTES] = slot_bytes;
        w[head::word::SQ_AT] = sq_at;
        w[head::word::CQ_AT] = cq_at;
        w[head::word::GPU_AT] = gpu_at;
        return w;
    }
};

// `h`'s words at `at` in `mem`.
inline void write_head(Memory mem, std::uint32_t at, const Head &h) noexcept {
    std::array<std::uint32_t, head::WORDS> w = h.words();
    for (std::uint32_t i = 0; i < head::WORDS; i++) mem.set_word(at + 4 * i, w[i]);
}

} // namespace xpute
