// cpp/xpute/core/io/head.hpp

// `Head` is the header of the memory's front (spec/io/head.json). A runtime
// writes these words in the page past the stack before the first turn, and
// they say how much a frame's packet takes and where the stream it laid out
// lies. A core that lays out its own front, as the conformance core does,
// writes the header itself (`write_head`).

#pragma once

#include <array>
#include <cstdint>

#include "xpute/core/mem.hpp"
#include "head.spec.hpp"

namespace xpute {

// The header's words after MAGIC.
struct Head {
    std::uint32_t pkt_bytes;
    // 0 without a stream.
    std::uint32_t stream_at;

    constexpr std::array<std::uint32_t, head::WORDS> words() const noexcept {
        std::array<std::uint32_t, head::WORDS> w{};
        w[head::word::MAGIC] = head::MAGIC;
        w[head::word::PKT_BYTES] = pkt_bytes;
        w[head::word::STREAM_AT] = stream_at;
        return w;
    }
};

inline void write_head(Memory mem, std::uint32_t at, const Head &h) noexcept {
    std::array<std::uint32_t, head::WORDS> w = h.words();
    for (std::uint32_t i = 0; i < head::WORDS; i++) mem.set_word(at + 4 * i, w[i]);
}

} // namespace xpute
