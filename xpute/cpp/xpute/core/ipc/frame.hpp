// cpp/xpute/core/ipc/frame.hpp

// A frame's four words are laid out by spec/ipc/frame.json.

#pragma once

#include <cstdint>

#include "frame.spec.hpp"

namespace xpute {

// A frame's four words as read, with its command word split into the command
// and its flags.
struct Frame {
    std::uint32_t tag;
    std::uint32_t cmd;
    std::uint32_t flags;
    std::int32_t result;
    // Where the frame's packet lies in the memory, or 0 for no packet.
    std::uint32_t pkt_at;

    bool operator==(const Frame &) const = default;
};

} // namespace xpute

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
