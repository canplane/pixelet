// cpp/xpute/runtime/ipc/frame.hpp

// A frame: one message, four words — who asked (the tag), what is asked (a
// command in the low half of word 1, its flags in the high half), what came
// back (a value, or a negative errno), and where its packet is (an offset
// into the memory, 0 for none). The contract's rule 3 in ARCHITECTURE.xpute.md
// says what the flags mean.

#pragma once

#include <cstdint>

namespace xpute::frame {

inline constexpr std::uint32_t TAG = 0;
inline constexpr std::uint32_t CMD = 1;
inline constexpr std::uint32_t RESULT = 2;
inline constexpr std::uint32_t PACKET = 3;
inline constexpr std::uint32_t WORDS = 4;

// Where the flags start in word 1.
inline constexpr std::uint32_t FLAG_SHIFT = 16;

// A reply: set by whoever sends it, so a completion without it is one the
// guest raised on its own.
inline constexpr std::uint32_t RES = 1;

// Exactly one reply is sent; without it, one is sent only when the message
// failed.
inline constexpr std::uint32_t ACKREQ = 2;

constexpr std::uint32_t cmd_word(std::uint32_t cmd, std::uint32_t flags) noexcept {
    return flags << FLAG_SHIFT | (cmd & 0xffffu);
}

constexpr std::uint32_t cmd_of(std::uint32_t word) noexcept {
    return word & 0xffffu;
}

constexpr std::uint32_t flags_of(std::uint32_t word) noexcept {
    return word >> FLAG_SHIFT;
}

} // namespace xpute::frame
