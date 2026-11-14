// cpp/xpute/core/abi/clocks/system_clock.hpp
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

#pragma once

#include <cstddef>
#include <cstdint>

#include "xpute/link/cabi.hpp"

namespace xpute::abi::clocks::system_clock {

// The interface's handle, named by `xpute:clocks/system-clock` (frame.json `NAMED`).
inline constexpr std::uint32_t HANDLE = 0x7dfa61;

// `now`
inline constexpr std::uint32_t NOW = 0;

struct Instant {
    std::int64_t seconds;
    std::uint32_t nanoseconds;

    static constexpr std::size_t SZ = 16;
    static constexpr std::size_t ALIGN = 8;

    void store(::xpute::link::Lower &w, std::size_t at) const noexcept {
        w.put<std::int64_t>(at + 0, this->seconds);
        w.put<std::uint32_t>(at + 8, this->nanoseconds);
    }

    static Instant load(::xpute::link::Lift &r, std::size_t at) noexcept {
        return Instant{
            .seconds = r.get<std::int64_t>(at + 0),
            .nanoseconds = r.get<std::uint32_t>(at + 8),
        };
    }
};

} // namespace xpute::abi::clocks::system_clock
