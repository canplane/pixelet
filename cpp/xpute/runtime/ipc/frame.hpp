// cpp/xpute/runtime/ipc/frame.hpp

// A message's four words, laid out by spec/xpute/ipc/frame.json.

#pragma once

#include <cstdint>

#include "frame.spec.hpp"

namespace xpute::frame {

inline constexpr std::uint32_t CMD_MASK = (1u << FLAG_SHIFT) - 1;

constexpr std::uint32_t cmd_word(std::uint32_t cmd, std::uint32_t flags) noexcept {
    return flags << FLAG_SHIFT | (cmd & CMD_MASK);
}

constexpr std::uint32_t cmd_of(std::uint32_t word) noexcept {
    return word & CMD_MASK;
}

constexpr std::uint32_t flags_of(std::uint32_t word) noexcept {
    return word >> FLAG_SHIFT;
}

} // namespace xpute::frame
