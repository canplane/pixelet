// cpp/xpute/core/abi/enum.hpp

// The members of an enum generated from a table, as the Rust side's `ALL` and
// `of` give them.

#pragma once

#include <cstdint>
#include <optional>

namespace xpute::abi {

// `Members` holds the members of an enum generated from a table. The generated
// code specializes it beside the enum, with every member in `ALL`, as the Rust
// side's `ALL` lists them.
template <class E> struct Members;

// `member_of` answers the member whose number is `number`, or none when the
// table gives no member that number.
template <class E> constexpr std::optional<E> member_of(std::uint32_t number) noexcept {
    for (E m : Members<E>::ALL) {
        if (static_cast<std::uint32_t>(m) == number) return m;
    }
    return std::nullopt;
}

} // namespace xpute::abi
