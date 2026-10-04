// cpp/xpute/runtime/ipc/frame.hpp

// A message is four words: tag, command (low half) and flags (high half),
// result (a value or a negative errno), and packet offset (0 for none).

#pragma once

#include <cstdint>

namespace xpute::frame {

inline constexpr std::uint32_t TAG = 0;
inline constexpr std::uint32_t CMD = 1;
inline constexpr std::uint32_t RESULT = 2;
inline constexpr std::uint32_t PACKET = 3;
inline constexpr std::uint32_t WORDS = 4;

inline constexpr std::uint32_t FLAG_SHIFT = 16;

// Set by the sender, so a completion without it was raised by the guest.
inline constexpr std::uint32_t RES = 1;

// Without it, a reply is sent only on failure.
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
