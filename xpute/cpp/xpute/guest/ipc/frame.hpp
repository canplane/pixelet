// cpp/xpute/guest/ipc/frame.hpp

// A frame's four words, laid out by spec/ipc/frame.json.

#pragma once

#include <cstdint>

#include "frame.spec.hpp"

namespace xpute {

// A frame's four words as read, its command word split.
struct Frame {
    std::uint32_t tag;
    std::uint32_t cmd;
    std::uint32_t flags;
    std::int32_t result;
    // 0 for none.
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

// The far port a frame's tag names.
constexpr std::uint32_t port_of(std::uint32_t tag) noexcept {
    return tag >> PORT_SHIFT;
}

// The tag of a frame for `port`, its id or subject `id`.
constexpr std::uint32_t tagged(std::uint32_t port, std::uint32_t id) noexcept {
    return port << PORT_SHIFT | id;
}

} // namespace xpute::frame
