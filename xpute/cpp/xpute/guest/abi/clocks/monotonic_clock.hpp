// cpp/xpute/guest/abi/clocks/monotonic_clock.hpp
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

#pragma once

#include <cstdint>
#include <optional>
#include <utility>
#include <variant>

#include "xpute/guest/link.hpp"
#include "xpute/guest/wait.hpp"
#include "xpute/kit/status/error.hpp"
#include "xpute/link/cabi.hpp"
#include "xpute/link/link.hpp"
#include "xpute/link/wait.hpp"

namespace xpute::abi::clocks::monotonic_clock {

namespace _rt {
using ::xpute::Result;
using ::xpute::guest::call;
using ::xpute::guest::wait;
using ::xpute::link::Handle;
using ::xpute::link::Lift;
using ::xpute::link::Lower;
using ::xpute::link::Method;
using ::xpute::link::Packet;
using ::xpute::link::Task;
using ::xpute::link::answer;
} // namespace _rt

// The interface's handle, named by `xpute:clocks/monotonic-clock` (frame.json5 `NAMED`).
inline constexpr _rt::Handle HANDLE = 0x73f703;

// `now`
inline constexpr _rt::Method NOW = 0;
// `wait-for`
inline constexpr _rt::Method WAIT_FOR = 1;

using Mark = std::uint64_t;

inline _rt::Result<std::uint64_t> now() noexcept {
    auto [result_, data_] = _rt::call(HANDLE, NOW, std::nullopt);
    return _rt::answer(result_, data_, [&](_rt::Lift &r_) { return r_.get<std::uint64_t>(0); });
}

inline _rt::Task<_rt::Result<void>> wait_for(std::uint64_t how_long) noexcept {
    auto sent_ = _rt::wait(HANDLE, WAIT_FOR, _rt::Packet{8, [&](_rt::Lower &w_) { w_.put<std::uint64_t>(0, how_long); }}, std::monostate());
    auto [result_, data_, lent_] = co_await std::move(sent_);
    co_return _rt::answer(result_);
}

} // namespace xpute::abi::clocks::monotonic_clock
