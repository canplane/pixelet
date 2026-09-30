// cpp/xpute/core/abi/cmd.hpp

// A command's number: a major that names what it is about, and a minor that
// names what is done to it — `major << 8 | minor` in the 16 bits a message
// carries. The minors below 0x10 are the four every subject can have; a
// subject's own verbs start at MINOR_OWN. What the majors are is the
// program's, not this kit's.

#pragma once

#include <cstdint>

namespace xpute {

enum class Minor : std::uint32_t {
    create = 1,
    read = 2,
    update = 3,
    remove = 4,
};

// The first minor a subject numbers for itself.
inline constexpr std::uint32_t MINOR_OWN = 0x10;

constexpr std::uint32_t cmd(std::uint32_t major, std::uint32_t minor) noexcept {
    return (major & 0xff) << 8 | (minor & 0xff);
}

constexpr std::uint32_t cmd_major(std::uint32_t c) noexcept {
    return c >> 8 & 0xff;
}

constexpr std::uint32_t cmd_minor(std::uint32_t c) noexcept {
    return c & 0xff;
}

} // namespace xpute
