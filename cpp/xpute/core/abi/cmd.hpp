// cpp/xpute/core/abi/cmd.hpp

// `major << 8
// minor` in the 16 bits a message carries.

#pragma once

#include <cstdint>
#include <optional>

namespace xpute {

constexpr std::uint32_t cmd(std::uint32_t major, std::uint32_t minor) noexcept {
    return (major & 0xff) << 8 | (minor & 0xff);
}

constexpr std::uint32_t cmd_major(std::uint32_t c) noexcept {
    return c >> 8 & 0xff;
}

constexpr std::uint32_t cmd_minor(std::uint32_t c) noexcept {
    return c & 0xff;
}

// Declared beside the enum by its table, as Rust's `ALL`.
template <class E> struct Members;

// None for a number nobody gave out.
template <class E> constexpr std::optional<E> member_of(std::uint32_t number) noexcept {
    for (E m : Members<E>::ALL) {
        if (static_cast<std::uint32_t>(m) == number) return m;
    }
    return std::nullopt;
}

} // namespace xpute
