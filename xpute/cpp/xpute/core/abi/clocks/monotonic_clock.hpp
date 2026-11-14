// cpp/xpute/core/abi/clocks/monotonic_clock.hpp
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

#pragma once

#include <cstdint>

namespace xpute::abi::clocks::monotonic_clock {

// The interface's handle, named by `xpute:clocks/monotonic-clock` (frame.json `NAMED`).
inline constexpr std::uint32_t HANDLE = 0x73f703;

// `now`
inline constexpr std::uint32_t NOW = 0;
// `wait-for`
inline constexpr std::uint32_t WAIT_FOR = 1;

using Mark = std::uint64_t;

} // namespace xpute::abi::clocks::monotonic_clock
