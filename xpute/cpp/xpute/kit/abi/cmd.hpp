// cpp/xpute/kit/abi/cmd.hpp

// A command number is `major << 8 | minor`, which fits the 16 bits a frame
// carries for it.

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

// The members of an enum generated from a table. The generated code
// specializes it beside the enum, with every member in `ALL`, as the Rust
// side's `ALL` lists them.
template <class E> struct Members;

// The member whose number is `number`, or none when the table gives no
// member that number.
template <class E> constexpr std::optional<E> member_of(std::uint32_t number) noexcept {
    for (E m : Members<E>::ALL) {
        if (static_cast<std::uint32_t>(m) == number) return m;
    }
    return std::nullopt;
}

} // namespace xpute
