// cpp/xpute/core/abi/cmd.hpp

// A command's number: a major that names what it is about, and a minor that
// names what is done to it — `major << 8 | minor` in the 16 bits a message
// carries. What the majors and minors are is the program's, not this kit's.

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

// The members of a command table's enum, in the order the table gives them:
// declared beside the enum by the table, as Rust's `ALL` is.
template <class E> struct Members;

// The member a number names, or none for a number nobody gave out, as Rust's
// `of`.
template <class E> constexpr std::optional<E> member_of(std::uint32_t number) noexcept {
    for (E m : Members<E>::ALL) {
        if (static_cast<std::uint32_t>(m) == number) return m;
    }
    return std::nullopt;
}

} // namespace xpute
